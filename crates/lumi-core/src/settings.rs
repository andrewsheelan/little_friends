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
    Dad,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VoiceId {
    #[serde(alias = "amy")]
    Bella,
    #[serde(alias = "lessac")]
    Sky,
    Emma,
    Adam,
    George,
}

/// The voice that belongs to a character. It is not chosen separately.
pub fn character_voice(personality: Personality) -> VoiceId {
    match personality {
        Personality::Cheerful => VoiceId::Sky,
        Personality::Calm => VoiceId::Emma,
        Personality::Study => VoiceId::Adam,
        Personality::Dad => VoiceId::George,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ListenMode {
    Click,
    Wake,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    English,
    Tamil,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    pub personality: Personality,
    pub model: String,
    pub voice: VoiceId,
    pub listen_mode: ListenMode,
    pub wake_phrase: String,
    /// Missing on older settings files, which stay English.
    #[serde(default)]
    pub language: Language,
    /// Folder that receives a copy of every conversation. Empty means off.
    #[serde(default)]
    pub backup_dir: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            personality: Personality::Cheerful,
            model: "gemma4:latest".to_string(),
            voice: VoiceId::Bella,
            listen_mode: ListenMode::Click,
            wake_phrase: "hey lumi".to_string(),
            language: Language::English,
            backup_dir: String::new(),
        }
    }
}

pub fn app_support_dir(home: &Path) -> PathBuf {
    home.join("Library/Application Support/com.lumi.companion")
}

pub fn settings_path(home: &Path) -> PathBuf {
    app_support_dir(home).join("settings.json")
}

pub fn memories_path(home: &Path) -> PathBuf {
    app_support_dir(home).join("memories.json")
}

/// One saved line of a person's conversation. System instructions are not stored.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredMessage {
    pub role: String,
    pub content: String,
}

/// Lumi, Pip, and Milo each keep their own conversation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PersonMemories {
    #[serde(default)]
    pub cheerful: Vec<StoredMessage>,
    #[serde(default)]
    pub calm: Vec<StoredMessage>,
    #[serde(default)]
    pub study: Vec<StoredMessage>,
    #[serde(default)]
    pub dad: Vec<StoredMessage>,
}

pub fn load_memories(path: &Path) -> PersonMemories {
    let Ok(text) = fs::read_to_string(path) else {
        return PersonMemories::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

pub fn save_memories(path: &Path, memories: &PersonMemories) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(memories).expect("memories serialize");
    fs::write(path, text)
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
        assert_eq!(settings.voice, VoiceId::Bella);
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
    fn round_trip_keeps_the_saved_fields() {
        let path = std::env::temp_dir().join("lumi-settings-roundtrip.json");
        let settings = Settings {
            personality: Personality::Study,
            model: "qwen3.6:latest".into(),
            voice: VoiceId::Sky,
            listen_mode: ListenMode::Wake,
            wake_phrase: "hey lumi".into(),
            language: Language::Tamil,
            backup_dir: "/tmp/lumi-backup".into(),
        };
        save_settings(&path, &settings).unwrap();
        let saved: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let mut keys: Vec<_> = saved.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(
            keys,
            vec![
                "backup_dir",
                "language",
                "listen_mode",
                "model",
                "personality",
                "voice",
                "wake_phrase"
            ]
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

    #[test]
    fn saved_piper_names_load_as_kokoro_voices() {
        let path = std::env::temp_dir().join("lumi-settings-piper-alias.json");
        std::fs::write(
            &path,
            r#"{"personality":"cheerful","model":"gemma4:latest","voice":"amy","listen_mode":"click","wake_phrase":"hey lumi"}"#,
        )
        .unwrap();
        assert_eq!(load_settings(&path).voice, VoiceId::Bella);
        std::fs::write(
            &path,
            r#"{"personality":"cheerful","model":"gemma4:latest","voice":"lessac","listen_mode":"click","wake_phrase":"hey lumi"}"#,
        )
        .unwrap();
        assert_eq!(load_settings(&path).voice, VoiceId::Sky);
    }

    #[test]
    fn older_settings_without_a_backup_folder_stay_off() {
        let path = std::env::temp_dir().join("lumi-settings-no-backup.json");
        std::fs::write(
            &path,
            r#"{"personality":"cheerful","model":"gemma4:latest","voice":"bella","listen_mode":"click","wake_phrase":"hey lumi"}"#,
        )
        .unwrap();
        assert_eq!(load_settings(&path).backup_dir, "");
    }

    #[test]
    fn older_settings_without_a_language_stay_english() {
        let path = std::env::temp_dir().join("lumi-settings-no-language.json");
        std::fs::write(
            &path,
            r#"{"personality":"cheerful","model":"gemma4:latest","voice":"bella","listen_mode":"click","wake_phrase":"hey lumi"}"#,
        )
        .unwrap();
        assert_eq!(load_settings(&path).language, Language::English);
    }

    #[test]
    fn memories_round_trip_keeps_each_person() {
        let path = std::env::temp_dir().join("lumi-memories-roundtrip.json");
        let memories = PersonMemories {
            cheerful: vec![StoredMessage {
                role: "user".into(),
                content: "Hello Lumi".into(),
            }],
            calm: vec![StoredMessage {
                role: "assistant".into(),
                content: "Pip here".into(),
            }],
            study: Vec::new(),
            dad: Vec::new(),
        };
        save_memories(&path, &memories).unwrap();
        assert_eq!(load_memories(&path), memories);
        std::fs::write(&path, "{not json").unwrap();
        assert_eq!(load_memories(&path), PersonMemories::default());
    }

    #[test]
    fn each_character_has_a_different_voice() {
        let voices = [
            character_voice(Personality::Cheerful),
            character_voice(Personality::Calm),
            character_voice(Personality::Study),
            character_voice(Personality::Dad),
        ];
        assert_eq!(
            voices,
            [
                VoiceId::Sky,
                VoiceId::Emma,
                VoiceId::Adam,
                VoiceId::George
            ]
        );
        assert_eq!(voices.iter().collect::<std::collections::HashSet<_>>().len(), 4);
    }

    #[test]
    fn an_older_memory_file_without_hank_stays_readable() {
        let path = std::env::temp_dir().join("lumi-memories-no-dad.json");
        std::fs::write(&path, r#"{"cheerful":[],"calm":[],"study":[]}"#).unwrap();
        assert!(load_memories(&path).dad.is_empty());
    }
}
