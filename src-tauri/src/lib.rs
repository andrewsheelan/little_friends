use std::num::NonZero;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use lumi_core::app::{
    apply_fetch_report, apply_tag_result, download_client, App, DownloadProgress, FetchReport,
    Pose, PublicState, STATUS_EMPTY_HEARING, STATUS_MIC,
};
use lumi_core::backup::{backup_picker_start, sync_backup};
use lumi_core::audio::{frame_mouth_levels, resample_linear, take_max_duration};
use lumi_core::ollama::{ollama_serve_command, OllamaClient, OLLAMA_BASE};
use lumi_core::packs::{download_bytes, extract_tar_bz2, missing_files, speech_packs};
use lumi_core::prompts::speech_rate;
use lumi_core::speech_text::{natural_speech, spoken_beats, spoken_in_tamil};
use lumi_core::tamil_listen;
use lumi_core::tamil_voice::{self, TamilVoice};
use lumi_core::settings::{
    app_support_dir, load_memories, load_settings, memories_path, save_memories, save_settings,
    settings_path, Language, Personality, VoiceId,
};
use lumi_core::turn::STATUS_COULD_NOT_SPEAK;
use rodio::buffer::SamplesBuffer;
use tauri::{Emitter, Runtime};

mod speech;

type SharedApp = Arc<Mutex<App>>;

/// The active microphone capture plus a counter that is bumped whenever a
/// capture is started or cancelled, so an open that finishes late can tell it
/// has been superseded and must not be stored.
#[derive(Default)]
struct RecorderState {
    recording: Option<speech::Recording>,
    generation: u64,
}

type SharedRecorder = Arc<Mutex<RecorderState>>;

/// One fetch at a time, so a retry click cannot extract the same archive twice.
static PACK_FETCH_BUSY: AtomicBool = AtomicBool::new(false);

struct PackFetchGuard;

impl Drop for PackFetchGuard {
    fn drop(&mut self) {
        PACK_FETCH_BUSY.store(false, Ordering::SeqCst);
    }
}

fn lock_recorder(recorder: &SharedRecorder) -> Result<MutexGuard<'_, RecorderState>, String> {
    recorder
        .lock()
        .map_err(|_| "recorder is unavailable".to_string())
}

/// Cancels any capture, open or running. Dropping the `Recording` closes the mic.
fn cancel_capture(recorder: &SharedRecorder) -> Option<speech::Recording> {
    let mut state = recorder.lock().ok()?;
    state.generation += 1;
    state.recording.take()
}

const OLLAMA_WAIT: Duration = Duration::from_secs(20);
const OLLAMA_POLL: Duration = Duration::from_millis(500);
const PLAYBACK_TICK: Duration = Duration::from_millis(50);
/// Longest utterance passed to Whisper, in seconds. The pause detector already
/// ends a turn at 30 seconds of speech; this leaves room for the quiet before it.
const MAX_LISTEN_SECS: u32 = 45;
const WHISPER_RATE: u32 = 16_000;

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn lock(shared: &SharedApp) -> Result<MutexGuard<'_, App>, String> {
    shared
        .lock()
        .map_err(|_| "state is unavailable".to_string())
}

fn emit_state<R: Runtime>(target: &impl Emitter<R>, state: &PublicState) {
    let _ = target.emit("state", state);
}

fn emit_status<R: Runtime>(target: &impl Emitter<R>, status: &str) {
    let _ = target.emit("status", status);
}

fn emit_progress<R: Runtime>(target: &impl Emitter<R>, pack: &str, percent: u8) {
    let _ = target.emit(
        "download_progress",
        DownloadProgress {
            pack: pack.to_string(),
            percent,
            failed: false,
        },
    );
}

fn emit_failure<R: Runtime>(target: &impl Emitter<R>, pack: &str) {
    let _ = target.emit(
        "download_progress",
        DownloadProgress {
            pack: pack.to_string(),
            percent: 0,
            failed: true,
        },
    );
}

/// Downloads every speech pack whose files are missing. The App mutex is never
/// held while a download or extraction runs. A second call while one is in
/// progress returns without starting another extract.
fn fetch_packs<R: Runtime>(target: &impl Emitter<R>, shared: &SharedApp, home: &Path) {
    if PACK_FETCH_BUSY
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }
    let _guard = PackFetchGuard;
    let models = app_support_dir(home).join("models");
    let mut report = FetchReport {
        whisper: false,
        tamil: false,
        kokoro: false,
        failures: Vec::new(),
    };
    let client = download_client();
    for pack in speech_packs() {
        if !missing_files(&models, pack).is_empty() {
            emit_progress(target, pack.label, 0);
            let fetched = match (&client, pack.id) {
                (Ok(client), "tamil-listen") => tamil_listen::fetch(&models, client),
                (Ok(client), _) => download_bytes(pack.url, client)
                    .and_then(|bytes| extract_tar_bz2(&bytes, &models)),
                (Err(error), _) => Err(error.clone()),
            };
            if fetched.is_ok() && missing_files(&models, pack).is_empty() {
                emit_progress(target, pack.label, 100);
            }
        }
        let loaded = missing_files(&models, pack).is_empty();
        match pack.id {
            "whisper" => report.whisper = loaded,
            "tamil-listen" => report.tamil = loaded,
            "kokoro" => report.kokoro = loaded,
            _ => {}
        }
        if !loaded {
            report.failures.push(pack.label.to_string());
            emit_failure(target, pack.label);
        }
    }
    for voice in TamilVoice::ALL {
        if tamil_voice::is_ready(&models, voice) {
            continue;
        }
        let label = format!("Tamil voice {}", voice.label());
        emit_progress(target, &label, 0);
        let fetched = match &client {
            Ok(client) => tamil_voice::fetch(&models, voice, client),
            Err(error) => Err(error.clone()),
        };
        if fetched.is_ok() {
            emit_progress(target, &label, 100);
        } else {
            report.failures.push(label.clone());
            emit_failure(target, &label);
        }
    }
    if let Some(label) = report.failures.last() {
        emit_failure(target, label);
    }
    {
        // Emit while the lock is held so this snapshot cannot overwrite an
        // Ollama status that landed after we released it.
        let Ok(mut app) = lock(shared) else {
            return;
        };
        apply_fetch_report(&mut app, &report);
        let state = app.public_state();
        emit_status(target, &state.status);
        emit_state(target, &state);
    }
    // Build the recognizer and voices off this thread so Ollama polling is not delayed.
    std::thread::spawn(move || speech::preload(&models));
}

/// Checks the default input device once, without opening a stream.
fn probe_mic<R: Runtime>(target: &impl Emitter<R>, shared: &SharedApp) {
    let allowed = speech::probe_microphone() == speech::MicProbe::Ready;
    let state = match lock(shared) {
        Ok(mut app) => {
            app.mark_mic(allowed);
            if !allowed {
                app.set_status(STATUS_MIC);
            }
            app.public_state()
        }
        Err(_) => return,
    };
    if !allowed {
        emit_status(target, &state.status);
    }
    emit_state(target, &state);
}

/// Asks the model for three short next lines. They are not spoken or saved.
fn queue_suggestions<R: Runtime, W: Emitter<R> + Clone + Send + 'static>(
    window: &W,
    shared: &SharedApp,
    force: bool,
) {
    let job = {
        let Ok(mut app) = lock(shared) else {
            return;
        };
        app.take_suggestion_job(force)
    };
    let Some((epoch, model, messages)) = job else {
        return;
    };
    let window = window.clone();
    let shared = shared.clone();
    std::thread::spawn(move || {
        let text = OllamaClient::new(OLLAMA_BASE)
            .complete(&model, &messages)
            .unwrap_or_default();
        let lines = lumi_core::suggestions::parse_suggestions(&text);
        let Ok(mut app) = lock(&shared) else {
            return;
        };
        if app.offer_suggestions(epoch, lines) {
            emit_state(&window, &app.public_state());
        }
    });
}

/// Asks Ollama for its models, starting `ollama serve` once if it is not up.
fn poll_ollama<R: Runtime, W: Emitter<R> + Clone + Send + 'static>(
    target: &W,
    shared: &SharedApp,
) {
    let client = OllamaClient::new(OLLAMA_BASE);
    let mut result = client.tags();
    if result.is_err() {
        let mut command = ollama_serve_command();
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let _ = command.spawn();
        let started = Instant::now();
        while result.is_err() && started.elapsed() < OLLAMA_WAIT {
            std::thread::sleep(OLLAMA_POLL);
            result = client.tags();
        }
    }
    {
        let Ok(mut app) = lock(shared) else {
            return;
        };
        apply_tag_result(&mut app, result);
        let state = app.public_state();
        emit_status(target, &state.status);
        emit_state(target, &state);
    }
    queue_suggestions(target, shared, false);
}

fn save_current(home: &Path, app: &App) -> Result<(), String> {
    save_settings(&settings_path(home), &app.public_state().settings)
        .map_err(|error| error.to_string())
}

fn save_people(home: &Path, app: &App) -> Result<(), String> {
    let memories = app.memories();
    save_memories(&memories_path(home), &memories).map_err(|error| error.to_string())?;
    let _ = sync_backup(&app.public_state().settings.backup_dir, &memories);
    Ok(())
}

#[tauri::command]
fn get_state(app: tauri::State<'_, SharedApp>) -> Result<PublicState, String> {
    Ok(lock(&app)?.public_state())
}

#[tauri::command]
async fn send_message(
    window: tauri::Window,
    app: tauri::State<'_, SharedApp>,
    recorder: tauri::State<'_, SharedRecorder>,
    text: String,
) -> Result<PublicState, String> {
    let shared: SharedApp = app.inner().clone();
    let recorder: SharedRecorder = recorder.inner().clone();
    // A typed message ends the open capture. The conversation stays open and
    // listens again after this reply.
    drop(cancel_capture(&recorder));
    tauri::async_runtime::spawn_blocking(move || run_message(&window, &shared, &recorder, &text))
        .await
        .map_err(|error| error.to_string())?
}

fn run_message<R: Runtime, W: Emitter<R> + Clone + Send + 'static>(
    window: &W,
    shared: &SharedApp,
    recorder: &SharedRecorder,
    text: &str,
) -> Result<PublicState, String> {
    let prepared = {
        let mut app = lock(shared)?;
        match app.prepare_turn(text) {
            Ok(prepared) => prepared,
            Err(()) => return Ok(app.public_state()),
        }
    };
    let (model, messages, cancel) = prepared;
    emit_state(window, &lock(shared)?.public_state());

    let client = OllamaClient::new(OLLAMA_BASE);
    let streamed =
        lumi_core::turn::stream_reply(&client, &model, &messages, &cancel, &mut |token| {
            let still_current = match lock(shared) {
                Ok(mut app) => {
                    app.push_turn_token(&cancel, token);
                    Arc::ptr_eq(&cancel, &app.cancel_flag()) && !cancel.load(Ordering::SeqCst)
                }
                Err(_) => false,
            };
            if still_current {
                let _ = window.emit("token", token);
            } else if let Ok(app) = lock(shared) {
                emit_state(window, &app.public_state());
            }
        });

    let spoken_text = streamed.text.clone();
    let (state, current, voice) = {
        let mut app = lock(shared)?;
        let current = app.finish_stream(&cancel, streamed.text, streamed.result);
        if current && app.public_state().pose != Pose::Speaking {
            app.finish_pet_reply();
        }
        let voice = app.resolved_voice();
        let _ = save_people(&home_dir(), &app);
        (app.public_state(), current, voice)
    };
    if !current {
        // Superseded: publish the kept partial reply, but leave pose and status alone.
        emit_state(window, &state);
        return Ok(state);
    }
    queue_suggestions(window, shared, true);
    let _ = window.emit("turn_done", ());
    if state.pose == Pose::Speaking {
        let voice = if state.settings.language == Language::Tamil
            || state.settings.personality == Personality::Calm
        {
            voice.unwrap_or(VoiceId::Bella)
        } else {
            let Some(voice) = voice else {
                speech_failed(window, shared, recorder, &cancel);
                return Ok(lock(shared)?.public_state());
            };
            voice
        };
        let _ = window.emit("pose", "speaking");
        // Playback runs on its own thread so this command returns at once and a
        // new message or Stop can cut the voice off. When it finishes, listening
        // starts again if the conversation is still open.
        let window = window.clone();
        let shared_for_voice = shared.clone();
        let recorder_for_voice = recorder.clone();
        let personality = state.settings.personality;
        let language = state.settings.language;
        std::thread::spawn(move || {
            play_reply(
                &window,
                &shared_for_voice,
                &recorder_for_voice,
                &cancel,
                voice,
                personality,
                language,
                &spoken_text,
            );
        });
        return Ok(lock(shared)?.public_state());
    }
    let _ = window.emit("status", &state.status);
    let _ = window.emit("pose", "idle");
    emit_state(window, &state);
    maybe_resume(window, shared, recorder);
    Ok(lock(shared)?.public_state())
}

/// Runs `change` only if the turn that owns `cancel` is still the current one:
/// its flag is clear and it is still the App's cancel flag. The check and the
/// change happen under one lock, so a newer turn cannot slip in between.
fn if_turn_current(
    shared: &SharedApp,
    cancel: &Arc<AtomicBool>,
    change: impl FnOnce(&mut App),
) -> bool {
    let Ok(mut app) = lock(shared) else {
        return false;
    };
    if cancel.load(Ordering::SeqCst) || !Arc::ptr_eq(cancel, &app.cancel_flag()) {
        return false;
    }
    change(&mut app);
    true
}

/// The voice could not be produced: say so and go idle, unless a newer turn
/// has taken over.
fn speech_failed<R: Runtime>(
    window: &(impl Emitter<R> + Clone + Send + 'static),
    shared: &SharedApp,
    recorder: &SharedRecorder,
    cancel: &Arc<AtomicBool>,
) {
    let current = if_turn_current(shared, cancel, |app| {
        app.set_status(STATUS_COULD_NOT_SPEAK);
        app.finish_pet_reply();
        app.finish_playback();
        emit_state(window, &app.public_state());
        let _ = window.emit("status", STATUS_COULD_NOT_SPEAK);
        let _ = window.emit("pose", "idle");
    });
    if current {
        maybe_resume(window, shared, recorder);
    }
}

/// Speaks `text` and plays it, moving the mouth every 50 ms. The App mutex is
/// not held while the voice is generated or played. A turn that is cancelled
/// or replaced (by a new message or recording) stops the sound and leaves pose
/// and status to the turn that replaced it.
fn play_reply<R: Runtime>(
    window: &(impl Emitter<R> + Clone + Send + 'static),
    shared: &SharedApp,
    recorder: &SharedRecorder,
    cancel: &Arc<AtomicBool>,
    voice: VoiceId,
    personality: Personality,
    language: Language,
    text: &str,
) {
    let models = app_support_dir(&home_dir()).join("models");
    // The small built-in voice speaks when a better one is missing, so a reply is
    // never silent.
    let kokoro = || {
        let beats = spoken_beats(personality, text);
        speech::speak(&models, voice, speech_rate(personality), &beats, cancel)
            .map(|audio| (audio, None))
    };
    // A whole piece of audio, plus (for the Phonon voice) the pieces still coming.
    let spoken: Result<(speech::Audio, Option<speech::MoreAudio>), String> =
        if language == Language::Tamil && spoken_in_tamil(text) {
            let tamil = TamilVoice::for_personality(personality);
            speech::speak_tamil(&models, text, tamil, cancel).map(|audio| (audio, None))
        } else if personality == Personality::Calm {
            match speech::speak_robot(text, cancel) {
                Ok(audio) => Ok((audio, None)),
                Err(_) if cancel.load(Ordering::SeqCst) => Err("The voice was cancelled.".into()),
                Err(_) => kokoro(),
            }
        } else {
            // The Phonon voice first.
            match speech::speak_human(&natural_speech(text), voice, cancel) {
                Ok((first, more)) => Ok((first, Some(more))),
                Err(_) if cancel.load(Ordering::SeqCst) => Err("The voice was cancelled.".into()),
                Err(_) => kokoro(),
            }
        };
    if cancel.load(Ordering::SeqCst) {
        return;
    }
    let ((samples, sample_rate), mut more) = match spoken {
        Ok(((samples, rate), more)) if !samples.is_empty() && rate > 0 => {
            ((samples, rate), more)
        }
        _ => return speech_failed(window, shared, recorder, cancel),
    };
    let Ok(output) = rodio::DeviceSinkBuilder::open_default_sink() else {
        return speech_failed(window, shared, recorder, cancel);
    };
    let (Some(channels), Some(rate)) = (NonZero::new(1u16), NonZero::new(sample_rate)) else {
        return speech_failed(window, shared, recorder, cancel);
    };
    let frame_len = (sample_rate / 20) as usize;
    let mut levels = frame_mouth_levels(&samples, frame_len);
    let player = rodio::Player::connect_new(output.mixer());
    player.append(SamplesBuffer::new(channels, rate, samples));
    let mut frame = 0usize;
    loop {
        std::thread::sleep(PLAYBACK_TICK);
        if cancel.load(Ordering::SeqCst) {
            player.stop();
            let _ = window.emit("mouth", 0u8);
            return;
        }
        // Later pieces join the queue as soon as they are voiced.
        while let Some(receiver) = more.as_ref() {
            match receiver.try_recv() {
                Ok(Ok((next, next_rate))) if next_rate == sample_rate && !next.is_empty() => {
                    levels.extend(frame_mouth_levels(&next, frame_len));
                    player.append(SamplesBuffer::new(channels, rate, next));
                }
                Ok(Ok(_)) => {}
                Ok(Err(_)) | Err(mpsc::TryRecvError::Disconnected) => more = None,
                Err(mpsc::TryRecvError::Empty) => break,
            }
        }
        if player.empty() {
            if more.is_none() {
                break;
            }
            // The next piece is still being voiced: the mouth rests.
            let _ = window.emit("mouth", 0u8);
            continue;
        }
        let _ = window.emit("mouth", levels.get(frame).copied().unwrap_or(0));
        frame += 1;
    }
    // Let the device drain its last buffer before the output is dropped.
    std::thread::sleep(PLAYBACK_TICK * 3);
    let finished = if_turn_current(shared, cancel, |app| {
        app.finish_pet_reply();
        app.finish_playback();
        emit_state(window, &app.public_state());
        let _ = window.emit("mouth", 0u8);
        let _ = window.emit("pose", "idle");
    });
    if !finished {
        // Cancelled or replaced during the drain: leave pose and status alone.
        player.stop();
        return;
    }
    maybe_resume(window, shared, recorder);
}

/// After a reply, wait a moment so the speaker is not heard as the next turn,
/// then listen again while the conversation is still open.
fn maybe_resume<R: Runtime, W: Emitter<R> + Clone + Send + 'static>(
    window: &W,
    shared: &SharedApp,
    recorder: &SharedRecorder,
) {
    let window = window.clone();
    let shared = shared.clone();
    let recorder = recorder.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        let resume = lock(&shared)
            .map(|mut app| app.can_resume_conversation())
            .unwrap_or(false);
        if !resume {
            return;
        }
        if let Ok(state) = open_microphone(&window, &shared, &recorder, true) {
            emit_state(&window, &state);
        }
    });
}

/// Opens the microphone for a hands-free conversation. `resume` continues one
/// that is already open. A one-second pause after speech transcribes and replies.
fn open_microphone<R: Runtime, W: Emitter<R> + Clone + Send + 'static>(
    window: &W,
    shared: &SharedApp,
    recorder: &SharedRecorder,
    resume: bool,
) -> Result<PublicState, String> {
    if speech::microphone_denied() {
        let mut app = lock(shared)?;
        app.mark_mic(false);
        app.end_conversation();
        app.set_status(STATUS_MIC);
        let state = app.public_state();
        drop(app);
        emit_status(window, &state.status);
        emit_state(window, &state);
        return Ok(state);
    }
    // Replace any earlier capture and note which start this is.
    let mine = {
        let mut state = lock_recorder(recorder)?;
        state.generation += 1;
        state.recording = None;
        state.generation
    };
    {
        let mut app = lock(shared)?;
        if resume {
            app.resume_listen()?;
        } else {
            app.begin_listen()?;
        }
    }
    let window_for_turn = window.clone();
    let shared_for_turn = shared.clone();
    let recorder_for_turn = recorder.clone();
    // Opening the microphone can take a moment, so no mutex is held.
    // A pause of more than a second, after the person has spoken, runs the turn.
    let opened = speech::start_capture(move || {
        let _ = run_stop(
            &window_for_turn,
            &shared_for_turn,
            &recorder_for_turn,
            Some(mine),
        );
    });
    // Still the same start, and still listening? Stop or a new message cancels
    // this open, and the device must then be closed.
    let mut state = lock_recorder(recorder)?;
    let mut app = lock(shared)?;
    let wanted = state.generation == mine && app.public_state().recording;
    if speech::microphone_denied() {
        drop(opened);
        if state.generation == mine {
            state.generation = state.generation.saturating_add(1);
        }
        drop(state);
        if wanted {
            app.mark_mic(false);
            app.end_conversation();
            app.set_status(STATUS_MIC);
        }
        let current = app.public_state();
        drop(app);
        if wanted {
            emit_status(window, &current.status);
            emit_state(window, &current);
        }
        return Ok(current);
    }
    match opened {
        Ok(recording) => {
            if wanted {
                state.recording = Some(recording);
            }
            // Otherwise `recording` drops here and closes the device.
            Ok(app.public_state())
        }
        Err(error) => {
            if wanted {
                if let speech::CaptureError::Failed(reason) = &error {
                    eprintln!("microphone could not be opened: {reason}");
                    app.mark_mic(false);
                }
                app.end_conversation();
                app.set_status(STATUS_MIC);
            }
            let current = app.public_state();
            drop(app);
            drop(state);
            if wanted {
                emit_status(window, &current.status);
                emit_state(window, &current);
            }
            Ok(current)
        }
    }
}

#[tauri::command]
async fn start_listening(
    window: tauri::Window,
    app: tauri::State<'_, SharedApp>,
    recorder: tauri::State<'_, SharedRecorder>,
) -> Result<PublicState, String> {
    let shared: SharedApp = app.inner().clone();
    let recorder: SharedRecorder = recorder.inner().clone();
    tauri::async_runtime::spawn_blocking(move || open_microphone(&window, &shared, &recorder, false))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn stop_listening(
    window: tauri::Window,
    app: tauri::State<'_, SharedApp>,
    recorder: tauri::State<'_, SharedRecorder>,
) -> Result<PublicState, String> {
    let shared: SharedApp = app.inner().clone();
    let recorder: SharedRecorder = recorder.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        {
            let mut app = lock(&shared)?;
            app.end_conversation();
        }
        drop(cancel_capture(&recorder));
        let state = lock(&shared)?.public_state();
        let _ = window.emit("pose", "idle");
        let _ = window.emit("mouth", 0u8);
        emit_state(&window, &state);
        Ok(state)
    })
    .await
    .map_err(|error| error.to_string())?
}

/// Turns a finished utterance into text and sends it like a typed message.
/// The conversation stays open. The App mutex is not held while the audio is processed.
fn run_stop<R: Runtime, W: Emitter<R> + Clone + Send + 'static>(
    window: &W,
    shared: &SharedApp,
    recorder: &SharedRecorder,
    expected_generation: Option<u64>,
) -> Result<PublicState, String> {
    // `Some(generation)` is the 30-second cap. If the user already cancelled
    // that capture, the counter no longer matches and this stop does nothing.
    let recording = {
        let mut state = lock_recorder(recorder)?;
        if expected_generation.is_some_and(|expected| state.generation != expected) {
            drop(state);
            return Ok(lock(shared)?.public_state());
        }
        state.generation += 1;
        state.recording.take()
    };
    if !lock(shared)?.conversing() {
        return Ok(lock(shared)?.public_state());
    }
    let (state, listen_turn) = {
        let mut app = lock(shared)?;
        app.catch_utterance();
        (app.public_state(), app.cancel_flag())
    };
    emit_state(window, &state);
    let Some(recording) = recording else {
        maybe_resume(window, shared, recorder);
        return Ok(state);
    };
    // Permission denied is not silence.
    if speech::microphone_denied() {
        drop(recording);
        let mut announced = None;
        if_turn_current(shared, &listen_turn, |app| {
            app.mark_mic(false);
            app.end_conversation();
            app.set_status(STATUS_MIC);
            announced = Some(STATUS_MIC.to_string());
        });
        let state = lock(shared)?.public_state();
        if let Some(status) = announced {
            emit_status(window, &status);
            let _ = window.emit("pose", "idle");
            emit_state(window, &state);
        }
        return Ok(state);
    }
    let (samples, sample_rate) = recording.finish();
    let clipped = take_max_duration(&samples, sample_rate, MAX_LISTEN_SECS);
    let samples_16k = resample_linear(clipped, sample_rate, WHISPER_RATE);
    let models = app_support_dir(&home_dir()).join("models");
    let language = state.settings.language;
    if !lock(shared)?.conversing() {
        let mut app = lock(shared)?;
        if !app.conversing() {
            app.end_conversation();
        }
        let state = app.public_state();
        emit_state(window, &state);
        return Ok(state);
    }
    match speech::transcribe(&models, &samples_16k, language) {
        Ok(text) if !text.trim().is_empty() => run_message(window, shared, recorder, text.trim()),
        heard => {
            // Nothing heard: listen again. A recognizer failure ends the conversation.
            let failure = heard.err();
            let mut announced = None;
            let mut resume = false;
            if_turn_current(shared, &listen_turn, |app| match &failure {
                None if app.conversing() => resume = true,
                None => {
                    app.hear_nothing();
                    announced = Some(STATUS_EMPTY_HEARING.to_string());
                }
                Some(message) => {
                    app.end_conversation();
                    app.set_status(message.clone());
                    announced = Some(message.clone());
                }
            });
            let state = lock(shared)?.public_state();
            if let Some(status) = announced {
                emit_status(window, &status);
                let _ = window.emit("pose", "idle");
                emit_state(window, &state);
            }
            if resume {
                maybe_resume(window, shared, recorder);
            }
            Ok(state)
        }
    }
}

#[tauri::command]
async fn set_personality(
    window: tauri::Window,
    app: tauri::State<'_, SharedApp>,
    recorder: tauri::State<'_, SharedRecorder>,
    personality: Personality,
) -> Result<PublicState, String> {
    let shared: SharedApp = app.inner().clone();
    let recorder: SharedRecorder = recorder.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        // Drop any open capture before the switch, so words meant for one
        // person are not transcribed as the next person's turn.
        drop(cancel_capture(&recorder));
        let switched = {
            let mut app = lock(&shared)?;
            let switched = app.set_personality(personality);
            save_current(&home_dir(), &app)?;
            save_people(&home_dir(), &app)?;
            switched
        };
        maybe_resume(&window, &shared, &recorder);
        let state = lock(&shared)?.public_state();
        emit_state(&window, &state);
        if switched {
            queue_suggestions(&window, &shared, true);
        }
        Ok(state)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
fn set_model(app: tauri::State<'_, SharedApp>, model: String) -> Result<PublicState, String> {
    let mut app = lock(&app)?;
    app.set_model(model);
    save_current(&home_dir(), &app)?;
    Ok(app.public_state())
}

#[tauri::command]
fn set_language(
    app: tauri::State<'_, SharedApp>,
    language: Language,
) -> Result<PublicState, String> {
    let mut app = lock(&app)?;
    app.set_language(language);
    save_current(&home_dir(), &app)?;
    Ok(app.public_state())
}

#[tauri::command]
async fn pick_backup_folder(app: tauri::State<'_, SharedApp>) -> Result<PublicState, String> {
    let start = backup_picker_start(&home_dir());
    let chosen = rfd::AsyncFileDialog::new()
        .set_title("Choose a folder for Lumi backups")
        .set_directory(start)
        .pick_folder()
        .await;
    let Some(folder) = chosen else {
        return Ok(lock(&app)?.public_state());
    };
    let mut locked = lock(&app)?;
    locked.set_backup_dir(folder.path().to_string_lossy().into_owned());
    save_current(&home_dir(), &locked)?;
    save_people(&home_dir(), &locked)?;
    Ok(locked.public_state())
}

#[tauri::command]
fn set_voice(app: tauri::State<'_, SharedApp>, voice: VoiceId) -> Result<PublicState, String> {
    let mut app = lock(&app)?;
    app.set_voice(voice)?;
    save_current(&home_dir(), &app)?;
    Ok(app.public_state())
}

#[tauri::command]
async fn retry_download(
    window: tauri::Window,
    app: tauri::State<'_, SharedApp>,
) -> Result<PublicState, String> {
    let shared: SharedApp = app.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        fetch_packs(&window, &shared, &home_dir());
        Ok(lock(&shared)?.public_state())
    })
    .await
    .map_err(|error| error.to_string())?
}

pub fn run() {
    let home = home_dir();
    let settings = load_settings(&settings_path(&home));
    let memories = load_memories(&memories_path(&home));
    let shared: SharedApp = Arc::new(Mutex::new(App::with_memories(settings, memories)));
    let startup = shared.clone();
    let mic_probe = shared.clone();
    let recorder: SharedRecorder = Arc::new(Mutex::new(RecorderState::default()));

    tauri::Builder::default()
        .manage(shared)
        .manage(recorder)
        .setup(move |app| {
            let handle = app.handle().clone();
            let mic_handle = handle.clone();
            std::thread::spawn(move || probe_mic(&mic_handle, &mic_probe));
            let ollama_window = handle.clone();
            let ollama_app = startup.clone();
            // Ollama starts at once. A model download must not delay typing.
            std::thread::spawn(move || poll_ollama(&ollama_window, &ollama_app));
            std::thread::spawn(move || fetch_packs(&handle, &startup, &home_dir()));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            send_message,
            start_listening,
            stop_listening,
            set_personality,
            set_model,
            set_voice,
            set_language,
            pick_backup_folder,
            retry_download
        ])
        .run(tauri::generate_context!())
        .expect("error while running Lumi");
}
