use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample};
use lumi_core::audio::{rms, SpeechPause, TurnListener};
use lumi_core::ollama::OLLAMA_BASE;
use lumi_core::phonon::{speech_chunks, VoiceClient};
use lumi_core::settings::{Language, VoiceId};
use lumi_core::speech_text::SpokenBeat;
use lumi_core::tamil_listen;
use lumi_core::tamil_voice::{self, TamilVoice};
use sherpa_onnx::{
    GenerationConfig, OfflineNemoEncDecCtcModelConfig, OfflineRecognizer, OfflineRecognizerConfig,
    OfflineTts, OfflineTtsConfig, OfflineTtsKokoroModelConfig, OfflineTtsVitsModelConfig,
    OfflineWhisperModelConfig,
};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::Duration;

// One recognizer and one engine per voice, built on first use and then kept, so
// a voice change never rebuilds a model. A model whose files are missing stays
// unloaded (`None`) and is tried again on the next call.
static RECOGNIZER: Mutex<Option<OfflineRecognizer>> = Mutex::new(None);
static TAMIL_RECOGNIZER: Mutex<Option<OfflineRecognizer>> = Mutex::new(None);

struct VoiceSlot {
    tts: Option<OfflineTts>,
    /// True while a generation has taken the engine out of the slot.
    busy: bool,
}

static KOKORO: Mutex<VoiceSlot> = Mutex::new(VoiceSlot {
    tts: None,
    busy: false,
});

fn guard<T>(slot: &Mutex<T>) -> MutexGuard<'_, T> {
    slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn path_string(path: std::path::PathBuf) -> String {
    path.display().to_string()
}

fn build_recognizer(models_dir: &Path) -> Result<OfflineRecognizer, String> {
    let root = models_dir.join("sherpa-onnx-whisper-small.en");
    if !lumi_core::packs::onnx_is_complete(&root.join("small.en-encoder.int8.onnx"))
        || !lumi_core::packs::onnx_is_complete(&root.join("small.en-decoder.int8.onnx"))
    {
        return Err("Whisper could not be loaded.".to_string());
    }
    let mut config = OfflineRecognizerConfig::default();
    config.model_config.whisper = OfflineWhisperModelConfig {
        encoder: Some(path_string(root.join("small.en-encoder.int8.onnx"))),
        decoder: Some(path_string(root.join("small.en-decoder.int8.onnx"))),
        language: Some("en".into()),
        task: Some("transcribe".into()),
        tail_paddings: 800,
        enable_token_timestamps: false,
        enable_segment_timestamps: false,
    };
    config.model_config.tokens = Some(path_string(root.join("small.en-tokens.txt")));
    config.model_config.num_threads = 2;
    OfflineRecognizer::create(&config).ok_or_else(|| "Whisper could not be loaded.".to_string())
}

/// AI4Bharat IndicConformer (Tamil-only CTC). The earlier omnilingual model
/// was a 1,600-language generalist and missed too many Tamil words.
fn build_tamil_recognizer(models_dir: &Path) -> Result<OfflineRecognizer, String> {
    if !tamil_listen::is_ready(models_dir) {
        return Err("Tamil is still loading.".to_string());
    }
    let mut config = OfflineRecognizerConfig::default();
    config.model_config.nemo_ctc = OfflineNemoEncDecCtcModelConfig {
        model: Some(path_string(tamil_listen::model_path(models_dir))),
    };
    config.model_config.tokens = Some(path_string(tamil_listen::tokens_path(models_dir)));
    config.model_config.model_type = Some("nemo_ctc".into());
    config.model_config.num_threads = 2;
    config.decoding_method = Some("greedy_search".into());
    OfflineRecognizer::create(&config).ok_or_else(|| "Tamil is still loading.".to_string())
}

fn speaker_id(voice: VoiceId) -> i32 {
    match voice {
        VoiceId::Bella => 1,
        VoiceId::Sky => 4,
        VoiceId::Emma => 7,
        VoiceId::Adam => 5,
        VoiceId::George => 9,
    }
}

fn kokoro_ready(models_dir: &Path) -> bool {
    let root = models_dir.join("kokoro-en-v0_19");
    root.join("model.onnx").is_file()
        && root.join("voices.bin").is_file()
        && root.join("tokens.txt").is_file()
}

fn build_kokoro(models_dir: &Path) -> Result<OfflineTts, String> {
    let root = models_dir.join("kokoro-en-v0_19");
    let config = OfflineTtsConfig {
        model: sherpa_onnx::OfflineTtsModelConfig {
            kokoro: OfflineTtsKokoroModelConfig {
                model: Some(path_string(root.join("model.onnx"))),
                voices: Some(path_string(root.join("voices.bin"))),
                tokens: Some(path_string(root.join("tokens.txt"))),
                data_dir: Some(path_string(root.join("espeak-ng-data"))),
                ..Default::default()
            },
            num_threads: 2,
            ..Default::default()
        },
        ..Default::default()
    };
    OfflineTts::create(&config).ok_or_else(|| "The voice could not be loaded.".to_string())
}

/// Loads every model whose files are on disk and that is not loaded yet.
pub fn preload(models_dir: &Path) {
    {
        let mut slot = guard(&RECOGNIZER);
        if slot.is_none() && models_dir.join("sherpa-onnx-whisper-small.en").is_dir() {
            *slot = build_recognizer(models_dir).ok();
        }
    }
    {
        let mut slot = guard(&TAMIL_RECOGNIZER);
        if slot.is_none() && tamil_listen::is_ready(models_dir) {
            *slot = build_tamil_recognizer(models_dir).ok();
        }
    }
    {
        let mut slot = guard(&KOKORO);
        if !slot.busy && slot.tts.is_none() && kokoro_ready(models_dir) {
            slot.tts = build_kokoro(models_dir).ok();
        }
    }
}

pub fn transcribe(
    models_dir: &Path,
    samples_16k: &[f32],
    language: Language,
) -> Result<String, String> {
    if language == Language::Tamil {
        return transcribe_tamil(models_dir, samples_16k);
    }
    let mut slot = guard(&RECOGNIZER);
    if slot.is_none() {
        *slot = Some(build_recognizer(models_dir)?);
    }
    let recognizer = slot.as_ref().ok_or("Whisper is not loaded.")?;
    let stream = recognizer.create_stream();
    stream.accept_waveform(16_000, samples_16k);
    recognizer.decode(&stream);
    Ok(stream
        .get_result()
        .map(|result| result.text)
        .unwrap_or_default())
}

fn transcribe_tamil(models_dir: &Path, samples_16k: &[f32]) -> Result<String, String> {
    let mut slot = guard(&TAMIL_RECOGNIZER);
    if slot.is_none() {
        *slot = Some(build_tamil_recognizer(models_dir)?);
    }
    let recognizer = slot.as_ref().ok_or("Tamil is still loading.")?;
    // CTC drops the first and last phones without a little quiet on each side.
    let pad = vec![0.0; 16_000 / 4];
    let mut padded = pad.clone();
    padded.extend_from_slice(samples_16k);
    padded.extend_from_slice(&pad);
    let stream = recognizer.create_stream();
    stream.accept_waveform(16_000, &padded);
    recognizer.decode(&stream);
    Ok(stream
        .get_result()
        .map(|result| result.text)
        .unwrap_or_default())
}

/// Tamil voices, built on first use and then kept.
static TAMIL_VOICES: Mutex<Vec<(TamilVoice, OfflineTts)>> = Mutex::new(Vec::new());

fn build_tamil_voice(models_dir: &Path, voice: TamilVoice) -> Result<OfflineTts, String> {
    let config = OfflineTtsConfig {
        model: sherpa_onnx::OfflineTtsModelConfig {
            vits: OfflineTtsVitsModelConfig {
                model: Some(path_string(tamil_voice::model_path(models_dir, voice))),
                tokens: Some(path_string(tamil_voice::tokens_path(models_dir, voice))),
                // The espeak-ng data that comes with Kokoro knows Tamil too.
                data_dir: Some(path_string(
                    models_dir.join("kokoro-en-v0_19").join("espeak-ng-data"),
                )),
                noise_scale: 0.667,
                noise_scale_w: 0.8,
                length_scale: 1.0,
                ..Default::default()
            },
            num_threads: 2,
            ..Default::default()
        },
        ..Default::default()
    };
    OfflineTts::create(&config).ok_or_else(|| "The Tamil voice could not be loaded.".to_string())
}

/// Speaks Tamil with this character's Piper voice. If the voice is not installed
/// (or cannot load), eSpeak NG speaks it instead so the reply is not silent.
pub fn speak_tamil(
    models_dir: &Path,
    text: &str,
    voice: TamilVoice,
    cancel: &Arc<AtomicBool>,
) -> Result<(Vec<f32>, u32), String> {
    if tamil_voice::is_ready(models_dir, voice) {
        match speak_piper(models_dir, text, voice, cancel) {
            Ok(audio) => return Ok(audio),
            Err(_) if cancel.load(Ordering::SeqCst) => {
                return Err("The voice was cancelled.".into())
            }
            Err(_) => {}
        }
    }
    speak_espeak(
        "ta",
        150,
        text,
        cancel,
        "The Tamil voice could not speak.",
    )
}

fn speak_piper(
    models_dir: &Path,
    text: &str,
    voice: TamilVoice,
    cancel: &Arc<AtomicBool>,
) -> Result<(Vec<f32>, u32), String> {
    let mut voices = guard(&TAMIL_VOICES);
    if !voices.iter().any(|(known, _)| *known == voice) {
        voices.push((voice, build_tamil_voice(models_dir, voice)?));
    }
    let tts = voices
        .iter()
        .find(|(known, _)| *known == voice)
        .map(|(_, tts)| tts)
        .ok_or("The Tamil voice is not loaded.")?;
    let stop = Arc::clone(cancel);
    let audio = tts
        .generate_with_config(
            text,
            &GenerationConfig {
                sid: 0,
                speed: 1.0,
                ..Default::default()
            },
            Some(move |_samples: &[f32], _progress: f32| !stop.load(Ordering::SeqCst)),
        )
        .ok_or("The Tamil voice could not speak.")?;
    let rate = audio.sample_rate() as u32;
    let samples = audio.samples().to_vec();
    if samples.is_empty() || rate == 0 {
        return Err("The Tamil voice could not speak.".into());
    }
    Ok((samples, rate))
}

/// One voiced piece of a reply.
pub type Audio = (Vec<f32>, u32);

/// The pieces after the first, arriving while the first is already playing.
pub type MoreAudio = mpsc::Receiver<Result<Audio, String>>;

/// Speaks with Ollama's Phonon voice, one sentence group at a time. Fails at
/// once when the voice is not ready, or when the first piece fails, so the
/// caller can use the small built-in voice instead. Returns the first piece
/// and a receiver for the rest, so playback starts long before the reply is
/// fully voiced. A cancel stops the wait and the remaining work.
pub fn speak_human(
    text: &str,
    voice: VoiceId,
    cancel: &Arc<AtomicBool>,
) -> Result<(Audio, MoreAudio), String> {
    let client = VoiceClient::new(OLLAMA_BASE);
    if !client.ready() {
        return Err("The Phonon voice is not installed.".into());
    }
    let pieces = speech_chunks(text, 600);
    if pieces.is_empty() {
        return Err("There was nothing to say.".into());
    }
    let (send, receive) = mpsc::channel();
    let stop = cancel.clone();
    std::thread::spawn(move || {
        for piece in pieces {
            if stop.load(Ordering::SeqCst) {
                return;
            }
            let result = client.speak(&piece, voice);
            let failed = result.is_err();
            if send.send(result).is_err() || failed {
                return;
            }
        }
    });
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err("The voice was cancelled.".into());
        }
        match receive.recv_timeout(Duration::from_millis(30)) {
            Ok(Ok(first)) => return Ok((first, receive)),
            Ok(Err(error)) => return Err(error),
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err("The Phonon voice stopped.".into())
            }
        }
    }
}

/// Speaks Pip with eSpeak NG, a small formant synthesizer that sounds like a robot
/// and runs on macOS and Linux. The Robosoft variant is the smoothed robot voice;
/// the plain voice sounded choppy.
pub fn speak_robot(text: &str, cancel: &Arc<AtomicBool>) -> Result<(Vec<f32>, u32), String> {
    speak_espeak("en-us+robosoft", 140, text, cancel, "She could not speak.")
}

/// Where eSpeak NG lives. A launched app may not have Homebrew on its PATH.
fn espeak_binary() -> std::path::PathBuf {
    ["/opt/homebrew/bin/espeak-ng", "/usr/local/bin/espeak-ng", "/usr/bin/espeak-ng"]
        .iter()
        .map(std::path::PathBuf::from)
        .find(|path| path.is_file())
        .unwrap_or_else(|| std::path::PathBuf::from("espeak-ng"))
}

fn speak_espeak(
    voice: &str,
    words_per_minute: u32,
    text: &str,
    cancel: &Arc<AtomicBool>,
    failure: &str,
) -> Result<(Vec<f32>, u32), String> {
    use std::io::Write;
    let path = std::env::temp_dir().join(format!("lumi-espeak-{}.wav", std::process::id()));
    let mut child = std::process::Command::new(espeak_binary())
        .args(["-v", voice, "-s", &words_per_minute.to_string(), "-w"])
        .arg(&path)
        .arg("--stdin")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|_| "The voice needs eSpeak NG (espeak-ng) to be installed.".to_string())?;
    if let Some(mut input) = child.stdin.take() {
        let _ = input.write_all(text.as_bytes());
    }
    loop {
        if cancel.load(Ordering::SeqCst) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_file(&path);
            return Err("The voice was cancelled.".into());
        }
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(Some(_)) | Err(_) => return Err(failure.to_string()),
            Ok(None) => std::thread::sleep(Duration::from_millis(30)),
        }
    }
    let audio = read_wav_pcm16(&path);
    let _ = std::fs::remove_file(&path);
    audio
}

fn read_wav_pcm16(path: &Path) -> Result<(Vec<f32>, u32), String> {
    let bytes = std::fs::read(path).map_err(|_| "The Tamil voice could not speak.".to_string())?;
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" {
        return Err("The Tamil voice could not speak.".into());
    }
    let mut index = 12;
    let mut rate = 22_050u32;
    while index + 8 <= bytes.len() {
        let id = &bytes[index..index + 4];
        let size = u32::from_le_bytes(bytes[index + 4..index + 8].try_into().unwrap()) as usize;
        let body = index + 8;
        if body > bytes.len() {
            break;
        }
        if id == b"fmt " && body + 8 <= bytes.len() {
            rate = u32::from_le_bytes(bytes[body + 4..body + 8].try_into().unwrap());
        }
        if id == b"data" {
            let end = (body + size).min(bytes.len());
            let mut samples = Vec::with_capacity((end - body) / 2);
            for frame in bytes[body..end].chunks_exact(2) {
                let sample = i16::from_le_bytes([frame[0], frame[1]]);
                samples.push(sample as f32 / 32_768.0);
            }
            if samples.is_empty() || rate == 0 {
                return Err("The Tamil voice could not speak.".into());
            }
            return Ok((samples, rate));
        }
        index = body + size + (size % 2);
    }
    Err("The Tamil voice could not speak.".into())
}

struct RestoreVoice {
    tts: Option<OfflineTts>,
}

impl Drop for RestoreVoice {
    fn drop(&mut self) {
        if let Some(tts) = self.tts.take() {
            let mut slot = guard(&KOKORO);
            slot.tts = Some(tts);
            slot.busy = false;
        }
    }
}

fn take_kokoro(models_dir: &Path, cancel: &AtomicBool) -> Result<OfflineTts, String> {
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err("The voice was cancelled.".into());
        }
        let mut slot = guard(&KOKORO);
        if slot.busy {
            drop(slot);
            std::thread::sleep(Duration::from_millis(20));
            continue;
        }
        if slot.tts.is_none() {
            slot.tts = Some(build_kokoro(models_dir)?);
        }
        slot.busy = true;
        return slot
            .tts
            .take()
            .ok_or_else(|| "The voice is not loaded.".into());
    }
}

pub fn speak(
    models_dir: &Path,
    voice: VoiceId,
    rate: f32,
    beats: &[SpokenBeat],
    cancel: &Arc<AtomicBool>,
) -> Result<(Vec<f32>, u32), String> {
    let tts = take_kokoro(models_dir, cancel)?;
    let restore = RestoreVoice { tts: Some(tts) };
    let mut samples = Vec::new();
    let mut sample_rate = 0u32;
    let mut pending_pause = 0u32;
    for beat in beats {
        if cancel.load(Ordering::SeqCst) {
            drop(restore);
            return Err("The voice was cancelled.".into());
        }
        match beat {
            SpokenBeat::PauseMs(ms) => pending_pause = pending_pause.saturating_add(*ms),
            SpokenBeat::Words(text) => {
                let stop = Arc::clone(cancel);
                let audio = restore.tts.as_ref().and_then(|tts| {
                    tts.generate_with_config(
                        text,
                        &GenerationConfig {
                            sid: speaker_id(voice),
                            speed: rate,
                            // Keep Kokoro's own pauses short. The gap between
                            // sentences is the silence inserted below.
                            silence_scale: 0.35,
                            ..Default::default()
                        },
                        Some(move |_samples: &[f32], _progress: f32| {
                            !stop.load(Ordering::SeqCst)
                        }),
                    )
                });
                match audio {
                    Some(audio) => {
                        let rate_now = audio.sample_rate() as u32;
                        if sample_rate == 0 {
                            sample_rate = rate_now;
                        }
                        if pending_pause > 0 && !samples.is_empty() && rate_now == sample_rate {
                            samples.extend(std::iter::repeat(0.0).take(silence_samples(
                                pending_pause,
                                sample_rate,
                            )));
                        }
                        pending_pause = 0;
                        if rate_now == sample_rate {
                            samples.extend_from_slice(trim_edges(audio.samples(), rate_now));
                        }
                    }
                    None if samples.is_empty() => pending_pause = 0,
                    None => {
                        drop(restore);
                        return Err("The voice could not speak.".into());
                    }
                }
            }
        }
    }
    drop(restore);
    if samples.is_empty() || sample_rate == 0 {
        return Err("The voice could not speak.".into());
    }
    Ok((samples, sample_rate))
}

fn silence_samples(ms: u32, sample_rate: u32) -> usize {
    (u64::from(sample_rate) * u64::from(ms) / 1000) as usize
}

/// Drops the quiet lead-in and tail Kokoro adds, leaving 30 ms so a word is not clipped.
fn trim_edges(samples: &[f32], sample_rate: u32) -> &[f32] {
    const THRESHOLD: f32 = 0.01;
    let pad = sample_rate as usize * 30 / 1000;
    let Some(start) = samples.iter().position(|sample| sample.abs() >= THRESHOLD) else {
        return samples;
    };
    let Some(end) = samples.iter().rposition(|sample| sample.abs() >= THRESHOLD) else {
        return samples;
    };
    let from = start.saturating_sub(pad);
    let to = (end + 1 + pad).min(samples.len());
    &samples[from..to]
}

/// True when the default input device reports a usable config. It never opens a
/// stream, so it does not prompt for permission.
pub fn input_available() -> bool {
    cpal::default_host()
        .default_input_device()
        .is_some_and(|device| device.default_input_config().is_ok())
}

/// A microphone capture running on its own thread (a `cpal` stream is not `Send`).
pub struct Recording {
    samples: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Recording {
    /// Stops the stream and returns the mono samples with their sample rate.
    pub fn finish(mut self) -> (Vec<f32>, u32) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        let samples = std::mem::take(&mut *guard(&self.samples));
        (samples, self.sample_rate)
    }
}

impl Drop for Recording {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

fn input_stream<T>(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    samples: Arc<Mutex<Vec<f32>>>,
    stop: Arc<AtomicBool>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = usize::from(config.channels()).max(1);
    device
        .build_input_stream(
            (*config).into(),
            move |data: &[T], _: &_| {
                if stop.load(Ordering::SeqCst) {
                    return;
                }
                let Ok(mut buffer) = samples.lock() else {
                    return;
                };
                for frame in data.chunks(channels) {
                    let sum: f32 = frame.iter().map(|&s| f32::from_sample(s)).sum();
                    buffer.push(sum / frame.len() as f32);
                }
            },
            |_error| {},
            None,
        )
        .map_err(|error| error.to_string())
}

fn open_input(
    samples: Arc<Mutex<Vec<f32>>>,
    stop: Arc<AtomicBool>,
) -> Result<(cpal::Stream, u32), String> {
    let device = cpal::default_host()
        .default_input_device()
        .ok_or("No input device.")?;
    let config = device
        .default_input_config()
        .map_err(|error| error.to_string())?;
    let stream = match config.sample_format() {
        SampleFormat::I8 => input_stream::<i8>(&device, &config, samples, stop)?,
        SampleFormat::I16 => input_stream::<i16>(&device, &config, samples, stop)?,
        SampleFormat::I32 => input_stream::<i32>(&device, &config, samples, stop)?,
        SampleFormat::U8 => input_stream::<u8>(&device, &config, samples, stop)?,
        SampleFormat::U16 => input_stream::<u16>(&device, &config, samples, stop)?,
        SampleFormat::F32 => input_stream::<f32>(&device, &config, samples, stop)?,
        other => return Err(format!("Unsupported sample format {other}")),
    };
    stream.play().map_err(|error| error.to_string())?;
    Ok((stream, config.sample_rate()))
}

/// Why a capture did not start.
#[derive(Debug)]
pub enum CaptureError {
    /// The device reported an error (for example, no input device).
    Failed(String),
    /// The device did not finish opening in time. The stop flag is set, so a
    /// late open closes its own stream. The worker is not joined.
    TimedOut,
}

/// How long `start_capture` waits for the device to open.
const OPEN_TIMEOUT: Duration = Duration::from_secs(10);

/// Opens the default microphone and records until `finish` is called, or until
/// the person has spoken and then paused for more than a second. That pause
/// closes the stream and runs `on_utterance` on a fresh thread so it can join
/// this worker and transcribe. A user Stop sets only the stop flag, so
/// `on_utterance` does not run. Quiet waiting is dropped and the mic stays open.
pub fn start_capture(
    on_utterance: impl FnOnce() + Send + 'static,
) -> Result<Recording, CaptureError> {
    let samples = Arc::new(Mutex::new(Vec::new()));
    let stop = Arc::new(AtomicBool::new(false));
    let endpoint = Arc::new(AtomicBool::new(false));
    let (ready_tx, ready_rx) = mpsc::channel();
    let worker = {
        let samples = samples.clone();
        let stop = stop.clone();
        let endpoint = endpoint.clone();
        std::thread::spawn(move || {
            let mut on_utterance = Some(on_utterance);
            match open_input(samples.clone(), stop.clone()) {
                Ok((stream, rate)) => {
                    let _ = ready_tx.send(Ok(rate));
                    let mut listener = TurnListener::new(rate);
                    let mut seen = 0usize;
                    while !stop.load(Ordering::SeqCst) {
                        std::thread::sleep(Duration::from_millis(50));
                        let fresh = {
                            let buffer = guard(&samples);
                            if seen > buffer.len() {
                                seen = buffer.len();
                            }
                            let fresh = buffer[seen..].to_vec();
                            seen = buffer.len();
                            fresh
                        };
                        if fresh.is_empty() {
                            continue;
                        }
                        match listener.push(rms(&fresh), fresh.len()) {
                            SpeechPause::Keep => {}
                            SpeechPause::Endpoint => {
                                endpoint.store(true, Ordering::SeqCst);
                                stop.store(true, Ordering::SeqCst);
                            }
                            SpeechPause::ForgetQuiet => {
                                let mut buffer = guard(&samples);
                                let drop_until = seen.min(buffer.len());
                                buffer.drain(..drop_until);
                                seen = 0;
                            }
                        }
                    }
                    drop(stream);
                    if endpoint.load(Ordering::SeqCst) {
                        if let Some(on_utterance) = on_utterance.take() {
                            // Not this thread: `on_utterance` joins the worker.
                            std::thread::spawn(on_utterance);
                        }
                    }
                }
                Err(error) => {
                    let _ = ready_tx.send(Err(error));
                }
            }
        })
    };
    match ready_rx.recv_timeout(OPEN_TIMEOUT) {
        Ok(Ok(sample_rate)) => Ok(Recording {
            samples,
            sample_rate,
            stop,
            worker: Some(worker),
        }),
        Ok(Err(error)) => {
            let _ = worker.join();
            Err(CaptureError::Failed(error))
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            stop.store(true, Ordering::SeqCst);
            Err(CaptureError::TimedOut)
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            Err(CaptureError::Failed("The microphone did not open.".into()))
        }
    }
}

/// What a permission and device check found, without opening a capture stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MicProbe {
    Ready,
    /// macOS microphone permission is denied or restricted.
    Denied,
    Missing,
}

/// True when macOS has denied or restricted microphone access. A stream of
/// silence is not treated as a denial. Other platforms return false.
pub fn microphone_denied() -> bool {
    // 1 = restricted, 2 = denied. Not-determined and authorized stay usable.
    matches!(authorization_status(), Some(1) | Some(2))
}

pub fn probe_microphone() -> MicProbe {
    if microphone_denied() {
        MicProbe::Denied
    } else if input_available() {
        MicProbe::Ready
    } else {
        MicProbe::Missing
    }
}

#[cfg(not(target_os = "macos"))]
fn authorization_status() -> Option<isize> {
    None
}

#[cfg(target_os = "macos")]
fn authorization_status() -> Option<isize> {
    unsafe { av_audio_authorization() }
}

#[cfg(target_os = "macos")]
unsafe fn av_audio_authorization() -> Option<isize> {
    #[allow(improper_ctypes)]
    #[link(name = "objc")]
    extern "C" {
        fn objc_getClass(name: *const i8) -> *mut std::ffi::c_void;
        fn sel_registerName(name: *const i8) -> *mut std::ffi::c_void;
        fn objc_msgSend();
        fn objc_autoreleasePoolPush() -> *mut std::ffi::c_void;
        fn objc_autoreleasePoolPop(pool: *mut std::ffi::c_void);
    }
    #[link(name = "Foundation", kind = "framework")]
    extern "C" {}
    #[link(name = "AVFoundation", kind = "framework")]
    extern "C" {}

    // AVMediaTypeAudio is the four-character code "soun".
    let pool = objc_autoreleasePoolPush();
    let class = objc_getClass(b"AVCaptureDevice\0".as_ptr() as *const i8);
    let strings = objc_getClass(b"NSString\0".as_ptr() as *const i8);
    let status = if class.is_null() || strings.is_null() {
        None
    } else {
        let utf8 = sel_registerName(b"stringWithUTF8String:\0".as_ptr() as *const i8);
        let make: unsafe extern "C" fn(
            *mut std::ffi::c_void,
            *mut std::ffi::c_void,
            *const i8,
        ) -> *mut std::ffi::c_void = std::mem::transmute(objc_msgSend as *const ());
        let media = make(strings, utf8, b"soun\0".as_ptr() as *const i8);
        if media.is_null() {
            None
        } else {
            let sel = sel_registerName(b"authorizationStatusForMediaType:\0".as_ptr() as *const i8);
            let ask: unsafe extern "C" fn(
                *mut std::ffi::c_void,
                *mut std::ffi::c_void,
                *mut std::ffi::c_void,
            ) -> isize = std::mem::transmute(objc_msgSend as *const ());
            Some(ask(class, sel, media))
        }
    };
    objc_autoreleasePoolPop(pool);
    status
}

/// These speak for real, so they need the installed models, eSpeak NG, and the
/// network. Run them on purpose:
/// `cargo test -p lumi-app --lib live_ -- --ignored --nocapture --test-threads=1`
#[cfg(test)]
mod live_tests {
    use super::*;
    use lumi_core::settings::app_support_dir;

    fn models() -> std::path::PathBuf {
        let home = std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap();
        app_support_dir(&home).join("models")
    }

    fn idle() -> Arc<AtomicBool> {
        Arc::new(AtomicBool::new(false))
    }

    fn loud(audio: &(Vec<f32>, u32), name: &str) {
        let (samples, rate) = audio;
        let seconds = samples.len() as f32 / *rate as f32;
        let rms = (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
        println!("{name}: {seconds:.1}s at {rate} Hz, rms {rms:.3}");
        assert!(seconds > 1.5, "{name} was too short");
        assert!(rms > 0.01, "{name} was silent");
        let pcm: Vec<u8> = samples
            .iter()
            .flat_map(|s| ((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())
            .collect();
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + pcm.len() as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&rate.to_le_bytes());
        wav.extend_from_slice(&(rate * 2).to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
        wav.extend_from_slice(&pcm);
        let _ = std::fs::write(std::env::temp_dir().join(format!("lumi-live-{name}.wav")), wav);
    }

    const TAMIL: &str = "வணக்கம்! நான் லூமி. இன்று உங்களுக்கு எப்படி உதவலாம்?";

    #[test]
    #[ignore]
    fn live_each_tamil_voice_installs_and_speaks() {
        let client = lumi_core::app::download_client().unwrap();
        for voice in TamilVoice::ALL {
            tamil_voice::fetch(&models(), voice, &client).unwrap();
            assert!(tamil_voice::is_ready(&models(), voice));
            let started = std::time::Instant::now();
            let audio = speak_piper(&models(), TAMIL, voice, &idle()).unwrap();
            println!("{} took {:?}", voice.label(), started.elapsed());
            loud(&audio, &format!("tamil-{}", voice.label().replace(' ', "")));
        }
    }

    #[test]
    #[ignore]
    fn live_espeak_speaks_the_robot_and_tamil() {
        loud(&speak_robot("Beep boop. Hello, I am Pip, your helper robot.", &idle()).unwrap(), "robot");
        loud(&speak_espeak("ta", 150, TAMIL, &idle(), "failed").unwrap(), "tamil-espeak");
    }

    /// A Tamil voice speaks a sentence and the Tamil recognizer hears it back.
    #[test]
    #[ignore]
    fn live_tamil_speech_round_trips_through_the_recognizer() {
        let (samples, rate) = speak_piper(&models(), TAMIL, TamilVoice::HemaLatha, &idle()).unwrap();
        let samples_16k = lumi_core::audio::resample_linear(&samples, rate, 16_000);
        let heard = transcribe(&models(), &samples_16k, Language::Tamil).unwrap();
        println!("heard: {heard}");
        let tamil_letters = heard.chars().filter(|c| ('\u{0B80}'..='\u{0BFF}').contains(c)).count();
        assert!(tamil_letters >= 20, "expected Tamil text, got {heard:?}");
        // Spelling varies a little, so check for words that came through whole.
        assert!(heard.contains("இன்று"), "got {heard:?}");
    }

    #[test]
    #[ignore]
    fn live_a_cancelled_robot_stops() {
        let cancel = Arc::new(AtomicBool::new(true));
        assert!(speak_robot("Hello there.", &cancel).is_err());
    }
}
