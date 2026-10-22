# Lumi Companion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the first Lumi build: a macOS window where a cartoon girl chats through local Ollama, listens with click-to-talk Whisper, and speaks with Piper.

**Architecture:** A tested Rust library (`lumi-core`) owns settings, prompts, the transcript, model choice, Ollama streaming, speech-pack files, and the app state machine. A Tauri 2 shell draws Lumi and calls that library. sherpa-onnx is loaded only in the shell, for Whisper and Piper.

**Tech Stack:** Rust, Tauri 2.12, Vite, TypeScript, Ollama HTTP, sherpa-onnx 1.13.8, cpal, rodio, reqwest.

**Spec:** `docs/superpowers/specs/2026-10-22-lumi-companion-design.md`

## Global Constraints

- First build only. Do not download or run the wake-word model. `listen_mode` and `wake_phrase` are stored and ignored by speech code. The window has no Wake word control. `active_listen_mode()` always returns Click.
- Tauri 2, bundle id `com.lumi.companion`. Window default 960×640. Lumi column is 280 pixels.
- Vite + TypeScript. No React.
- Ollama HTTP API at `http://127.0.0.1:11434`. Poll up to 20 seconds after starting `ollama serve`.
- Hide model tags whose names contain `embed`.
- Saved-model fallback: `gemma4:latest`, then the first tag that starts with `gemma4`, then the first remaining visible tag.
- Settings and models live under `~/Library/Application Support/com.lumi.companion/`.
- Default personality `cheerful`, voice `amy`, model `gemma4:latest`, listen mode `click`, wake phrase `hey lumi`.
- Speech rates: cheerful 1.08, calm 0.92, study 1.00.
- System prompts, copied verbatim:
  - Cheerful: `You are Lumi, a cheerful cartoon kid friend on this computer. Use short, playful, curious sentences and simple words. Replies are spoken aloud, so use plain sentences, no markdown tables, and no long outlines. You only chat. If asked to open an app, read a file, or set a reminder, say you cannot do that yet and answer in words when you can. Stay kind. You do not do romantic or adult roleplay.`
  - Calm: `You are Lumi, a calm little helper on this computer. Be polite, brief, and clear. Replies are spoken aloud, so use plain sentences, no markdown tables, and no long outlines. You only chat. If asked to open an app, read a file, or set a reminder, say you cannot do that yet and answer in words when you can. Stay kind. You do not do romantic or adult roleplay.`
  - Study: `You are Lumi, a study buddy on this computer. Explain things simply and offer one example when it helps. Replies are spoken aloud, so use plain sentences, no markdown tables, and no long outlines. You only chat. If asked to open an app, read a file, or set a reminder, say you cannot do that yet and answer in words when you can. Stay kind. You do not do romantic or adult roleplay.`
- The Ollama request does not send tools. The tool list stays empty.
- Transcript stays in memory. Quitting discards it. `settings.json` has only the five settings fields.
- Whisper Tiny English int8 and Piper Amy and Lessac from the spec URLs. Mic audio is resampled to 16000 Hz. A recording stops at 30 seconds.
- Microphone usage string: `Lumi uses the microphone so you can talk to her.`
- Mouth levels are 0, 1, 2, or 3. Rust emits them. The page only displays them.
- Status strings: `It did not catch anything.` `She could not speak.` `No chat model is installed.` `Ollama is unavailable.` `The microphone is unavailable.` A failed pack uses `{label} download failed.`
- Target is macOS arm64.

## Review Focus

- A second message arrives while tokens are still streaming. The first stream stops, tokens already shown stay in that assistant bubble, and later tokens from the cancelled stream are not appended.
- The saved Ollama tag is not in the list. Choice falls through `gemma4:latest`, then any `gemma4` prefix, then the first visible tag, and an embed-only list yields no model.
- The typed text or the transcript is only whitespace. No user bubble is added and the model client is not called.
- Ollama returns an error after at least one token. That token stays, the status is the error text, and Lumi is not asked to speak.
- Settings say `listen_mode` is `wake`. The first build still waits for a click, leaves the microphone closed, and reports Click as the active mode.

## File structure

- `crates/lumi-core/src/settings.rs` — load, save, defaults, support path.
- `crates/lumi-core/src/prompts.rs` — exact system text and speech rate.
- `crates/lumi-core/src/conversation.rs` — transcript, empty input, empty tool list.
- `crates/lumi-core/src/model_choice.rs` — hide embed tags and pick a model.
- `crates/lumi-core/src/turn.rs` — one streamed turn against a `ModelClient`.
- `crates/lumi-core/src/ollama.rs` — HTTP tags and streaming chat.
- `crates/lumi-core/src/packs.rs` — the three archives and required files.
- `crates/lumi-core/src/audio.rs` — resample, 30 second cap, mouth level.
- `crates/lumi-core/src/app.rs` — pose, status, selectors, forced Click mode.
- `crates/lumi-core/src/lib.rs` — re-exports.
- `src-tauri/` — Tauri shell, microphone, Whisper, Piper, playback.
- `src/` — Vite page, SVG Lumi, transcript, selectors.

---

### Task 1: Settings

**Files:**
- Create: `Cargo.toml`
- Create: `crates/lumi-core/Cargo.toml`
- Create: `crates/lumi-core/src/lib.rs`
- Create: `crates/lumi-core/src/settings.rs`
- Create: `.gitignore`
- Test: `crates/lumi-core/src/settings.rs`

**Interfaces:**
- Consumes: nothing
- Produces: `Personality`, `VoiceId`, `ListenMode`, `Settings`, `load_settings(path) -> Settings`, `save_settings(path, &Settings) -> io::Result<()>`, `app_support_dir(home) -> PathBuf`, `settings_path(home) -> PathBuf`

- [ ] **Step 1: Write the failing test**

Create the workspace and a settings module whose tests call functions that do not exist yet.

`Cargo.toml`:

```toml
[workspace]
members = ["crates/lumi-core"]
resolver = "2"
```

`crates/lumi-core/Cargo.toml`:

```toml
[package]
name = "lumi-core"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

`crates/lumi-core/src/lib.rs`:

```rust
pub mod settings;
```

`crates/lumi-core/src/settings.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn missing_file_uses_defaults() {
        let settings = load_settings(std::path::Path::new("/tmp/lumi-missing-settings.json"));
        assert_eq!(settings, Settings::default());
        assert_eq!(settings.personality, Personality::Cheerful);
        assert_eq!(settings.model, "gemma4:latest");
        assert_eq!(settings.voice, VoiceId::Amy);
        assert_eq!(settings.listen_mode, ListenMode::Click);
        assert_eq!(settings.wake_phrase, "hey lumi");
    }

    #[test]
    fn unreadable_json_uses_defaults() {
        let path = std::env::temp_dir().join("lumi-bad-settings.json");
        std::fs::write(&path, "{not json").unwrap();
        assert_eq!(load_settings(&path), Settings::default());
    }

    #[test]
    fn round_trip_keeps_only_the_five_fields() {
        let path = std::env::temp_dir().join("lumi-settings-roundtrip.json");
        let settings = Settings {
            personality: Personality::Study,
            model: "qwen3.6:latest".into(),
            voice: VoiceId::Lessac,
            listen_mode: ListenMode::Wake,
            wake_phrase: "hey lumi".into(),
        };
        save_settings(&path, &settings).unwrap();
        let saved: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let mut keys: Vec<_> = saved.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(
            keys,
            vec!["listen_mode", "model", "personality", "voice", "wake_phrase"]
        );
        assert_eq!(load_settings(&path), settings);
    }

    #[test]
    fn support_dir_uses_the_bundle_id() {
        let home = PathBuf::from("/Users/example");
        assert_eq!(
            app_support_dir(&home),
            PathBuf::from("/Users/example/Library/Application Support/com.lumi.companion")
        );
        assert_eq!(
            settings_path(&home),
            app_support_dir(&home).join("settings.json")
        );
    }
}
```

`.gitignore`:

```gitignore
/target
/src-tauri/target
/dist
/node_modules
.DS_Store
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p lumi-core --lib settings::`

Expected: FAIL to compile because `load_settings` is not defined.

- [ ] **Step 3: Write minimal implementation**

Replace `crates/lumi-core/src/settings.rs` with the tests from Step 1 below this implementation:

```rust
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Personality {
    Cheerful,
    Calm,
    Study,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VoiceId {
    Amy,
    Lessac,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ListenMode {
    Click,
    Wake,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    pub personality: Personality,
    pub model: String,
    pub voice: VoiceId,
    pub listen_mode: ListenMode,
    pub wake_phrase: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            personality: Personality::Cheerful,
            model: "gemma4:latest".to_string(),
            voice: VoiceId::Amy,
            listen_mode: ListenMode::Click,
            wake_phrase: "hey lumi".to_string(),
        }
    }
}

pub fn app_support_dir(home: &Path) -> PathBuf {
    home.join("Library/Application Support/com.lumi.companion")
}

pub fn settings_path(home: &Path) -> PathBuf {
    app_support_dir(home).join("settings.json")
}

pub fn load_settings(path: &Path) -> Settings {
    let Ok(text) = fs::read_to_string(path) else {
        return Settings::default();
    };
    serde_json::from_str(&text).unwrap_or_else(|_| Settings::default())
}

pub fn save_settings(path: &Path, settings: &Settings) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(settings).expect("settings serialize");
    fs::write(path, text)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p lumi-core --lib settings::`

Expected: PASS, 4 tests.

- [ ] **Step 5: Commit**

```bash
git init
git add Cargo.toml .gitignore crates/lumi-core
git commit -m "$(cat <<'EOF'
feat: store Lumi settings with defaults

EOF
)"
```

---

### Task 2: Personality prompts

**Files:**
- Create: `crates/lumi-core/src/prompts.rs`
- Modify: `crates/lumi-core/src/lib.rs`
- Test: `crates/lumi-core/src/prompts.rs`

**Interfaces:**
- Consumes: `Personality`
- Produces: `system_prompt(personality) -> &'static str`, `speech_rate(personality) -> f32`

- [ ] **Step 1: Write the failing test**

Add `pub mod prompts;` to `lib.rs`. Create `prompts.rs` with only:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Personality;

    #[test]
    fn prompts_match_the_spec() {
        assert_eq!(
            system_prompt(Personality::Cheerful),
            "You are Lumi, a cheerful cartoon kid friend on this computer. Use short, playful, curious sentences and simple words. Replies are spoken aloud, so use plain sentences, no markdown tables, and no long outlines. You only chat. If asked to open an app, read a file, or set a reminder, say you cannot do that yet and answer in words when you can. Stay kind. You do not do romantic or adult roleplay."
        );
        assert_eq!(
            system_prompt(Personality::Calm),
            "You are Lumi, a calm little helper on this computer. Be polite, brief, and clear. Replies are spoken aloud, so use plain sentences, no markdown tables, and no long outlines. You only chat. If asked to open an app, read a file, or set a reminder, say you cannot do that yet and answer in words when you can. Stay kind. You do not do romantic or adult roleplay."
        );
        assert_eq!(
            system_prompt(Personality::Study),
            "You are Lumi, a study buddy on this computer. Explain things simply and offer one example when it helps. Replies are spoken aloud, so use plain sentences, no markdown tables, and no long outlines. You only chat. If asked to open an app, read a file, or set a reminder, say you cannot do that yet and answer in words when you can. Stay kind. You do not do romantic or adult roleplay."
        );
    }

    #[test]
    fn speech_rates_match_the_spec() {
        assert_eq!(speech_rate(Personality::Cheerful), 1.08);
        assert_eq!(speech_rate(Personality::Calm), 0.92);
        assert_eq!(speech_rate(Personality::Study), 1.00);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p lumi-core --lib prompts::`

Expected: FAIL to compile because `system_prompt` is not defined.

- [ ] **Step 3: Write minimal implementation**

Put this above the tests:

```rust
use crate::settings::Personality;

pub fn system_prompt(personality: Personality) -> &'static str {
    match personality {
        Personality::Cheerful => "You are Lumi, a cheerful cartoon kid friend on this computer. Use short, playful, curious sentences and simple words. Replies are spoken aloud, so use plain sentences, no markdown tables, and no long outlines. You only chat. If asked to open an app, read a file, or set a reminder, say you cannot do that yet and answer in words when you can. Stay kind. You do not do romantic or adult roleplay.",
        Personality::Calm => "You are Lumi, a calm little helper on this computer. Be polite, brief, and clear. Replies are spoken aloud, so use plain sentences, no markdown tables, and no long outlines. You only chat. If asked to open an app, read a file, or set a reminder, say you cannot do that yet and answer in words when you can. Stay kind. You do not do romantic or adult roleplay.",
        Personality::Study => "You are Lumi, a study buddy on this computer. Explain things simply and offer one example when it helps. Replies are spoken aloud, so use plain sentences, no markdown tables, and no long outlines. You only chat. If asked to open an app, read a file, or set a reminder, say you cannot do that yet and answer in words when you can. Stay kind. You do not do romantic or adult roleplay.",
    }
}

pub fn speech_rate(personality: Personality) -> f32 {
    match personality {
        Personality::Cheerful => 1.08,
        Personality::Calm => 0.92,
        Personality::Study => 1.00,
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p lumi-core --lib prompts::`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/lumi-core/src/lib.rs crates/lumi-core/src/prompts.rs
git commit -m "$(cat <<'EOF'
feat: add Lumi personality prompts and speech rates

EOF
)"
```

---

### Task 3: Conversation transcript

**Files:**
- Create: `crates/lumi-core/src/conversation.rs`
- Modify: `crates/lumi-core/src/lib.rs`
- Test: `crates/lumi-core/src/conversation.rs`

**Interfaces:**
- Consumes: `Personality`, `system_prompt`
- Produces:
  - `Role::{System, User, Assistant}`
  - `ChatMessage { role, content }`
  - `Tool`
  - `EmptyInput`
  - `Conversation::new(personality) -> Self`
  - `set_personality(&mut self, Personality)`
  - `personality(&self) -> Personality`
  - `messages(&self) -> &[ChatMessage]`
  - `tools(&self) -> &[Tool]`
  - `push_user(&mut self, &str) -> Result<(), EmptyInput>`
  - `start_assistant(&mut self)`
  - `push_assistant_token(&mut self, &str)`
  - `messages_mut(&mut self) -> &mut Vec<ChatMessage>`
  - `request_messages(&self) -> Vec<ChatMessage>`

- [ ] **Step 1: Write the failing test**

Add `pub mod conversation;` and create the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompts::system_prompt;
    use crate::settings::Personality;

    #[test]
    fn empty_and_whitespace_do_not_append() {
        let mut conversation = Conversation::new(Personality::Cheerful);
        assert!(conversation.push_user("").is_err());
        assert!(conversation.push_user("  \n").is_err());
        assert!(conversation.messages().is_empty());
    }

    #[test]
    fn personality_change_keeps_messages_and_swaps_the_system_prompt() {
        let mut conversation = Conversation::new(Personality::Cheerful);
        conversation.push_user("Hi").unwrap();
        conversation.start_assistant();
        conversation.push_assistant_token("Hello");
        conversation.set_personality(Personality::Calm);
        assert_eq!(conversation.messages().len(), 2);
        assert_eq!(conversation.messages()[0].content, "Hi");
        let request = conversation.request_messages();
        assert_eq!(request[0].role, Role::System);
        assert_eq!(request[0].content, system_prompt(Personality::Calm));
        assert_eq!(request[1].content, "Hi");
        assert_eq!(request[2].content, "Hello");
        assert!(conversation.tools().is_empty());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p lumi-core --lib conversation::`

Expected: FAIL to compile because `Conversation` is not defined.

- [ ] **Step 3: Write minimal implementation**

```rust
use crate::prompts::system_prompt;
use crate::settings::Personality;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    System,
    User,
    Assistant,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tool {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmptyInput;

#[derive(Clone, Debug)]
pub struct Conversation {
    personality: Personality,
    messages: Vec<ChatMessage>,
    tools: Vec<Tool>,
}

impl Conversation {
    pub fn new(personality: Personality) -> Self {
        Self {
            personality,
            messages: Vec::new(),
            tools: Vec::new(),
        }
    }

    pub fn set_personality(&mut self, personality: Personality) {
        self.personality = personality;
    }

    pub fn personality(&self) -> Personality {
        self.personality
    }

    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    pub fn tools(&self) -> &[Tool] {
        &self.tools
    }

    pub fn push_user(&mut self, text: &str) -> Result<(), EmptyInput> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err(EmptyInput);
        }
        self.messages.push(ChatMessage {
            role: Role::User,
            content: trimmed.to_string(),
        });
        Ok(())
    }

    pub fn start_assistant(&mut self) {
        if matches!(self.messages.last().map(|message| message.role), Some(Role::Assistant)) {
            return;
        }
        self.messages.push(ChatMessage {
            role: Role::Assistant,
            content: String::new(),
        });
    }

    pub fn push_assistant_token(&mut self, token: &str) {
        self.start_assistant();
        if let Some(message) = self.messages.last_mut() {
            message.content.push_str(token);
        }
    }

    pub fn messages_mut(&mut self) -> &mut Vec<ChatMessage> {
        &mut self.messages
    }

    pub fn request_messages(&self) -> Vec<ChatMessage> {
        let mut messages = vec![ChatMessage {
            role: Role::System,
            content: system_prompt(self.personality).to_string(),
        }];
        messages.extend(self.messages.clone());
        messages
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p lumi-core --lib conversation::`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/lumi-core/src/lib.rs crates/lumi-core/src/conversation.rs
git commit -m "$(cat <<'EOF'
feat: keep the session transcript in memory

EOF
)"
```

---

### Task 4: Model choice

**Files:**
- Create: `crates/lumi-core/src/model_choice.rs`
- Modify: `crates/lumi-core/src/lib.rs`
- Test: `crates/lumi-core/src/model_choice.rs`

**Interfaces:**
- Consumes: nothing from earlier tasks
- Produces: `visible_models(&[String]) -> Vec<String>`, `choose_model(&str, &[String]) -> Option<String>`

- [ ] **Step 1: Write the failing test**

Add `pub mod model_choice;` and this test:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embed_tags_are_hidden() {
        let tags = vec![
            "gemma4:latest".into(),
            "nomic-embed-text:latest".into(),
            "my-embed-model".into(),
        ];
        assert_eq!(visible_models(&tags), vec!["gemma4:latest".to_string()]);
    }

    #[test]
    fn missing_saved_model_falls_through_the_spec_order() {
        assert_eq!(
            choose_model("gone:latest", &["gemma4:latest".into(), "qwen3.6:latest".into()]),
            Some("gemma4:latest".into())
        );
        assert_eq!(
            choose_model("gone:latest", &["qwen3.6:latest".into(), "gemma4:e4b".into()]),
            Some("gemma4:e4b".into())
        );
        assert_eq!(
            choose_model(
                "gone:latest",
                &["nomic-embed-text:latest".into(), "qwen3.6:latest".into()]
            ),
            Some("qwen3.6:latest".into())
        );
        assert_eq!(
            choose_model("gone:latest", &["nomic-embed-text:latest".into()]),
            None
        );
    }

    #[test]
    fn saved_visible_model_wins() {
        assert_eq!(
            choose_model("qwen3.6:latest", &["gemma4:latest".into(), "qwen3.6:latest".into()]),
            Some("qwen3.6:latest".into())
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p lumi-core --lib model_choice::`

Expected: FAIL to compile because `visible_models` is not defined.

- [ ] **Step 3: Write minimal implementation**

```rust
pub fn visible_models(tags: &[String]) -> Vec<String> {
    tags.iter()
        .filter(|name| !name.contains("embed"))
        .cloned()
        .collect()
}

pub fn choose_model(saved: &str, tags: &[String]) -> Option<String> {
    let visible = visible_models(tags);
    if visible.iter().any(|name| name == saved) {
        return Some(saved.to_string());
    }
    if let Some(tag) = visible.iter().find(|name| name.as_str() == "gemma4:latest") {
        return Some(tag.clone());
    }
    if let Some(tag) = visible.iter().find(|name| name.starts_with("gemma4")) {
        return Some(tag.clone());
    }
    visible.first().cloned()
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p lumi-core --lib model_choice::`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/lumi-core/src/lib.rs crates/lumi-core/src/model_choice.rs
git commit -m "$(cat <<'EOF'
feat: choose a visible Ollama chat model

EOF
)"
```

---

### Task 5: One chat turn

**Files:**
- Create: `crates/lumi-core/src/turn.rs`
- Modify: `crates/lumi-core/src/lib.rs`
- Test: `crates/lumi-core/src/turn.rs`

**Interfaces:**
- Consumes: `Conversation`, `ChatMessage`, `Role`
- Produces:
  - `Control::{Continue, Cancel}`
  - `ModelError::{Failed(String), Cancelled}`
  - `trait ModelClient { fn stream(&self, model: &str, messages: &[ChatMessage], on_token: &mut dyn FnMut(&str) -> Control) -> Result<(), ModelError>; }`
  - `TurnOutcome { assistant_text: String, speak: bool, status: Option<String>, cancelled: bool, called_model: bool }`
  - `run_turn(conversation, model, user_text, voice_loaded, cancel: &AtomicBool, client) -> TurnOutcome`
  - `STATUS_COULD_NOT_SPEAK: &str = "She could not speak."`

- [ ] **Step 1: Write the failing test**

Add `pub mod turn;` and tests that use a scripted client:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversation::Conversation;
    use crate::settings::Personality;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    struct Script {
        tokens: Vec<&'static str>,
        fail_at: Option<usize>,
        calls: AtomicUsize,
        cancel_flag: Option<std::sync::Arc<AtomicBool>>,
        cancel_after: Option<usize>,
    }

    impl ModelClient for Script {
        fn stream(
            &self,
            _model: &str,
            _messages: &[crate::conversation::ChatMessage],
            on_token: &mut dyn FnMut(&str) -> Control,
        ) -> Result<(), ModelError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            for (index, token) in self.tokens.iter().enumerate() {
                if self.fail_at == Some(index) {
                    return Err(ModelError::Failed("connection reset".into()));
                }
                if on_token(token) == Control::Cancel {
                    return Err(ModelError::Cancelled);
                }
                if self.cancel_after == Some(index) {
                    if let Some(flag) = &self.cancel_flag {
                        flag.store(true, Ordering::SeqCst);
                    }
                }
            }
            Ok(())
        }
    }

    #[test]
    fn whitespace_does_not_call_the_model() {
        let mut conversation = Conversation::new(Personality::Cheerful);
        let client = Script {
            tokens: vec!["nope"],
            fail_at: None,
            calls: AtomicUsize::new(0),
            cancel_flag: None,
            cancel_after: None,
        };
        let outcome = run_turn(
            &mut conversation,
            "gemma4:latest",
            " \n ",
            true,
            &AtomicBool::new(false),
            &client,
        );
        assert!(!outcome.called_model);
        assert_eq!(client.calls.load(Ordering::SeqCst), 0);
        assert!(conversation.messages().is_empty());
        assert!(!outcome.speak);
    }

    #[test]
    fn cancel_keeps_tokens_already_appended() {
        let flag = std::sync::Arc::new(AtomicBool::new(false));
        let mut conversation = Conversation::new(Personality::Cheerful);
        let client = Script {
            tokens: vec!["Hello", " there", " friend"],
            fail_at: None,
            calls: AtomicUsize::new(0),
            cancel_flag: Some(flag.clone()),
            cancel_after: Some(0),
        };
        let outcome = run_turn(
            &mut conversation,
            "gemma4:latest",
            "Hi",
            true,
            &flag,
            &client,
        );
        assert!(outcome.cancelled);
        assert_eq!(outcome.assistant_text, "Hello");
        assert!(!outcome.speak);
        assert_eq!(conversation.messages()[1].content, "Hello");
    }

    #[test]
    fn mid_stream_error_keeps_partial_text() {
        let mut conversation = Conversation::new(Personality::Calm);
        let client = Script {
            tokens: vec!["Sure", " thing"],
            fail_at: Some(1),
            calls: AtomicUsize::new(0),
            cancel_flag: None,
            cancel_after: None,
        };
        let outcome = run_turn(
            &mut conversation,
            "gemma4:latest",
            "Help",
            true,
            &AtomicBool::new(false),
            &client,
        );
        assert_eq!(outcome.assistant_text, "Sure");
        assert_eq!(outcome.status.as_deref(), Some("connection reset"));
        assert!(!outcome.speak);
        assert!(!outcome.cancelled);
    }

    #[test]
    fn missing_voice_keeps_the_text_and_sets_status() {
        let mut conversation = Conversation::new(Personality::Study);
        let client = Script {
            tokens: vec!["One example."],
            fail_at: None,
            calls: AtomicUsize::new(0),
            cancel_flag: None,
            cancel_after: None,
        };
        let outcome = run_turn(
            &mut conversation,
            "gemma4:latest",
            "Explain",
            false,
            &AtomicBool::new(false),
            &client,
        );
        assert_eq!(outcome.assistant_text, "One example.");
        assert!(!outcome.speak);
        assert_eq!(outcome.status.as_deref(), Some(STATUS_COULD_NOT_SPEAK));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p lumi-core --lib turn::`

Expected: FAIL to compile because `run_turn` is not defined.

- [ ] **Step 3: Write minimal implementation**

`run_turn` trims through `push_user`. On `EmptyInput`, return `called_model: false` and leave the conversation unchanged. Otherwise start an assistant message and stream. The token callback returns `Control::Cancel` when `cancel` is already true. After `Cancel`, drop a trailing empty assistant message if no token arrived. On `Failed`, keep received tokens, copy the error into `status`, and set `speak` false. On success, `speak` is `voice_loaded`. When `voice_loaded` is false, set `status` to `STATUS_COULD_NOT_SPEAK`.

```rust
use crate::conversation::{ChatMessage, Conversation, EmptyInput};
use std::sync::atomic::{AtomicBool, Ordering};

pub const STATUS_COULD_NOT_SPEAK: &str = "She could not speak.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    Continue,
    Cancel,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelError {
    Failed(String),
    Cancelled,
}

pub trait ModelClient {
    fn stream(
        &self,
        model: &str,
        messages: &[ChatMessage],
        on_token: &mut dyn FnMut(&str) -> Control,
    ) -> Result<(), ModelError>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TurnOutcome {
    pub assistant_text: String,
    pub speak: bool,
    pub status: Option<String>,
    pub cancelled: bool,
    pub called_model: bool,
}

pub fn run_turn(
    conversation: &mut Conversation,
    model: &str,
    user_text: &str,
    voice_loaded: bool,
    cancel: &AtomicBool,
    client: &dyn ModelClient,
) -> TurnOutcome {
    if conversation.push_user(user_text).is_err() {
        let _ = EmptyInput;
        return TurnOutcome {
            assistant_text: String::new(),
            speak: false,
            status: None,
            cancelled: false,
            called_model: false,
        };
    }

    conversation.start_assistant();
    let mut messages = conversation.request_messages();
    if messages.last().is_some_and(|message| {
        message.role == crate::conversation::Role::Assistant && message.content.is_empty()
    }) {
        messages.pop();
    }
    let result = client.stream(model, &messages, &mut |token| {
        if cancel.load(Ordering::SeqCst) {
            return Control::Cancel;
        }
        conversation.push_assistant_token(token);
        if cancel.load(Ordering::SeqCst) {
            Control::Cancel
        } else {
            Control::Continue
        }
    });

    let assistant_text = conversation
        .messages()
        .last()
        .filter(|message| message.role == crate::conversation::Role::Assistant)
        .map(|message| message.content.clone())
        .unwrap_or_default();

    match result {
        Ok(()) => TurnOutcome {
            assistant_text,
            speak: voice_loaded,
            status: if voice_loaded {
                None
            } else {
                Some(STATUS_COULD_NOT_SPEAK.to_string())
            },
            cancelled: false,
            called_model: true,
        },
        Err(ModelError::Cancelled) => {
            if assistant_text.is_empty() {
                conversation.messages_mut().pop();
            }
            TurnOutcome {
                assistant_text,
                speak: false,
                status: None,
                cancelled: true,
                called_model: true,
            }
        }
        Err(ModelError::Failed(message)) => TurnOutcome {
            assistant_text,
            speak: false,
            status: Some(message),
            cancelled: false,
            called_model: true,
        },
    }
}
```

`messages_mut` is already on `Conversation` from Task 3. The cancel test sets the flag after token 0, so token 1 is refused before it is stored. The stored assistant text is `Hello`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p lumi-core --lib turn::`

Expected: PASS, including the whitespace, cancel, mid-stream error, and missing-voice tests.

- [ ] **Step 5: Commit**

```bash
git add crates/lumi-core/src/lib.rs crates/lumi-core/src/turn.rs crates/lumi-core/src/conversation.rs
git commit -m "$(cat <<'EOF'
feat: run one cancellable chat turn

EOF
)"
```

---

### Task 6: Ollama client

**Files:**
- Create: `crates/lumi-core/src/ollama.rs`
- Modify: `crates/lumi-core/Cargo.toml`
- Modify: `crates/lumi-core/src/lib.rs`
- Test: `crates/lumi-core/src/ollama.rs`

**Interfaces:**
- Consumes: `ModelClient`, `ChatMessage`, `Role`, `Control`, `ModelError`
- Produces:
  - `OLLAMA_BASE: &str = "http://127.0.0.1:11434"`
  - `OllamaClient::new(base: &str) -> Self`
  - `OllamaClient::tags(&self) -> Result<Vec<String>, String>`
  - `impl ModelClient for OllamaClient`
  - `ollama_serve_command() -> std::process::Command`

- [ ] **Step 1: Write the failing test**

Add reqwest to `crates/lumi-core/Cargo.toml`:

```toml
reqwest = { version = "0.12.23", default-features = false, features = ["blocking", "json", "rustls-tls"] }
```

Add `pub mod ollama;`. Test against a one-shot local HTTP server:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversation::{ChatMessage, Role};
    use crate::turn::{Control, ModelClient};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};
    use std::thread;

    fn serve(body: &'static str, captured: Arc<Mutex<String>>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
        let mut request = Vec::new();
        let mut buf = [0_u8; 8192];
        let mut header_end = 0_usize;
        loop {
            let read = socket.read(&mut buf).unwrap();
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buf[..read]);
            if let Some(pos) = request.windows(4).position(|window| window == b"\r\n\r\n") {
                header_end = pos + 4;
                break;
            }
        }
        let header = String::from_utf8_lossy(&request[..header_end]).to_lowercase();
        let length = header
            .lines()
            .find_map(|line| line.strip_prefix("content-length:"))
            .and_then(|value| value.trim().parse::<usize>().ok())
            .unwrap_or(0);
        while request.len() < header_end + length {
            let read = socket.read(&mut buf).unwrap();
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buf[..read]);
        }
        *captured.lock().unwrap() = String::from_utf8_lossy(&request).into_owned();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            socket.write_all(response.as_bytes()).unwrap();
        });
        format!("http://127.0.0.1:{port}")
    }

    #[test]
    fn tags_read_model_names() {
        let captured = Arc::new(Mutex::new(String::new()));
        let base = serve(
            r#"{"models":[{"name":"gemma4:latest"},{"name":"nomic-embed-text:latest"}]}"#,
            captured,
        );
        let names = OllamaClient::new(&base).tags().unwrap();
        assert_eq!(names, vec!["gemma4:latest", "nomic-embed-text:latest"]);
    }

    #[test]
    fn chat_stream_emits_tokens_and_omits_tools() {
        let captured = Arc::new(Mutex::new(String::new()));
        let body = "{\"message\":{\"content\":\"Hi\"},\"done\":false}\n{\"message\":{\"content\":\" there\"},\"done\":true}\n";
        let base = serve(body, captured.clone());
        let client = OllamaClient::new(&base);
        let messages = vec![
            ChatMessage { role: Role::System, content: "sys".into() },
            ChatMessage { role: Role::User, content: "Hi".into() },
        ];
        let mut got = String::new();
        client
            .stream("gemma4:latest", &messages, &mut |token| {
                got.push_str(token);
                Control::Continue
            })
            .unwrap();
        assert_eq!(got, "Hi there");
        let request = captured.lock().unwrap().clone();
        assert!(!request.contains("\"tools\""));
        assert!(request.contains("\"stream\":true"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p lumi-core --lib ollama::`

Expected: FAIL to compile because `OllamaClient` is not defined.

- [ ] **Step 3: Write minimal implementation**

```rust
use crate::conversation::{ChatMessage, Role};
use crate::turn::{Control, ModelClient, ModelError};
use reqwest::blocking::Client;
use serde::Deserialize;
use std::io::{BufRead, BufReader};
use std::process::Command;
use std::time::Duration;

pub const OLLAMA_BASE: &str = "http://127.0.0.1:11434";

pub struct OllamaClient {
    base: String,
    http: Client,
}

#[derive(Deserialize)]
struct TagsResponse {
    models: Vec<TagModel>,
}

#[derive(Deserialize)]
struct TagModel {
    name: String,
}

#[derive(Deserialize)]
struct StreamLine {
    message: Option<StreamMessage>,
    done: bool,
}

#[derive(Deserialize)]
struct StreamMessage {
    content: String,
}

impl OllamaClient {
    pub fn new(base: &str) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(600))
            .build()
            .expect("http client");
        Self {
            base: base.trim_end_matches('/').to_string(),
            http,
        }
    }

    pub fn tags(&self) -> Result<Vec<String>, String> {
        let response = self
            .http
            .get(format!("{}/api/tags", self.base))
            .send()
            .map_err(|error| error.to_string())?;
        let body: TagsResponse = response.json().map_err(|error| error.to_string())?;
        Ok(body.models.into_iter().map(|model| model.name).collect())
    }
}

impl ModelClient for OllamaClient {
    fn stream(
        &self,
        model: &str,
        messages: &[ChatMessage],
        on_token: &mut dyn FnMut(&str) -> Control,
    ) -> Result<(), ModelError> {
        let payload = serde_json::json!({
            "model": model,
            "stream": true,
            "messages": messages.iter().map(|message| {
                let role = match message.role {
                    Role::System => "system",
                    Role::User => "user",
                    Role::Assistant => "assistant",
                };
                serde_json::json!({"role": role, "content": message.content})
            }).collect::<Vec<_>>()
        });
        let response = self
            .http
            .post(format!("{}/api/chat", self.base))
            .json(&payload)
            .send()
            .map_err(|error| ModelError::Failed(error.to_string()))?;
        if !response.status().is_success() {
            return Err(ModelError::Failed(format!("Ollama returned {}", response.status())));
        }
        let reader = BufReader::new(response);
        for line in reader.lines() {
            let line = line.map_err(|error| ModelError::Failed(error.to_string()))?;
            if line.trim().is_empty() {
                continue;
            }
            let parsed: StreamLine =
                serde_json::from_str(&line).map_err(|error| ModelError::Failed(error.to_string()))?;
            if let Some(message) = parsed.message {
                if !message.content.is_empty() && on_token(&message.content) == Control::Cancel {
                    return Err(ModelError::Cancelled);
                }
            }
            if parsed.done {
                break;
            }
        }
        Ok(())
    }
}

pub fn ollama_serve_command() -> Command {
    let mut command = Command::new("ollama");
    command.arg("serve");
    command
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p lumi-core --lib ollama::`

Expected: PASS. Both tests see a full request. The chat test asserts the body has no `tools` key.

- [ ] **Step 5: Commit**

```bash
git add crates/lumi-core/Cargo.toml crates/lumi-core/src/lib.rs crates/lumi-core/src/ollama.rs
git commit -m "$(cat <<'EOF'
feat: stream chat from the local Ollama API

EOF
)"
```

---

### Task 7: Speech pack files

**Files:**
- Create: `crates/lumi-core/src/packs.rs`
- Modify: `crates/lumi-core/Cargo.toml`
- Modify: `crates/lumi-core/src/lib.rs`
- Test: `crates/lumi-core/src/packs.rs`

**Interfaces:**
- Consumes: nothing
- Produces:
  - `SpeechPack { id, label, url, required }`
  - `speech_packs() -> &'static [SpeechPack]`
  - `missing_files(models_dir, pack) -> Vec<PathBuf>`
  - `extract_tar_bz2(bytes, dest_dir) -> Result<(), String>`
  - `download_bytes(url, &reqwest::blocking::Client) -> Result<Vec<u8>, String>`

- [ ] **Step 1: Write the failing test**

Add `tar = "0.4"` and `bzip2 = "0.6"` to the core crate. Add `pub mod packs;`.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn packs_use_the_spec_archives_and_files() {
        let packs = speech_packs();
        assert_eq!(packs.len(), 3);
        assert_eq!(
            packs[0].url,
            "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-tiny.en.tar.bz2"
        );
        assert!(packs[0].required.contains(&"sherpa-onnx-whisper-tiny.en/tiny.en-encoder.int8.onnx"));
        assert!(packs[0].required.contains(&"sherpa-onnx-whisper-tiny.en/tiny.en-decoder.int8.onnx"));
        assert!(packs[0].required.contains(&"sherpa-onnx-whisper-tiny.en/tiny.en-tokens.txt"));
        assert!(packs[1].url.contains("vits-piper-en_US-amy-medium.tar.bz2"));
        assert!(packs[1].required.contains(&"vits-piper-en_US-amy-medium/en_US-amy-medium.onnx"));
        assert!(packs[1].required.contains(&"vits-piper-en_US-amy-medium/espeak-ng-data/phontab"));
        assert!(packs[2].url.contains("vits-piper-en_US-lessac-medium.tar.bz2"));
        assert!(packs[2].required.contains(&"vits-piper-en_US-lessac-medium/en_US-lessac-medium.onnx"));
    }

    #[test]
    fn missing_files_lists_only_what_is_absent() {
        let dir = std::env::temp_dir().join(format!("lumi-packs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sherpa-onnx-whisper-tiny.en")).unwrap();
        std::fs::write(
            dir.join("sherpa-onnx-whisper-tiny.en/tiny.en-tokens.txt"),
            "tok",
        )
        .unwrap();
        let missing = missing_files(&dir, &speech_packs()[0]);
        assert_eq!(missing.len(), 2);
        assert!(missing.iter().all(|path| !path.ends_with("tiny.en-tokens.txt")));
    }

    #[test]
    fn extract_tar_bz2_writes_the_archive_entries() {
        let mut builder = tar::Builder::new(Vec::new());
        let mut header = tar::Header::new_gnu();
        let bytes = b"hello";
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append_data(&mut header, "demo/file.txt", &bytes[..]).unwrap();
        let tar_bytes = builder.into_inner().unwrap();
        let mut compressed = Vec::new();
        {
            let mut encoder = bzip2::write::BzEncoder::new(&mut compressed, bzip2::Compression::fast());
            encoder.write_all(&tar_bytes).unwrap();
            encoder.finish().unwrap();
        }
        let dest = std::env::temp_dir().join(format!("lumi-extract-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dest);
        extract_tar_bz2(&compressed, &dest).unwrap();
        assert_eq!(std::fs::read_to_string(dest.join("demo/file.txt")).unwrap(), "hello");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p lumi-core --lib packs::`

Expected: FAIL to compile because `speech_packs` is not defined.

- [ ] **Step 3: Write minimal implementation**

```rust
use bzip2::read::BzDecoder;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

pub struct SpeechPack {
    pub id: &'static str,
    pub label: &'static str,
    pub url: &'static str,
    pub required: &'static [&'static str],
}

pub fn speech_packs() -> &'static [SpeechPack] {
    const PACKS: &[SpeechPack] = &[
        SpeechPack {
            id: "whisper",
            label: "Whisper Tiny English",
            url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-tiny.en.tar.bz2",
            required: &[
                "sherpa-onnx-whisper-tiny.en/tiny.en-encoder.int8.onnx",
                "sherpa-onnx-whisper-tiny.en/tiny.en-decoder.int8.onnx",
                "sherpa-onnx-whisper-tiny.en/tiny.en-tokens.txt",
            ],
        },
        SpeechPack {
            id: "amy",
            label: "Piper Amy",
            url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/vits-piper-en_US-amy-medium.tar.bz2",
            required: &[
                "vits-piper-en_US-amy-medium/en_US-amy-medium.onnx",
                "vits-piper-en_US-amy-medium/tokens.txt",
                "vits-piper-en_US-amy-medium/espeak-ng-data/phontab",
            ],
        },
        SpeechPack {
            id: "lessac",
            label: "Piper Lessac",
            url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/vits-piper-en_US-lessac-medium.tar.bz2",
            required: &[
                "vits-piper-en_US-lessac-medium/en_US-lessac-medium.onnx",
                "vits-piper-en_US-lessac-medium/tokens.txt",
                "vits-piper-en_US-lessac-medium/espeak-ng-data/phontab",
            ],
        },
    ];
    PACKS
}

pub fn missing_files(models_dir: &Path, pack: &SpeechPack) -> Vec<PathBuf> {
    pack.required
        .iter()
        .map(|relative| models_dir.join(relative))
        .filter(|path| !path.is_file())
        .collect()
}

pub fn extract_tar_bz2(bytes: &[u8], dest_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dest_dir).map_err(|error| error.to_string())?;
    let decoder = BzDecoder::new(bytes);
    let mut archive = tar::Archive::new(decoder);
    archive.unpack(dest_dir).map_err(|error| error.to_string())
}

pub fn download_bytes(url: &str, client: &reqwest::blocking::Client) -> Result<Vec<u8>, String> {
    let mut response = client.get(url).send().map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("download failed: {}", response.status()));
    }
    let mut bytes = Vec::new();
    response.read_to_end(&mut bytes).map_err(|error| error.to_string())?;
    Ok(bytes)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p lumi-core --lib packs::`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/lumi-core
git commit -m "$(cat <<'EOF'
feat: name the local Whisper and Piper files

EOF
)"
```

---

### Task 8: Audio levels

**Files:**
- Create: `crates/lumi-core/src/audio.rs`
- Modify: `crates/lumi-core/src/lib.rs`
- Test: `crates/lumi-core/src/audio.rs`

**Interfaces:**
- Consumes: nothing
- Produces:
  - `mouth_level(&[f32]) -> u8`
  - `frame_mouth_levels(&[f32], frame_len: usize) -> Vec<u8>`
  - `resample_linear(&[f32], from_hz: u32, to_hz: u32) -> Vec<f32>`
  - `take_max_duration(&[f32], sample_rate: u32, max_secs: u32) -> &[f32]`

- [ ] **Step 1: Write the failing test**

Add `pub mod audio;` and:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mouth_level_uses_four_steps() {
        assert_eq!(mouth_level(&[0.0, 0.0, 0.0]), 0);
        assert_eq!(mouth_level(&[0.04, -0.04, 0.04]), 1);
        assert_eq!(mouth_level(&[0.09, -0.09]), 2);
        assert_eq!(mouth_level(&[0.4, -0.4, 0.4]), 3);
    }

    #[test]
    fn frames_map_one_level_each() {
        let samples = vec![0.0, 0.0, 0.4, -0.4];
        assert_eq!(frame_mouth_levels(&samples, 2), vec![0, 3]);
    }

    #[test]
    fn resample_halves_a_constant_signal() {
        let input = vec![0.5_f32; 8];
        let output = resample_linear(&input, 16000, 8000);
        assert_eq!(output.len(), 4);
        assert!(output.iter().all(|sample| (*sample - 0.5).abs() < 0.001));
    }

    #[test]
    fn duration_cap_keeps_thirty_seconds() {
        let samples = vec![0.1_f32; 48_000];
        let capped = take_max_duration(&samples, 16_000, 30);
        assert_eq!(capped.len(), 48_000);
        let long = vec![0.1_f32; 160_000];
        assert_eq!(take_max_duration(&long, 16_000, 30).len(), 480_000.min(160_000));
        assert_eq!(take_max_duration(&long, 16_000, 1).len(), 16_000);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p lumi-core --lib audio::`

Expected: FAIL to compile because `mouth_level` is not defined.

- [ ] **Step 3: Write minimal implementation**

```rust
pub fn mouth_level(samples: &[f32]) -> u8 {
    if samples.is_empty() {
        return 0;
    }
    let energy: f32 = samples.iter().map(|sample| sample * sample).sum();
    let rms = (energy / samples.len() as f32).sqrt();
    if rms < 0.02 {
        0
    } else if rms < 0.06 {
        1
    } else if rms < 0.12 {
        2
    } else {
        3
    }
}

pub fn frame_mouth_levels(samples: &[f32], frame_len: usize) -> Vec<u8> {
    if frame_len == 0 {
        return Vec::new();
    }
    samples
        .chunks(frame_len)
        .map(mouth_level)
        .collect()
}

pub fn resample_linear(input: &[f32], from_hz: u32, to_hz: u32) -> Vec<f32> {
    if input.is_empty() || from_hz == 0 || to_hz == 0 {
        return Vec::new();
    }
    if from_hz == to_hz {
        return input.to_vec();
    }
    let out_len = ((input.len() as u64) * (to_hz as u64) / (from_hz as u64)) as usize;
    let mut output = Vec::with_capacity(out_len.max(1));
    for index in 0..out_len {
        let position = index as f32 * from_hz as f32 / to_hz as f32;
        let left = position.floor() as usize;
        let right = (left + 1).min(input.len() - 1);
        let mix = position - left as f32;
        output.push(input[left] * (1.0 - mix) + input[right] * mix);
    }
    output
}

pub fn take_max_duration(samples: &[f32], sample_rate: u32, max_secs: u32) -> &[f32] {
    let max_len = (sample_rate as usize).saturating_mul(max_secs as usize);
    if samples.len() <= max_len {
        samples
    } else {
        &samples[..max_len]
    }
}
```

Check the RMS of the test vectors before keeping the thresholds. `[0.04, -0.04, 0.04]` has RMS 0.04, which is level 1. `[0.09, -0.09]` has RMS 0.09, which is level 2. `[0.4, -0.4, 0.4]` has RMS 0.4, which is level 3. Those match the thresholds.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p lumi-core --lib audio::`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/lumi-core/src/lib.rs crates/lumi-core/src/audio.rs
git commit -m "$(cat <<'EOF'
feat: measure mouth shape and cap recordings

EOF
)"
```

---

### Task 9: App state

**Files:**
- Create: `crates/lumi-core/src/app.rs`
- Modify: `crates/lumi-core/src/lib.rs`
- Test: `crates/lumi-core/src/app.rs`

**Interfaces:**
- Consumes: `Settings`, `Conversation`, `VoiceId`, `ListenMode`, `Personality`, `choose_model`, `run_turn`, `TurnOutcome`, `ModelClient`
- Produces:
  - `Pose::{Idle, Listening, Thinking, Speaking}`
  - `PublicState` with `settings`, `pose`, `mouth`, `status`, `transcript`, `amy_loaded`, `lessac_loaded`, `whisper_loaded`, `mic_allowed`, `models`, `recording`
  - `App::new(Settings) -> Self`
  - `public_state(&self) -> PublicState`
  - `set_personality`, `set_model`, `set_voice -> Result<(), String>`
  - `active_listen_mode(&self) -> ListenMode`
  - `resolved_model(&self) -> Option<String>`
  - `submit_text(&mut self, &str, &dyn ModelClient) -> TurnOutcome`
  - `begin_listen(&mut self) -> Result<(), String>`
  - `finish_playback(&mut self)`
  - `mark_whisper`, `mark_voice`, `mark_mic`, `set_tags`
  - `note_download_failure(&mut self, label: &str)`
  - `cancel(&self)`, `cancel_flag(&self) -> Arc<AtomicBool>`, `hear_nothing(&mut self)`
  - `STATUS_OLLAMA_DOWN`, `STATUS_NO_MODEL`, `STATUS_MIC`, `STATUS_EMPTY_HEARING`

- [ ] **Step 1: Write the failing test**

Add `pub mod app;`. Cover the wake-mode review case and the failure strings:

```rust
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
    fn personality_change_does_not_rewrite_the_transcript() {
        let mut app = App::new(Settings::default());
        app.set_tags(vec!["gemma4:latest".into()]);
        app.mark_voice(VoiceId::Amy, true);
        app.submit_text("Hello", &Counting(AtomicUsize::new(0)));
        let before = app.public_state().transcript.len();
        app.set_personality(Personality::Study);
        assert_eq!(app.public_state().transcript.len(), before);
        assert_eq!(app.public_state().settings.personality, Personality::Study);
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
    fn unloaded_voice_cannot_be_selected() {
        let mut app = App::new(Settings::default());
        assert!(app.set_voice(VoiceId::Lessac).is_err());
        app.mark_voice(VoiceId::Lessac, true);
        app.set_voice(VoiceId::Lessac).unwrap();
        assert_eq!(app.public_state().settings.voice, VoiceId::Lessac);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p lumi-core --lib app::`

Expected: FAIL to compile because `App` is not defined.

- [ ] **Step 3: Write minimal implementation**

Add the Ollama failure test next to the other tests in `app.rs`, then write the module below. `submit_text` stays the synchronous path the unit tests call. Task 11 adds `prepare_turn` and `complete_turn` beside it for the window, which needs to emit tokens while the mutex is released.

```rust
#[test]
fn ollama_failure_sets_the_unavailable_status() {
    let mut app = App::new(Settings::default());
    apply_tag_result(&mut app, Err("connection refused".into()));
    assert_eq!(app.public_state().status, "Ollama is unavailable.");
}
```

```rust
use crate::conversation::{Conversation, Role};
use crate::model_choice::{choose_model, visible_models};
use crate::settings::{ListenMode, Personality, Settings, VoiceId};
use crate::turn::{run_turn, ModelClient, TurnOutcome};
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
    pub amy_loaded: bool,
    pub lessac_loaded: bool,
    pub whisper_loaded: bool,
    pub mic_allowed: bool,
    pub models: Vec<String>,
    pub recording: bool,
}

pub struct App {
    settings: Settings,
    conversation: Conversation,
    pose: Pose,
    mouth: u8,
    status: String,
    whisper_loaded: bool,
    amy_loaded: bool,
    lessac_loaded: bool,
    mic_allowed: bool,
    tags: Vec<String>,
    recording: bool,
    cancel: Arc<AtomicBool>,
}

impl App {
    pub fn new(settings: Settings) -> Self {
        let personality = settings.personality;
        Self {
            settings,
            conversation: Conversation::new(personality),
            pose: Pose::Idle,
            mouth: 0,
            status: String::new(),
            whisper_loaded: false,
            amy_loaded: false,
            lessac_loaded: false,
            mic_allowed: false,
            tags: Vec::new(),
            recording: false,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn public_state(&self) -> PublicState {
        PublicState {
            settings: self.settings.clone(),
            pose: self.pose,
            mouth: self.mouth,
            status: self.status.clone(),
            transcript: self.conversation.messages().iter().map(|message| PublicMessage {
                role: match message.role {
                    Role::User => "user".into(),
                    Role::Assistant => "assistant".into(),
                    Role::System => "system".into(),
                },
                content: message.content.clone(),
            }).collect(),
            amy_loaded: self.amy_loaded,
            lessac_loaded: self.lessac_loaded,
            whisper_loaded: self.whisper_loaded,
            mic_allowed: self.mic_allowed,
            models: visible_models(&self.tags),
            recording: self.recording,
        }
    }

    pub fn set_status(&mut self, status: impl Into<String>) {
        self.status = status.into();
    }

    pub fn set_personality(&mut self, personality: Personality) {
        self.settings.personality = personality;
        self.conversation.set_personality(personality);
    }

    pub fn set_model(&mut self, model: String) {
        self.settings.model = model;
    }

    pub fn set_voice(&mut self, voice: VoiceId) -> Result<(), String> {
        let loaded = match voice {
            VoiceId::Amy => self.amy_loaded,
            VoiceId::Lessac => self.lessac_loaded,
        };
        if !loaded {
            return Err("That voice is still loading.".into());
        }
        self.settings.voice = voice;
        Ok(())
    }

    pub fn active_listen_mode(&self) -> ListenMode {
        ListenMode::Click
    }

    pub fn resolved_model(&self) -> Option<String> {
        choose_model(&self.settings.model, &self.tags)
    }

    pub fn voice_loaded(&self) -> bool {
        match self.settings.voice {
            VoiceId::Amy => self.amy_loaded,
            VoiceId::Lessac => self.lessac_loaded,
        }
    }

    pub fn submit_text(&mut self, text: &str, client: &dyn ModelClient) -> TurnOutcome {
        self.cancel.store(true, Ordering::SeqCst);
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel = cancel.clone();
        let Some(model) = self.resolved_model() else {
            self.status = STATUS_NO_MODEL.into();
            return TurnOutcome {
                assistant_text: String::new(),
                speak: false,
                status: Some(self.status.clone()),
                cancelled: false,
                called_model: false,
            };
        };
        self.recording = false;
        self.pose = Pose::Thinking;
        let outcome = run_turn(
            &mut self.conversation,
            &model,
            text,
            self.voice_loaded(),
            cancel.as_ref(),
            client,
        );
        if let Some(status) = &outcome.status {
            self.status = status.clone();
        }
        self.pose = if outcome.speak { Pose::Speaking } else { Pose::Idle };
        self.mouth = 0;
        outcome
    }

    pub fn begin_listen(&mut self) -> Result<(), String> {
        if !self.mic_allowed {
            self.status = STATUS_MIC.into();
            return Err(self.status.clone());
        }
        if !self.whisper_loaded {
            return Err("Whisper is still loading.".into());
        }
        self.cancel.store(true, Ordering::SeqCst);
        self.recording = true;
        self.pose = Pose::Listening;
        Ok(())
    }

    pub fn finish_playback(&mut self) {
        self.pose = Pose::Idle;
        self.mouth = 0;
    }

    pub fn mark_whisper(&mut self, loaded: bool) {
        self.whisper_loaded = loaded;
    }

    pub fn mark_voice(&mut self, voice: VoiceId, loaded: bool) {
        match voice {
            VoiceId::Amy => self.amy_loaded = loaded,
            VoiceId::Lessac => self.lessac_loaded = loaded,
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

    pub fn cancel_flag(&self) -> Arc<AtomicBool> {
        self.cancel.clone()
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
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
            }
        }
        Err(_) => app.set_status(STATUS_OLLAMA_DOWN),
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p lumi-core --lib`

Expected: PASS, the whole core library, including the wake-mode test.

- [ ] **Step 5: Commit**

```bash
git add crates/lumi-core/src/lib.rs crates/lumi-core/src/app.rs
git commit -m "$(cat <<'EOF'
feat: keep Lumi's pose and listen mode in app state

EOF
)"
```

---

### Task 10: Window and cartoon

**Files:**
- Create: `package.json`, `tsconfig.json`, `vite.config.ts`, `index.html`
- Create: `src/main.ts`, `src/styles.css`
- Create: `src-tauri/Cargo.toml`, `src-tauri/build.rs`, `src-tauri/tauri.conf.json`
- Create: `src-tauri/capabilities/default.json`, `src-tauri/Info.plist`
- Create: `src-tauri/src/main.rs`, `src-tauri/src/lib.rs`
- Modify: `Cargo.toml` workspace members

**Interfaces:**
- Consumes: the page will later call `get_state`, `send_message`, `start_listening`, `stop_listening`, `set_personality`, `set_model`, `set_voice`, `retry_download`
- Produces: a window titled Lumi, identifier `com.lumi.companion`, size 960×640, with the SVG character and the controls. No Wake word control.

- [ ] **Step 1: Write the failing build**

Add `src-tauri` to the workspace members. Create the files below with the character and controls, but leave `src/main.ts` importing `./missing` so the frontend build fails.

`package.json`:

```json
{
  "name": "lumi",
  "private": true,
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "tauri": "tauri"
  },
  "dependencies": {
    "@tauri-apps/api": "2.12.4"
  },
  "devDependencies": {
    "@tauri-apps/cli": "2.12.4",
    "typescript": "5.9.3",
    "vite": "8.3.5"
  }
}
```

`vite.config.ts` uses `server.port = 1420` and `strictPort: true`. `tsconfig.json` includes `src` and emits no files (`noEmit` is not required because `tsc && vite build` needs a successful typecheck; set `"noEmit": true`).

`src-tauri/Cargo.toml` depends on `tauri = "2.12.5"` and `lumi-core = { path = "../crates/lumi-core" }`. Build dependency `tauri-build = "2.7.4"`. Lib name `lumi_app`, crate type `staticlib`, `cdylib`, `rlib`. Package name `lumi-app`.

`src-tauri/src/main.rs` calls `lumi_app::run()`. `run()` builds a Tauri window and does not yet register speech.

`tauri.conf.json` identifier is `com.lumi.companion`, window label `main`, title `Lumi`, width 960, height 640, `frontendDist` `../dist`, `devUrl` `http://localhost:1420`. `bundle.macOS.infoPlist` is `Info.plist`.

`Info.plist` contains exactly:

```xml
<key>NSMicrophoneUsageDescription</key>
<string>Lumi uses the microphone so you can talk to her.</string>
```

`index.html` is a two-column page: `#character` (280px) and `#panel`. The panel has `#download`, `#transcript`, `#status`, a personality `<select id="personality">` with values `cheerful`, `calm`, `study`, a model `<select id="model">`, a voice `<select id="voice">` with `amy` and `lessac`, `<button id="mic" type="button">Listen</button>`, and `<form id="composer"><input id="text" /></form>`. There is no wake-word control.

The SVG inside `#character` uses class `pose-idle` and these parts: `.figure`, `.hair`, `.eye`, `.eyelid`, `.pupil`, `.mouth`, `.sweater`, `.skirt`. It is an original drawing: large head, short dark bob, large eyes, flat sweater and skirt. `src/styles.css` blinks `.eyelid` on a 4 second animation, sways `.pose-idle .figure`, leans `.pose-listening .figure`, and shifts `.pose-thinking .pupil` upward. `.mouth` uses `[data-mouth="0"]` through `[data-mouth="3"]` to change height.

- [ ] **Step 2: Run build to verify it fails**

Run: `npm install` then `npm run build`

Expected: FAIL because `src/main.ts` imports a missing module.

- [ ] **Step 3: Write the page script**

`src/main.ts` imports `{ invoke } from "@tauri-apps/api/core"` and `{ listen } from "@tauri-apps/api/event"`. On load it calls `get_state` and renders. It listens for `token`, `turn_done`, `pose`, `mouth`, `status`, and `download_progress`.

Rendering rules:

- Pose event replaces the SVG root class with `pose-idle`, `pose-listening`, `pose-thinking`, or `pose-speaking`.
- Mouth event sets `data-mouth` to the level.
- Token appends to the last assistant row, creating one if needed.
- `turn_done` finishes that row.
- Status replaces `#status`.
- Download progress shows `#download` as `{pack} {percent}%` and hides it at 100 with no failures.
- Mic click calls `start_listening` when the label is Listen and `stop_listening` when the label is Stop.
- Mic is disabled when `whisper_loaded` is false or `mic_allowed` is false.
- A voice option is disabled when that voice is not loaded.
- Enter in the form calls `preventDefault` and `send_message` with the input text, then clears the input.
- Selector changes call `set_personality`, `set_model`, or `set_voice`.

`get_state` shape matches `PublicState` serialized in snake_case field names from Task 9. Add `serde::Serialize` to `PublicState` and the enums if Task 9 did not already derive it. Derive `Serialize` on `Pose` with `rename_all = "lowercase"`.

`src-tauri/src/lib.rs` `run()` manages an `Arc<Mutex<App>>` created from `load_settings`, and registers `get_state` only. `get_state` returns `app.public_state()`. Other commands are added in Task 11. The page may call them after Task 11; until then, only opening the window and rendering the initial state is required. Guard the invoke calls so a missing command shows the error on `#status` instead of throwing an empty page.

- [ ] **Step 4: Run build to verify it passes**

Run: `npm run build` and `cargo test -p lumi-core --lib`

Expected: both succeed. `dist/index.html` exists. The built page source contains `NSMicrophoneUsageDescription` only in `Info.plist`, and the HTML contains no control labeled Wake.

- [ ] **Step 5: Commit**

```bash
git add package.json package-lock.json tsconfig.json vite.config.ts index.html src src-tauri Cargo.toml
git commit -m "$(cat <<'EOF'
feat: open the Lumi window and cartoon

EOF
)"
```

---

### Task 11: Typed chat and downloads

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `crates/lumi-core/src/app.rs`
- Modify: `src/main.ts`
- Test: `crates/lumi-core/src/app.rs`

**Interfaces:**
- Consumes: `App`, `OllamaClient`, `OLLAMA_BASE`, `ollama_serve_command`, `apply_tag_result`, `speech_packs`, `missing_files`, `download_bytes`, `extract_tar_bz2`, `app_support_dir`
- Produces Tauri commands: `get_state`, `send_message`, `start_listening`, `stop_listening`, `set_personality`, `set_model`, `set_voice`, `retry_download`
- Produces events: `token`, `turn_done`, `pose`, `mouth`, `status`, `download_progress`

- [ ] **Step 1: Write the failing test**

Add this test to `app.rs`:

```rust
#[test]
fn fetch_report_marks_loaded_engines_and_names_failures() {
    let mut app = App::new(Settings::default());
    apply_fetch_report(
        &mut app,
        &FetchReport {
            whisper: true,
            amy: true,
            lessac: false,
            failures: vec!["Piper Lessac".into()],
        },
    );
    let state = app.public_state();
    assert!(state.whisper_loaded);
    assert!(state.amy_loaded);
    assert!(!state.lessac_loaded);
    assert_eq!(state.status, "Piper Lessac download failed.");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p lumi-core --lib app::fetch_report`

Expected: FAIL to compile because `apply_fetch_report` is not defined.

- [ ] **Step 3: Write minimal implementation**

In `app.rs`:

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FetchReport {
    pub whisper: bool,
    pub amy: bool,
    pub lessac: bool,
    pub failures: Vec<String>,
}

pub fn apply_fetch_report(app: &mut App, report: &FetchReport) {
    app.mark_whisper(report.whisper);
    app.mark_voice(VoiceId::Amy, report.amy);
    app.mark_voice(VoiceId::Lessac, report.lessac);
    if let Some(label) = report.failures.last() {
        app.note_download_failure(label);
    }
}
```

In `src-tauri/src/lib.rs`, spawn a thread at startup:

1. Resolve home with `std::env::var("HOME")`.
2. Load settings from `settings_path`.
3. Build `models` as `app_support_dir.join("models")`.
4. For each `speech_packs()` entry, if `missing_files` is non-empty, emit `download_progress` and call `download_bytes` plus `extract_tar_bz2`. On failure, push the pack label into `failures`. On success, emit progress 100.
5. Call `apply_fetch_report` and emit `status` plus a fresh `get_state` is not an event; emit `status` and let the page call `get_state` again. Also emit a `state` event with `public_state()` so the page can refresh buttons. Add `state` to the event list the page already listens for, carrying the same JSON as `get_state`.
6. Call `OllamaClient::new(OLLAMA_BASE).tags()`. On error, spawn `ollama_serve_command()` and poll `tags()` every 500 ms until 20 seconds pass. Pass the result to `apply_tag_result`. Emit `status`.

Leave `submit_text` in place for the Task 9 tests. The window uses these two methods, added to `impl App`:

```rust
pub fn prepare_turn(
    &mut self,
    text: &str,
) -> Result<(String, Vec<crate::conversation::ChatMessage>, Arc<AtomicBool>), ()> {
    if text.trim().is_empty() {
        return Err(());
    }
    let Some(model) = self.resolved_model() else {
        self.set_status(STATUS_NO_MODEL);
        return Err(());
    };
    self.cancel.store(true, Ordering::SeqCst);
    self.conversation.push_user(text).map_err(|_| ())?;
    let cancel = Arc::new(AtomicBool::new(false));
    self.cancel = cancel.clone();
    self.recording = false;
    self.pose = Pose::Thinking;
    let mut messages = self.conversation.request_messages();
    if messages.last().is_some_and(|message| {
        message.role == Role::Assistant && message.content.is_empty()
    }) {
        messages.pop();
    }
    Ok((model, messages, cancel))
}

pub fn complete_turn(
    &mut self,
    assistant_text: String,
    cancelled: bool,
    error: Option<String>,
) {
    if !assistant_text.is_empty() {
        self.conversation.start_assistant();
        self.conversation.push_assistant_token(&assistant_text);
    }
    if cancelled {
        self.pose = Pose::Idle;
        self.mouth = 0;
        return;
    }
    if let Some(error) = error {
        self.status = error;
        self.pose = Pose::Idle;
        self.mouth = 0;
        return;
    }
    if self.voice_loaded() {
        self.pose = Pose::Speaking;
    } else {
        self.status = crate::turn::STATUS_COULD_NOT_SPEAK.into();
        self.pose = Pose::Idle;
    }
}
```

`send_message` clones the Tauri window, locks `App`, and calls `prepare_turn`. On `Err`, it returns the current `public_state`. On `Ok`, it drops the lock, then streams with `OllamaClient` while emitting each token:

```rust
client.stream(&model, &messages, &mut |token| {
    let _ = window.emit("token", token);
    if cancel.load(Ordering::SeqCst) {
        Control::Cancel
    } else {
        Control::Continue
    }
})
```

It appends tokens into a local `String`. After the stream, it locks `App` again and calls `complete_turn(text, cancelled, error)`. If the pose is `Speaking`, Task 12 plays audio. This task emits `turn_done` and, when the voice is not loaded, emits `status` and pose `idle`. Do not hold the `App` mutex during `stream`.

`set_personality`, `set_model`, and `set_voice` lock `App`, call the matching method, `save_settings`, and return `public_state`. `retry_download` runs the pack loop again.

`start_listening` calls `begin_listen` and returns the error string on failure. `stop_listening` in this task only sets pose back to `Idle` and recording false. Task 12 replaces `stop_listening` with transcribe-and-submit.

Save settings on every selector change. Do not write the transcript.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p lumi-core --lib` and `cargo check -p lumi-app` and `npm run build`

Expected: all three succeed.

- [ ] **Step 5: Commit**

```bash
git add crates/lumi-core/src/app.rs src-tauri/src/lib.rs src/main.ts
git commit -m "$(cat <<'EOF'
feat: send typed chat to Ollama and download speech files

EOF
)"
```

---

### Task 12: Microphone, Whisper, and Piper

**Files:**
- Create: `src-tauri/src/speech.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/lib.rs`
- Modify: `README.md`
- Test: `cargo test -p lumi-core --lib` and `cargo check -p lumi-app`

**Interfaces:**
- Consumes: `speech_packs` paths, `resample_linear`, `take_max_duration`, `frame_mouth_levels`, `speech_rate`, `App::prepare_turn`, `App::complete_turn`, `begin_listen`
- Produces: `transcribe(models_dir, samples_16k) -> Result<String, String>`, `speak(models_dir, voice, rate, text) -> Result<(Vec<f32>, u32), String>`

- [ ] **Step 1: Write the failing check**

Add to `src-tauri/Cargo.toml`:

```toml
sherpa-onnx = "1.13.8"
cpal = "0.18"
rodio = "0.23"
```

In `src-tauri/src/lib.rs`, add `mod speech;` and call `speech::transcribe` from `stop_listening`. Do not create `speech.rs` yet.

- [ ] **Step 2: Run check to verify it fails**

Run: `cargo check -p lumi-app`

Expected: FAIL because `src-tauri/src/speech.rs` is missing.

- [ ] **Step 3: Write minimal implementation**

`src-tauri/src/speech.rs`:

```rust
use lumi_core::settings::VoiceId;
use sherpa_onnx::{
    GenerationConfig, OfflineRecognizer, OfflineRecognizerConfig, OfflineTts, OfflineTtsConfig,
    OfflineTtsVitsModelConfig, OfflineWhisperModelConfig,
};
use std::path::Path;

pub fn transcribe(models_dir: &Path, samples_16k: &[f32]) -> Result<String, String> {
    let root = models_dir.join("sherpa-onnx-whisper-tiny.en");
    let mut config = OfflineRecognizerConfig::default();
    config.model_config.whisper = OfflineWhisperModelConfig {
        encoder: Some(root.join("tiny.en-encoder.int8.onnx").display().to_string()),
        decoder: Some(root.join("tiny.en-decoder.int8.onnx").display().to_string()),
        language: Some("en".into()),
        task: Some("transcribe".into()),
        tail_paddings: 0,
        enable_token_timestamps: false,
        enable_segment_timestamps: false,
    };
    config.model_config.tokens = Some(root.join("tiny.en-tokens.txt").display().to_string());
    config.model_config.num_threads = 2;
    let recognizer = OfflineRecognizer::create(&config).map_err(|error| error.to_string())?;
    let stream = recognizer.create_stream();
    stream.accept_waveform(16_000, samples_16k);
    recognizer.decode(&stream);
    Ok(stream.get_result().map(|result| result.text).unwrap_or_default())
}

pub fn speak(
    models_dir: &Path,
    voice: VoiceId,
    rate: f32,
    text: &str,
) -> Result<(Vec<f32>, u32), String> {
    let (dir, model_file) = match voice {
        VoiceId::Amy => ("vits-piper-en_US-amy-medium", "en_US-amy-medium.onnx"),
        VoiceId::Lessac => ("vits-piper-en_US-lessac-medium", "en_US-lessac-medium.onnx"),
    };
    let root = models_dir.join(dir);
    let config = OfflineTtsConfig {
        model: sherpa_onnx::OfflineTtsModelConfig {
            vits: OfflineTtsVitsModelConfig {
                model: Some(root.join(model_file).display().to_string()),
                tokens: Some(root.join("tokens.txt").display().to_string()),
                data_dir: Some(root.join("espeak-ng-data").display().to_string()),
                ..Default::default()
            },
            num_threads: 2,
            ..Default::default()
        },
        ..Default::default()
    };
    let tts = OfflineTts::create(&config).map_err(|error| error.to_string())?;
    let audio = tts
        .generate_with_config(
            text,
            &GenerationConfig { sid: 0, speed: rate, ..Default::default() },
            None,
        )
        .map_err(|error| error.to_string())?;
    Ok((audio.samples().to_vec(), audio.sample_rate() as u32))
}
```

If `create` or `generate_with_config` returns a plain value instead of `Result` in sherpa-onnx 1.13.8, drop the `map_err` and use the value directly. Keep the file paths and the `en` / `transcribe` / speed arguments.

`start_listening` calls `begin_listen`. On success, start a `cpal` input stream on a worker thread. Convert samples to `f32`, append them to `Arc<Mutex<Vec<f32>>>`, and stop the stream when `samples.len()` reaches `sample_rate * 30`. Store the sample rate beside the buffer.

`stop_listening` stops the stream, takes the buffer, runs `take_max_duration(samples, sample_rate, 30)`, then `resample_linear(..., 16_000)`, then `transcribe`. If the text trims empty, call `App::hear_nothing` and emit status `It did not catch anything.` plus pose `idle`. Otherwise call the same send path as `send_message` with that text.

After `complete_turn` leaves the pose as `Speaking`, call `speak`. On error, set status `She could not speak.`, call `finish_playback`, and emit pose `idle`. On success, play with `rodio::Sink` and `SamplesBuffer::new(1, sample_rate, samples)`. Clone `App::cancel_flag()` before playback. Every 50 ms, if that flag is true, call `sink.stop()` and leave the loop. Otherwise emit `mouth` for the frame at `sample_rate / 20` samples from `frame_mouth_levels`. When the sink is empty, call `finish_playback` and emit pose `idle`. A new typed message or a new recording sets that flag through `prepare_turn` or `begin_listen`, which is what stops the voice.

Keep a recognizer and two TTS engines in a `OnceLock` or load them when a pack finishes, so a voice change does not rebuild the model. A voice whose files are missing stays unloaded.

`README.md` tells the next person to install Ollama, run `npm install`, then `npm run tauri dev` from the repo root. It says the first launch downloads Whisper Tiny English, Piper Amy, and Piper Lessac into Application Support.

- [ ] **Step 4: Run check to verify it passes**

Run: `cargo test -p lumi-core --lib` and `cargo check -p lumi-app`

Expected: tests PASS and the app crate typechecks. The sherpa build downloads its static library and needs network.

Manual check on this Mac:

- `npm run tauri dev`
- Allow the microphone.
- Wait until the download panel finishes.
- Type a sentence, press Enter, see the reply, and hear Amy.
- Click Listen, say a sentence, click Stop, and hear the reply.
- While she is speaking, type another message and confirm the first voice stops.
- Switch personality to Calm little helper and confirm the next reply is brief.
- Quit and confirm the transcript is gone while the personality remains.
- Move the speech-pack directory aside, launch again, type a message, and confirm the reply is written while the mic button stays disabled.

- [ ] **Step 5: Commit**

```bash
git add src-tauri README.md
git commit -m "$(cat <<'EOF'
feat: listen with Whisper and answer with Piper

EOF
)"
```
