use crate::conversation::{ChatMessage, Conversation, Role};
use crate::pets::PetView;
use crate::model_choice::{choose_model, visible_models};
use crate::settings::{
    character_voice, Language, ListenMode, PersonMemories, Personality, Settings, StoredMessage,
    VoiceId,
};
use crate::turn::{outcome_of, run_turn, ModelClient, ModelError, TurnOutcome};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub const STATUS_OLLAMA_DOWN: &str = "Ollama is unavailable.";
pub const STATUS_NO_MODEL: &str = "No chat model is installed.";
pub const STATUS_MIC: &str = "The microphone is unavailable.";
pub const STATUS_EMPTY_HEARING: &str = "It did not catch anything.";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Pose {
    Idle,
    Listening,
    Thinking,
    Speaking,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PublicMessage {
    pub role: String,
    pub content: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct PublicState {
    pub settings: Settings,
    pub pose: Pose,
    pub mouth: u8,
    pub status: String,
    pub transcript: Vec<PublicMessage>,
    pub bella_loaded: bool,
    pub sky_loaded: bool,
    pub whisper_loaded: bool,
    pub tamil_loaded: bool,
    pub mic_allowed: bool,
    pub models: Vec<String>,
    /// Tag that will actually be called. Empty when no chat model resolves.
    pub resolved_model: String,
    pub recording: bool,
    /// True from Listen until Stop. Stays true while thinking and speaking.
    pub conversing: bool,
    pub pet: Option<PetView>,
    /// Short lines the person can tap. They are not part of the transcript.
    pub suggestions: Vec<String>,
}

#[derive(Clone)]
struct Shelf {
    cheerful: Vec<ChatMessage>,
    calm: Vec<ChatMessage>,
    study: Vec<ChatMessage>,
    dad: Vec<ChatMessage>,
}

impl Shelf {
    fn from_memories(memories: PersonMemories) -> Self {
        Self {
            cheerful: chat_from_stored(&memories.cheerful),
            calm: chat_from_stored(&memories.calm),
            study: chat_from_stored(&memories.study),
            dad: chat_from_stored(&memories.dad),
        }
    }

    fn get(&self, personality: Personality) -> Vec<ChatMessage> {
        match personality {
            Personality::Cheerful => self.cheerful.clone(),
            Personality::Calm => self.calm.clone(),
            Personality::Study => self.study.clone(),
            Personality::Dad => self.dad.clone(),
        }
    }

    fn set(&mut self, personality: Personality, messages: Vec<ChatMessage>) {
        let slot = match personality {
            Personality::Cheerful => &mut self.cheerful,
            Personality::Calm => &mut self.calm,
            Personality::Study => &mut self.study,
            Personality::Dad => &mut self.dad,
        };
        *slot = messages;
    }

    fn to_memories(&self) -> PersonMemories {
        PersonMemories {
            cheerful: stored_from_chat(&self.cheerful),
            calm: stored_from_chat(&self.calm),
            study: stored_from_chat(&self.study),
            dad: stored_from_chat(&self.dad),
        }
    }
}

fn chat_from_stored(messages: &[StoredMessage]) -> Vec<ChatMessage> {
    messages
        .iter()
        .filter_map(|message| {
            let role = match message.role.as_str() {
                "user" => Role::User,
                "assistant" => Role::Assistant,
                _ => return None,
            };
            Some(ChatMessage {
                role,
                content: message.content.clone(),
            })
        })
        .collect()
}

fn stored_from_chat(messages: &[ChatMessage]) -> Vec<StoredMessage> {
    messages
        .iter()
        .filter_map(|message| {
            let role = match message.role {
                Role::User => "user",
                Role::Assistant => "assistant",
                Role::System => return None,
            };
            Some(StoredMessage {
                role: role.to_string(),
                content: message.content.clone(),
            })
        })
        .collect()
}

pub struct App {
    settings: Settings,
    conversation: Conversation,
    /// The other two characters' conversations. The active one lives in `conversation`.
    shelf: Shelf,
    pose: Pose,
    mouth: u8,
    status: String,
    whisper_loaded: bool,
    tamil_loaded: bool,
    bella_loaded: bool,
    sky_loaded: bool,
    mic_allowed: bool,
    tags: Vec<String>,
    recording: bool,
    /// Hands-free conversation is open until the user clicks Stop.
    conversing: bool,
    cancel: Arc<AtomicBool>,
    /// Unfinished typed turns: their cancel handle and the index of their user message.
    turns: Vec<(Arc<AtomicBool>, usize)>,
    suggestions: Vec<String>,
    suggest_epoch: u64,
    suggest_pending: bool,
}

impl App {
    pub fn new(settings: Settings) -> Self {
        Self::with_memories(settings, PersonMemories::default())
    }

    pub fn with_memories(mut settings: Settings, memories: PersonMemories) -> Self {
        let personality = settings.personality;
        let language = settings.language;
        settings.voice = character_voice(personality);
        let shelf = Shelf::from_memories(memories);
        let mut conversation = Conversation::new(personality);
        conversation.set_language(language);
        conversation.replace_messages(shelf.get(personality));
        Self {
            settings,
            conversation,
            shelf,
            pose: Pose::Idle,
            mouth: 0,
            status: String::new(),
            whisper_loaded: false,
            tamil_loaded: false,
            bella_loaded: false,
            sky_loaded: false,
            mic_allowed: false,
            tags: Vec::new(),
            recording: false,
            conversing: false,
            cancel: Arc::new(AtomicBool::new(false)),
            turns: Vec::new(),
            suggestions: Vec::new(),
            suggest_epoch: 0,
            suggest_pending: false,
        }
    }

    pub fn public_state(&self) -> PublicState {
        PublicState {
            settings: self.settings.clone(),
            pose: self.pose,
            mouth: self.mouth,
            status: self.status.clone(),
            transcript: self
                .conversation
                .messages()
                .iter()
                .map(|message| PublicMessage {
                    role: match message.role {
                        Role::User => "user".into(),
                        Role::Assistant => "assistant".into(),
                        Role::System => "system".into(),
                    },
                    content: match message.role {
                        Role::Assistant => crate::speech_text::drop_filler(&message.content),
                        _ => message.content.clone(),
                    },
                })
                .collect(),
            bella_loaded: self.bella_loaded,
            sky_loaded: self.sky_loaded,
            whisper_loaded: self.whisper_loaded,
            tamil_loaded: self.tamil_loaded,
            mic_allowed: self.mic_allowed,
            models: visible_models(&self.tags),
            resolved_model: self.resolved_model().unwrap_or_default(),
            recording: self.recording,
            conversing: self.conversing,
            pet: self.conversation.pet_view(),
            suggestions: self.suggestions.clone(),
        }
    }

    /// All three conversations, with the active one copied from the live transcript.
    pub fn memories(&self) -> PersonMemories {
        let mut shelf = self.shelf.clone();
        shelf.set(
            self.settings.personality,
            self.conversation.messages().to_vec(),
        );
        shelf.to_memories()
    }

    pub fn set_status(&mut self, status: impl Into<String>) {
        self.status = status.into();
    }

    pub fn set_personality(&mut self, personality: Personality) -> bool {
        if self.settings.personality == personality {
            return false;
        }
        // Stop the person who was talking. Their words stay in their own transcript.
        self.cancel.store(true, Ordering::SeqCst);
        self.turns.clear();
        self.shelf.set(
            self.settings.personality,
            self.conversation.take_messages(),
        );
        let next = self.shelf.get(personality);
        self.settings.personality = personality;
        self.settings.voice = character_voice(personality);
        self.conversation.set_personality(personality);
        self.conversation.set_language(self.settings.language);
        self.conversation.replace_messages(next);
        self.cancel = Arc::new(AtomicBool::new(false));
        self.recording = false;
        self.conversation.clear_pet();
        self.suggestions.clear();
        self.suggest_epoch = self.suggest_epoch.wrapping_add(1);
        self.suggest_pending = false;
        if matches!(self.pose, Pose::Speaking | Pose::Thinking | Pose::Listening) {
            self.pose = Pose::Idle;
            self.mouth = 0;
        }
        true
    }

    pub fn set_language(&mut self, language: Language) {
        self.settings.language = language;
        self.conversation.set_language(language);
    }

    pub fn set_model(&mut self, model: String) {
        self.settings.model = model;
    }

    pub fn set_backup_dir(&mut self, dir: String) {
        self.settings.backup_dir = dir;
    }

    pub fn set_voice(&mut self, _voice: VoiceId) -> Result<(), String> {
        Err("Each character has their own voice.".into())
    }

    pub fn active_listen_mode(&self) -> ListenMode {
        ListenMode::Click
    }

    pub fn resolved_model(&self) -> Option<String> {
        choose_model(&self.settings.model, &self.tags)
    }

    /// Tamil uses Vani. Pip uses the Mac robot voice. The others use Kokoro.
    pub fn can_speak(&self) -> bool {
        self.settings.language == Language::Tamil
            || self.settings.personality == Personality::Calm
            || self.resolved_voice().is_some()
    }

    pub fn ear_ready(&self) -> bool {
        match self.settings.language {
            Language::English => self.whisper_loaded,
            Language::Tamil => self.tamil_loaded,
        }
    }

    /// Kokoro is one pack, so any loaded flag means every character voice can speak.
    pub fn resolved_voice(&self) -> Option<VoiceId> {
        if self.bella_loaded || self.sky_loaded {
            Some(character_voice(self.settings.personality))
        } else {
            None
        }
    }

    fn waiting_outcome(&self) -> TurnOutcome {
        TurnOutcome {
            assistant_text: String::new(),
            speak: false,
            status: Some(self.status.clone()),
            cancelled: false,
            called_model: false,
        }
    }

    /// Statuses that must survive a new typed turn or a new recording.
    fn status_is_sticky(&self) -> bool {
        self.status == STATUS_OLLAMA_DOWN
            || self.status == STATUS_NO_MODEL
            || self.status == STATUS_MIC
            || self.status.ends_with("download failed.")
    }

    fn clear_transient_status(&mut self) {
        if !self.status_is_sticky() {
            self.status.clear();
        }
    }

    pub fn submit_text(&mut self, text: &str, client: &dyn ModelClient) -> TurnOutcome {
        if text.trim().is_empty() {
            return TurnOutcome {
                assistant_text: String::new(),
                speak: false,
                status: None,
                cancelled: false,
                called_model: false,
            };
        }
        if self.status == STATUS_OLLAMA_DOWN {
            return self.waiting_outcome();
        }
        let Some(model) = self.resolved_model() else {
            self.status = STATUS_NO_MODEL.into();
            return self.waiting_outcome();
        };
        self.clear_transient_status();
        self.cancel.store(true, Ordering::SeqCst);
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel = cancel.clone();
        self.recording = false;
        self.pose = Pose::Thinking;
        let voice_ready = self.can_speak();
        let outcome = run_turn(
            &mut self.conversation,
            &model,
            text,
            voice_ready,
            cancel.as_ref(),
            client,
        );
        self.apply_outcome(&outcome);
        outcome
    }

    pub fn begin_listen(&mut self) -> Result<(), String> {
        if !self.mic_allowed {
            self.status = STATUS_MIC.into();
            return Err(self.status.clone());
        }
        if !self.ear_ready() {
            self.status = if self.settings.language == Language::Tamil {
                "Tamil is still loading.".into()
            } else {
                "Whisper is still loading.".into()
            };
            return Err(self.status.clone());
        }
        self.clear_transient_status();
        self.cancel.store(true, Ordering::SeqCst);
        self.cancel = Arc::new(AtomicBool::new(false));
        self.conversing = true;
        self.recording = true;
        self.pose = Pose::Listening;
        Ok(())
    }

    /// Opens the ear again after a reply, only while the conversation is still open.
    pub fn resume_listen(&mut self) -> Result<(), String> {
        if self.status_is_sticky() {
            self.conversing = false;
            return Err(self.status.clone());
        }
        if !self.conversing {
            return Err("The conversation is closed.".into());
        }
        if !self.mic_allowed {
            self.conversing = false;
            self.status = STATUS_MIC.into();
            return Err(self.status.clone());
        }
        if !self.ear_ready() {
            self.conversing = false;
            self.status = if self.settings.language == Language::Tamil {
                "Tamil is still loading.".into()
            } else {
                "Whisper is still loading.".into()
            };
            return Err(self.status.clone());
        }
        self.clear_transient_status();
        self.cancel.store(true, Ordering::SeqCst);
        self.cancel = Arc::new(AtomicBool::new(false));
        self.recording = true;
        self.pose = Pose::Listening;
        Ok(())
    }

    pub fn conversing(&self) -> bool {
        self.conversing
    }

    /// True when the reply has finished and the microphone should open again.
    pub fn can_resume_conversation(&mut self) -> bool {
        if self.conversing && self.status_is_sticky() {
            self.conversing = false;
        }
        self.conversing && !self.recording
    }

    /// The user clicked Stop. Leave the conversation and drop any open capture.
    pub fn end_conversation(&mut self) {
        self.conversing = false;
        self.recording = false;
        self.mouth = 0;
        self.pose = Pose::Idle;
        self.cancel.store(true, Ordering::SeqCst);
        self.cancel = Arc::new(AtomicBool::new(false));
    }

    /// Speech has paused. Keep the conversation open and think about the answer.
    pub fn catch_utterance(&mut self) {
        self.recording = false;
        self.pose = Pose::Thinking;
    }

    pub fn finish_playback(&mut self) {
        self.pose = Pose::Idle;
        self.mouth = 0;
    }

    pub fn mark_whisper(&mut self, loaded: bool) {
        self.whisper_loaded = loaded;
    }

    pub fn mark_tamil(&mut self, loaded: bool) {
        self.tamil_loaded = loaded;
    }

    pub fn mark_voice(&mut self, voice: VoiceId, loaded: bool) {
        match voice {
            VoiceId::Bella => self.bella_loaded = loaded,
            VoiceId::Sky => self.sky_loaded = loaded,
            VoiceId::Emma | VoiceId::Adam | VoiceId::George => {
                self.bella_loaded = loaded;
                self.sky_loaded = loaded;
            }
        }
    }

    pub fn mark_mic(&mut self, allowed: bool) {
        self.mic_allowed = allowed;
    }

    pub fn set_tags(&mut self, tags: Vec<String>) {
        self.tags = tags;
    }

    pub fn note_download_failure(&mut self, label: &str) {
        self.status = format!("{label} download failed.");
    }

    pub fn finish_pet_reply(&mut self) {
        self.conversation.finish_pet_reply();
    }

    pub fn cancel_flag(&self) -> Arc<AtomicBool> {
        self.cancel.clone()
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    pub fn stop_listen(&mut self) {
        self.recording = false;
        self.pose = Pose::Idle;
    }

    pub fn prepare_turn(
        &mut self,
        text: &str,
    ) -> Result<
        (
            String,
            Vec<crate::conversation::ChatMessage>,
            Arc<AtomicBool>,
        ),
        (),
    > {
        if text.trim().is_empty() {
            return Err(());
        }
        if self.status == STATUS_OLLAMA_DOWN {
            return Err(());
        }
        let Some(model) = self.resolved_model() else {
            self.set_status(STATUS_NO_MODEL);
            return Err(());
        };
        self.clear_transient_status();
        self.suggestions.clear();
        self.suggest_epoch = self.suggest_epoch.wrapping_add(1);
        self.suggest_pending = false;
        self.cancel.store(true, Ordering::SeqCst);
        self.conversation.note_pet_line(text);
        self.conversation.push_user(text).map_err(|_| ())?;
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel = cancel.clone();
        self.turns
            .push((cancel.clone(), self.conversation.messages().len() - 1));
        self.recording = false;
        self.pose = Pose::Thinking;
        let mut messages = self.conversation.request_messages();
        if messages
            .last()
            .is_some_and(|message| message.role == Role::Assistant && message.content.is_empty())
        {
            messages.pop();
        }
        Ok((model, messages, cancel))
    }

    /// Starts one background suggestion request. A later turn drops its result.
    pub fn take_suggestion_job(
        &mut self,
        force: bool,
    ) -> Option<(u64, String, Vec<ChatMessage>)> {
        if self.suggest_pending {
            return None;
        }
        if !force && (self.pose != Pose::Idle || !self.suggestions.is_empty()) {
            return None;
        }
        let model = self.resolved_model()?;
        self.suggest_pending = true;
        self.suggest_epoch = self.suggest_epoch.wrapping_add(1);
        let mut messages =
            crate::suggestions::suggestion_messages(self.conversation.request_messages());
        if self.settings.language == Language::Tamil {
            if let Some(ask) = messages.last_mut() {
                ask.content.push_str(crate::prompts::TAMIL_NUDGE);
            }
        }
        Some((self.suggest_epoch, model, messages))
    }

    pub fn offer_suggestions(&mut self, epoch: u64, lines: Vec<String>) -> bool {
        if epoch != self.suggest_epoch {
            return false;
        }
        self.suggest_pending = false;
        self.suggestions = lines;
        true
    }

    pub fn complete_turn(
        &mut self,
        assistant_text: String,
        cancelled: bool,
        error: Option<String>,
    ) {
        let result = if cancelled {
            Err(ModelError::Cancelled)
        } else if let Some(message) = error {
            Err(ModelError::Failed(message))
        } else {
            Ok(())
        };
        let outcome = outcome_of(assistant_text, self.can_speak(), result);
        self.apply_outcome(&outcome);
    }

    fn apply_outcome(&mut self, outcome: &TurnOutcome) {
        if outcome.cancelled {
            self.pose = Pose::Idle;
            self.mouth = 0;
            return;
        }
        if let Some(status) = &outcome.status {
            self.status = status.clone();
        }
        if outcome.speak {
            self.pose = Pose::Speaking;
            self.mouth = 0;
        } else {
            self.pose = Pose::Idle;
            self.mouth = 0;
        }
    }

    fn shift_later_turns(&mut self, user_index: usize) {
        for (_, later) in self.turns.iter_mut() {
            if *later > user_index {
                *later += 1;
            }
        }
    }

    /// Appends one streamed token onto that turn's assistant message, creating
    /// the message after its user row when this is the first token.
    pub fn push_turn_token(&mut self, handle: &Arc<AtomicBool>, token: &str) {
        if token.is_empty() {
            return;
        }
        let Some(index) = self
            .turns
            .iter()
            .find(|(turn, _)| Arc::ptr_eq(turn, handle))
            .map(|(_, index)| *index)
        else {
            return;
        };
        let inserted = {
            let messages = self.conversation.messages_mut();
            if !messages
                .get(index)
                .is_some_and(|message| message.role == Role::User)
            {
                false
            } else if messages
                .get(index + 1)
                .is_some_and(|message| message.role == Role::Assistant)
            {
                messages[index + 1].content.push_str(token);
                false
            } else {
                messages.insert(
                    index + 1,
                    crate::conversation::ChatMessage {
                        role: Role::Assistant,
                        content: token.to_string(),
                    },
                );
                true
            }
        };
        if inserted {
            self.shift_later_turns(index);
        }
    }

    /// Stores `text` once. A reply already written by `push_turn_token` is left
    /// in place when it already matches, and is replaced when it is a shorter
    /// prefix, never appended onto.
    fn store_reply_once(&mut self, user_index: Option<usize>, text: &str) {
        if text.is_empty() {
            return;
        }
        let Some(index) = user_index else {
            let messages = self.conversation.messages_mut();
            if messages
                .last()
                .is_some_and(|message| message.role == Role::Assistant && message.content == text)
            {
                return;
            }
            messages.push(crate::conversation::ChatMessage {
                role: Role::Assistant,
                content: text.to_string(),
            });
            return;
        };
        let inserted = {
            let messages = self.conversation.messages_mut();
            if !messages
                .get(index)
                .is_some_and(|message| message.role == Role::User)
            {
                if messages.last().is_some_and(|message| {
                    message.role == Role::Assistant && message.content == text
                }) {
                    false
                } else {
                    messages.push(crate::conversation::ChatMessage {
                        role: Role::Assistant,
                        content: text.to_string(),
                    });
                    false
                }
            } else if messages
                .get(index + 1)
                .is_some_and(|message| message.role == Role::Assistant)
            {
                let existing = &messages[index + 1].content;
                if existing != text && !existing.starts_with(text) {
                    messages[index + 1].content = text.to_string();
                }
                false
            } else {
                messages.insert(
                    index + 1,
                    crate::conversation::ChatMessage {
                        role: Role::Assistant,
                        content: text.to_string(),
                    },
                );
                true
            }
        };
        if inserted {
            self.shift_later_turns(index);
        }
        self.clean_stored_reply(user_index);
    }

    fn clean_stored_reply(&mut self, user_index: Option<usize>) {
        let messages = self.conversation.messages_mut();
        let message = match user_index {
            Some(index) => messages.get_mut(index + 1),
            None => messages.last_mut(),
        };
        if let Some(message) = message {
            if message.role == Role::Assistant {
                message.content = crate::speech_text::drop_filler(&message.content);
            }
        }
    }

    /// Finishes the turn that owns `handle` from the stream `run_message` ran.
    /// Pose and status come from `outcome_of`. Returns true when the turn was
    /// still current.
    pub fn finish_stream(
        &mut self,
        handle: &Arc<AtomicBool>,
        assistant_text: String,
        result: Result<(), ModelError>,
    ) -> bool {
        let (cancelled, error) = match result {
            Ok(()) => (false, None),
            Err(ModelError::Cancelled) => (true, None),
            Err(ModelError::Failed(message)) => (false, Some(message)),
        };
        self.finish_turn(handle, assistant_text, cancelled, error)
    }

    /// Finishes the turn that owns `handle`. A turn that is still current goes
    /// through `complete_turn`. A superseded turn only puts its reply right after
    /// its own user message and leaves pose, mouth and status alone. Returns true
    /// when the turn was still current. A reply already stored token by token is
    /// not written again.
    pub fn finish_turn(
        &mut self,
        handle: &Arc<AtomicBool>,
        assistant_text: String,
        cancelled: bool,
        error: Option<String>,
    ) -> bool {
        let Some(position) = self
            .turns
            .iter()
            .position(|(turn, _)| Arc::ptr_eq(turn, handle))
        else {
            // The turn was retired when the character changed. Its partial reply
            // already sits in that character's transcript.
            return false;
        };
        let index = self.turns.remove(position).1;
        let current = Arc::ptr_eq(handle, &self.cancel);
        self.store_reply_once(Some(index), &assistant_text);
        if current {
            self.complete_turn(assistant_text, cancelled, error);
        }
        current
    }

    pub fn hear_nothing(&mut self) {
        self.recording = false;
        self.pose = Pose::Idle;
        self.status = STATUS_EMPTY_HEARING.into();
    }
}

pub fn apply_tag_result(app: &mut App, tags: Result<Vec<String>, String>) {
    match tags {
        Ok(tags) => {
            app.set_tags(tags);
            if app.resolved_model().is_none() {
                app.set_status(STATUS_NO_MODEL);
            } else if app.status == STATUS_OLLAMA_DOWN || app.status == STATUS_NO_MODEL {
                app.set_status("");
            }
        }
        Err(_) => app.set_status(STATUS_OLLAMA_DOWN),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FetchReport {
    pub whisper: bool,
    pub tamil: bool,
    pub kokoro: bool,
    pub failures: Vec<String>,
}

pub fn apply_fetch_report(app: &mut App, report: &FetchReport) {
    app.mark_whisper(report.whisper);
    app.mark_tamil(report.tamil);
    app.mark_voice(VoiceId::Bella, report.kokoro);
    app.mark_voice(VoiceId::Sky, report.kokoro);
    if let Some(label) = report.failures.last() {
        if app.status.is_empty() || app.status.ends_with("download failed.") {
            app.note_download_failure(label);
        }
    } else if app.status.ends_with(" download failed.") {
        app.set_status("");
    }
}

/// Payload of the `download_progress` event.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DownloadProgress {
    pub pack: String,
    pub percent: u8,
    pub failed: bool,
}

/// How long one speech archive may take, including its body.
const ARCHIVE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15 * 60);

/// HTTP client for the speech archives. Connecting still gives up after 30
/// seconds. Each archive gives up after fifteen minutes so a stall cannot sit
/// forever.
pub fn download_client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(30))
        .timeout(ARCHIVE_TIMEOUT)
        .build()
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{ListenMode, Personality, Settings, VoiceId};
    use crate::turn::{Control, ModelClient, ModelError};
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Quiet;
    impl ModelClient for Quiet {
        fn stream(
            &self,
            _model: &str,
            _messages: &[crate::conversation::ChatMessage],
            _on_token: &mut dyn FnMut(&str) -> Control,
        ) -> Result<(), ModelError> {
            Ok(())
        }
    }

    struct Counting(AtomicUsize);
    impl ModelClient for Counting {
        fn stream(
            &self,
            _model: &str,
            _messages: &[crate::conversation::ChatMessage],
            on_token: &mut dyn FnMut(&str) -> Control,
        ) -> Result<(), ModelError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            let _ = on_token("Hi");
            Ok(())
        }
    }

    fn wake_settings() -> Settings {
        Settings {
            listen_mode: ListenMode::Wake,
            ..Settings::default()
        }
    }

    #[test]
    fn wake_setting_still_uses_click() {
        let app = App::new(wake_settings());
        let state = app.public_state();
        assert_eq!(app.active_listen_mode(), ListenMode::Click);
        assert!(!state.recording);
        assert_eq!(state.settings.listen_mode, ListenMode::Wake);
        assert_eq!(state.settings.wake_phrase, "hey lumi");
    }

    #[test]
    fn listen_requires_whisper_and_permission() {
        let mut app = App::new(Settings::default());
        assert!(app.begin_listen().is_err());
        assert_eq!(app.public_state().status, "The microphone is unavailable.");
        app.mark_mic(true);
        assert!(app.begin_listen().is_err());
        app.mark_whisper(true);
        app.begin_listen().unwrap();
        assert!(app.public_state().recording);
        assert_eq!(app.public_state().pose, Pose::Listening);
    }

    #[test]
    fn no_model_does_not_call_the_client() {
        let mut app = App::new(Settings::default());
        app.set_tags(vec!["nomic-embed-text:latest".into()]);
        let calls = Counting(AtomicUsize::new(0));
        app.submit_text("Hello", &calls);
        assert_eq!(calls.0.load(Ordering::SeqCst), 0);
        assert_eq!(app.public_state().status, "No chat model is installed.");
    }

    #[test]
    fn each_personality_keeps_its_own_transcript() {
        let mut app = App::new(Settings::default());
        app.set_tags(vec!["gemma4:latest".into()]);
        app.mark_voice(VoiceId::Bella, true);
        app.submit_text("Hello", &Counting(AtomicUsize::new(0)));
        assert!(app
            .public_state()
            .transcript
            .iter()
            .any(|message| message.content == "Hello"));
        app.set_personality(Personality::Study);
        assert!(app.public_state().transcript.is_empty());
        assert_eq!(app.public_state().settings.personality, Personality::Study);
        app.submit_text("What is two plus two?", &Counting(AtomicUsize::new(0)));
        assert!(app
            .public_state()
            .transcript
            .iter()
            .all(|message| message.content != "Hello"));
        app.set_personality(Personality::Cheerful);
        let restored = app.public_state().transcript;
        assert!(restored.iter().any(|message| message.content == "Hello"));
        assert!(restored
            .iter()
            .all(|message| message.content != "What is two plus two?"));
    }

    #[test]
    fn a_later_turn_still_sees_the_earlier_one() {
        let mut app = App::new(Settings::default());
        app.set_tags(vec!["gemma4:latest".into()]);
        app.mark_voice(VoiceId::Bella, true);
        let (_, _, turn) = app.prepare_turn("Hello").unwrap();
        assert!(app.finish_turn(&turn, "Hi".into(), false, None));
        let (_, messages, _) = app.prepare_turn("What did I just say?").unwrap();
        assert!(messages.iter().any(|message| message.content == "Hello"));
        assert!(messages.iter().any(|message| message.content == "Hi"));
        assert_eq!(
            messages.last().map(|message| message.content.as_str()),
            Some("What did I just say?")
        );
    }

    #[test]
    fn saved_memories_come_back_with_the_same_person() {
        let mut app = App::new(Settings::default());
        app.set_tags(vec!["gemma4:latest".into()]);
        app.mark_voice(VoiceId::Bella, true);
        app.submit_text("Hello", &Counting(AtomicUsize::new(0)));
        app.set_personality(Personality::Calm);
        app.submit_text("Pip, are you there?", &Counting(AtomicUsize::new(0)));
        let saved = app.memories();
        let mut restored = App::with_memories(
            Settings {
                personality: Personality::Calm,
                ..Settings::default()
            },
            saved,
        );
        assert!(restored
            .public_state()
            .transcript
            .iter()
            .any(|message| message.content == "Pip, are you there?"));
        restored.set_personality(Personality::Cheerful);
        assert!(restored
            .public_state()
            .transcript
            .iter()
            .any(|message| message.content == "Hello"));
        assert!(restored
            .public_state()
            .transcript
            .iter()
            .all(|message| message.content != "Pip, are you there?"));
    }

    #[test]
    fn listen_stays_open_until_stop_and_a_closed_conversation_does_not_resume() {
        let mut app = App::new(Settings::default());
        app.mark_mic(true);
        app.mark_whisper(true);
        app.begin_listen().unwrap();
        assert!(app.public_state().conversing);
        assert!(app.public_state().recording);
        app.catch_utterance();
        assert!(app.conversing());
        assert!(!app.public_state().recording);
        assert!(app.can_resume_conversation());
        app.resume_listen().unwrap();
        assert_eq!(app.public_state().pose, Pose::Listening);
        app.end_conversation();
        assert!(!app.public_state().conversing);
        assert!(app.resume_listen().is_err());
    }

    #[test]
    fn download_failure_names_the_pack() {
        let mut app = App::new(Settings::default());
        app.note_download_failure("Whisper Tiny English");
        assert_eq!(
            app.public_state().status,
            "Whisper Tiny English download failed."
        );
    }

    #[test]
    fn a_voice_cannot_be_chosen_apart_from_its_character() {
        let mut app = App::new(Settings::default());
        app.mark_voice(VoiceId::Sky, true);
        assert_eq!(
            app.set_voice(VoiceId::Bella),
            Err("Each character has their own voice.".into())
        );
        assert_eq!(app.public_state().settings.voice, VoiceId::Sky);
        app.set_personality(Personality::Dad);
        assert_eq!(app.resolved_voice(), Some(VoiceId::George));
        assert!(app.public_state().transcript.is_empty());
    }

    #[test]
    fn ollama_failure_sets_the_unavailable_status() {
        let mut app = App::new(Settings::default());
        apply_tag_result(&mut app, Err("connection refused".into()));
        assert_eq!(app.public_state().status, "Ollama is unavailable.");
    }

    #[test]
    fn successful_tags_clear_stale_model_status() {
        let mut app = App::new(Settings::default());
        apply_tag_result(&mut app, Err("connection refused".into()));
        assert_eq!(app.public_state().status, "Ollama is unavailable.");
        apply_tag_result(&mut app, Ok(vec!["gemma4:latest".into()]));
        assert_eq!(app.public_state().status, "");

        apply_tag_result(&mut app, Ok(vec!["nomic-embed-text:latest".into()]));
        assert_eq!(app.public_state().status, "No chat model is installed.");
        apply_tag_result(&mut app, Ok(vec!["gemma4:latest".into()]));
        assert_eq!(app.public_state().status, "");
    }

    #[test]
    fn a_backup_folder_is_remembered() {
        let mut app = App::new(Settings::default());
        assert_eq!(app.public_state().settings.backup_dir, "");
        app.set_backup_dir("/tmp/lumi-notes".into());
        assert_eq!(app.public_state().settings.backup_dir, "/tmp/lumi-notes");
    }

    #[test]
    fn pip_can_speak_without_kokoro() {
        let app = App::new(Settings {
            personality: Personality::Calm,
            ..Settings::default()
        });
        assert!(app.can_speak());
        assert!(app.resolved_voice().is_none());
    }

    #[test]
    fn tamil_listens_with_the_tamil_recognizer_and_can_speak_without_kokoro() {
        let mut app = App::new(Settings {
            language: Language::Tamil,
            ..Settings::default()
        });
        app.mark_mic(true);
        app.mark_whisper(true);
        assert_eq!(app.begin_listen().unwrap_err(), "Tamil is still loading.");
        assert!(app.can_speak());
        app.mark_tamil(true);
        app.begin_listen().unwrap();
        assert_eq!(app.public_state().pose, Pose::Listening);
    }

    #[test]
    fn missing_whisper_replaces_microphone_status() {
        let mut app = App::new(Settings::default());
        assert_eq!(
            app.begin_listen().unwrap_err(),
            "The microphone is unavailable."
        );
        app.mark_mic(true);
        assert_eq!(app.begin_listen().unwrap_err(), "Whisper is still loading.");
        assert_eq!(app.public_state().status, "Whisper is still loading.");
    }

    #[test]
    fn superseded_turn_keeps_its_reply_after_its_own_question() {
        let mut app = App::new(Settings::default());
        app.set_tags(vec!["gemma4:latest".into()]);
        let (_, _, first) = app.prepare_turn("First").unwrap();
        let (_, _, second) = app.prepare_turn("Second").unwrap();
        assert!(first.load(Ordering::SeqCst));
        assert!(!app.finish_turn(&first, "One".into(), true, None));
        let state = app.public_state();
        let rows: Vec<_> = state
            .transcript
            .iter()
            .map(|m| (m.role.as_str(), m.content.as_str()))
            .collect();
        assert_eq!(
            rows,
            vec![("user", "First"), ("assistant", "One"), ("user", "Second")]
        );
        assert_eq!(state.pose, Pose::Thinking);
        assert_eq!(state.status, "");
        assert!(app.finish_turn(&second, "Two".into(), false, None));
        assert_eq!(app.public_state().transcript.len(), 4);
    }

    #[test]
    fn listening_survives_a_late_finish_of_the_old_turn() {
        let mut app = App::new(Settings::default());
        app.set_tags(vec!["gemma4:latest".into()]);
        app.mark_mic(true);
        app.mark_whisper(true);
        let (_, _, old) = app.prepare_turn("Hello").unwrap();
        app.begin_listen().unwrap();
        assert!(old.load(Ordering::SeqCst));
        assert!(!app.finish_turn(&old, "Hi".into(), true, None));
        let state = app.public_state();
        assert_eq!(state.pose, Pose::Listening);
        assert!(state.recording);
        assert_eq!(state.transcript.len(), 2);
        assert_eq!(state.transcript[1].content, "Hi");
    }

    #[test]
    fn fetch_report_marks_loaded_engines_and_names_failures() {
        let mut app = App::new(Settings::default());
        apply_fetch_report(
            &mut app,
            &FetchReport {
                whisper: true,
                tamil: false,
                kokoro: false,
                failures: vec!["Kokoro English".into()],
            },
        );
        let state = app.public_state();
        assert!(state.whisper_loaded);
        assert!(!state.bella_loaded);
        assert!(!state.sky_loaded);
        assert_eq!(state.status, "Kokoro English download failed.");

        apply_fetch_report(
            &mut app,
            &FetchReport {
                whisper: true,
                tamil: true,
                kokoro: true,
                failures: Vec::new(),
            },
        );
        let ready = app.public_state();
        assert!(ready.bella_loaded);
        assert!(ready.sky_loaded);
        assert!(ready.tamil_loaded);
    }

    #[test]
    fn download_failure_does_not_replace_ollama_or_model_status() {
        let mut app = App::new(Settings::default());
        let report = FetchReport {
            whisper: false,
            tamil: false,
            kokoro: false,
            failures: vec!["Kokoro English".into()],
        };
        app.set_status(STATUS_OLLAMA_DOWN);
        apply_fetch_report(&mut app, &report);
        assert_eq!(app.public_state().status, STATUS_OLLAMA_DOWN);
        app.set_status(STATUS_NO_MODEL);
        apply_fetch_report(&mut app, &report);
        assert_eq!(app.public_state().status, STATUS_NO_MODEL);
    }

    #[test]
    fn unavailable_ollama_keeps_typed_text_and_status() {
        let mut app = App::new(Settings::default());
        apply_tag_result(&mut app, Err("connection refused".into()));
        assert!(app.prepare_turn("Hello there").is_err());
        assert_eq!(app.public_state().status, STATUS_OLLAMA_DOWN);
        assert!(app.public_state().transcript.is_empty());
        let outcome = app.submit_text("Hello there", &Quiet);
        assert!(!outcome.called_model);
        assert!(app.public_state().transcript.is_empty());
        assert_eq!(app.public_state().status, STATUS_OLLAMA_DOWN);
    }

    #[test]
    fn new_turn_clears_stale_status_but_not_sticky_status() {
        let mut app = App::new(Settings::default());
        app.set_tags(vec!["gemma4:latest".into()]);
        app.set_status("She could not speak.");
        app.prepare_turn("Hi").unwrap();
        assert_eq!(app.public_state().status, "");

        app.set_status(STATUS_EMPTY_HEARING);
        app.prepare_turn("Again").unwrap();
        assert_eq!(app.public_state().status, "");

        app.set_status("Kokoro English download failed.");
        app.prepare_turn("Still").unwrap();
        assert_eq!(app.public_state().status, "Kokoro English download failed.");

        app.set_status(STATUS_MIC);
        app.mark_mic(true);
        app.mark_whisper(true);
        app.begin_listen().unwrap();
        assert_eq!(app.public_state().status, STATUS_MIC);
        app.stop_listen();

        app.set_status(STATUS_EMPTY_HEARING);
        app.begin_listen().unwrap();
        assert_eq!(app.public_state().status, "");
    }

    #[test]
    fn missing_sky_uses_loaded_bella() {
        let mut app = App::new(Settings {
            voice: VoiceId::Sky,
            ..Settings::default()
        });
        app.set_tags(vec!["gemma4:latest".into()]);
        assert!(app.resolved_voice().is_none());
        app.mark_voice(VoiceId::Bella, true);
        assert_eq!(app.resolved_voice(), Some(VoiceId::Sky));
        let (_, _, turn) = app.prepare_turn("Hello").unwrap();
        assert!(app.finish_turn(&turn, "Hi".into(), false, None));
        let state = app.public_state();
        assert_eq!(state.pose, Pose::Speaking);
        assert_eq!(
            state.transcript.last().map(|row| row.content.as_str()),
            Some("Hi")
        );
        assert_ne!(state.status, "She could not speak.");

        let mut quiet = App::new(Settings {
            voice: VoiceId::Sky,
            ..Settings::default()
        });
        quiet.set_tags(vec!["gemma4:latest".into()]);
        let (_, _, turn) = quiet.prepare_turn("Hello").unwrap();
        assert!(quiet.finish_turn(&turn, "Hi".into(), false, None));
        assert_eq!(quiet.public_state().status, "She could not speak.");
        assert_eq!(quiet.public_state().pose, Pose::Idle);
    }

    #[test]
    fn finish_turn_does_not_store_the_same_reply_twice() {
        let mut app = App::new(Settings::default());
        app.set_tags(vec!["gemma4:latest".into()]);
        app.mark_voice(VoiceId::Bella, true);
        let (_, _, first) = app.prepare_turn("One").unwrap();
        app.push_turn_token(&first, "Par");
        app.push_turn_token(&first, "tial");
        let (_, _, second) = app.prepare_turn("Two").unwrap();
        assert!(!app.finish_turn(&first, "Partial".into(), true, None));
        app.push_turn_token(&second, "Answer");
        assert!(app.finish_turn(&second, "Answer".into(), false, None));
        let assistants: Vec<_> = app
            .public_state()
            .transcript
            .iter()
            .filter(|message| message.role == "assistant")
            .map(|message| message.content.clone())
            .collect();
        assert_eq!(
            assistants,
            vec!["Partial".to_string(), "Answer".to_string()]
        );
    }

    #[test]
    fn public_state_names_the_model_that_will_be_called() {
        let mut app = App::new(Settings {
            model: "gone:latest".into(),
            ..Settings::default()
        });
        app.set_tags(vec!["gemma4:latest".into(), "qwen3.6:latest".into()]);
        let state = app.public_state();
        assert_eq!(state.settings.model, "gone:latest");
        assert_eq!(state.resolved_model, "gemma4:latest");
    }

    fn ready_app() -> App {
        let mut app = App::new(Settings::default());
        app.set_tags(vec!["gemma4:latest".into()]);
        app
    }

    #[test]
    fn the_assistants_words_do_not_keep_the_pet() {
        let mut app = ready_app();
        let (_, _, turn) = app.prepare_turn("Biscuit, sit").unwrap();
        app.finish_turn(&turn, " ".into(), false, None);
        app.finish_pet_reply();
        let (_, still, dinner) = app.prepare_turn("What is for dinner?").unwrap();
        assert!(still[0].content.contains("Biscuit the puppy is still here."));
        app.finish_turn(&dinner, "Biscuit is wonderful".into(), false, None);
        app.finish_pet_reply();
        let (_, leaving, _) = app.prepare_turn("What movie?").unwrap();
        assert!(leaving[0]
            .content
            .contains("bored and leaves during this reply"));
        assert!(app.public_state().pet.unwrap().leaving_after_reply);
    }

    #[test]
    fn switching_person_clears_the_pet() {
        let mut app = ready_app();
        app.prepare_turn("Puddle, quack").unwrap();
        app.set_personality(Personality::Dad);
        assert!(app.public_state().pet.is_none());
        assert!(app
            .prepare_turn("Hello")
            .unwrap()
            .1[0]
            .content
            .contains("No pet is in the room."));
    }

    #[test]
    fn an_empty_line_does_not_change_the_pet() {
        let mut app = ready_app();
        app.prepare_turn("Miso").unwrap();
        assert!(app.prepare_turn("  ").is_err());
        assert_eq!(app.public_state().pet.unwrap().name, "miso");
    }

    #[test]
    fn suggestions_stay_out_of_the_transcript() {
        let mut app = ready_app();
        let (epoch, model, messages) = app.take_suggestion_job(true).unwrap();
        assert_eq!(model, "gemma4:latest");
        assert!(messages.last().unwrap().content.contains("three short"));
        assert!(app.conversation.messages().is_empty());
        assert!(app.take_suggestion_job(true).is_none());
        assert!(app.offer_suggestions(epoch, vec!["Want to draw?".into()]));
        assert_eq!(
            app.public_state().suggestions,
            vec!["Want to draw?".to_string()]
        );
        assert!(app.take_suggestion_job(false).is_none());
    }

    #[test]
    fn a_new_turn_drops_a_stale_suggestion() {
        let mut app = ready_app();
        let (epoch, _, _) = app.take_suggestion_job(true).unwrap();
        app.prepare_turn("Hi").unwrap();
        assert!(!app.offer_suggestions(epoch, vec!["Old".into()]));
        assert!(app.public_state().suggestions.is_empty());
        assert!(app.conversation.messages().len() == 1);
    }
}
