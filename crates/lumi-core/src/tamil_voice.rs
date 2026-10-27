//! Tamil voices: three Piper VITS models that sherpa-onnx can run on any platform.
//!
//! The community models ship as a bare `.onnx` plus a Piper `.onnx.json`. sherpa-onnx
//! wants a `tokens.txt` and a few metadata entries inside the `.onnx`, so both are
//! made here when the voice is installed.

use crate::packs::{download_bytes, onnx_is_complete};
use crate::settings::Personality;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TamilVoice {
    HemaLatha,
    Rasa,
    Valluvar,
}

impl TamilVoice {
    pub const ALL: [TamilVoice; 3] = [TamilVoice::HemaLatha, TamilVoice::Rasa, TamilVoice::Valluvar];

    /// Lumi is the girl, so she gets the female voice. Milo, Hank, and Pip get the male ones.
    pub fn for_personality(personality: Personality) -> Self {
        match personality {
            Personality::Cheerful => TamilVoice::HemaLatha,
            Personality::Study | Personality::Calm => TamilVoice::Rasa,
            Personality::Dad => TamilVoice::Valluvar,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            TamilVoice::HemaLatha => "Hema Latha",
            TamilVoice::Rasa => "Rasa",
            TamilVoice::Valluvar => "Valluvar",
        }
    }

    fn folder(self) -> &'static str {
        match self {
            TamilVoice::HemaLatha => "piper-ta-hema-latha",
            TamilVoice::Rasa => "piper-ta-rasa",
            TamilVoice::Valluvar => "piper-ta-valluvar",
        }
    }

    fn url(self, extension: &str) -> String {
        let (repo, name) = match self {
            TamilVoice::HemaLatha => (
                "Jeyaram-K/piper-tamil-voices",
                "ta_IN-HemaLatha-medium/ta_IN-HemaLatha-medium",
            ),
            TamilVoice::Valluvar => (
                "Jeyaram-K/piper-tamil-voices",
                "ta_IN-ValluvarNeural-medium/ta_IN-ValluvarNeural-medium",
            ),
            TamilVoice::Rasa => (
                "tinisoft/piper-ta_IN-rasa_male-medium",
                "ta_IN-rasa_male-medium",
            ),
        };
        format!("https://huggingface.co/{repo}/resolve/main/{name}{extension}")
    }

    pub fn model_url(self) -> String {
        self.url(".onnx")
    }

    pub fn config_url(self) -> String {
        self.url(".onnx.json")
    }
}

pub fn voice_dir(models_dir: &Path, voice: TamilVoice) -> PathBuf {
    models_dir.join(voice.folder())
}

pub fn model_path(models_dir: &Path, voice: TamilVoice) -> PathBuf {
    voice_dir(models_dir, voice).join("model.onnx")
}

pub fn tokens_path(models_dir: &Path, voice: TamilVoice) -> PathBuf {
    voice_dir(models_dir, voice).join("tokens.txt")
}

/// True when the voice is fully installed.
pub fn is_ready(models_dir: &Path, voice: TamilVoice) -> bool {
    onnx_is_complete(&model_path(models_dir, voice)) && tokens_path(models_dir, voice).is_file()
}

/// One `symbol id` line per Piper phoneme, the format sherpa-onnx reads.
pub fn tokens_text(config: &str) -> Result<String, String> {
    let bad = || "The Tamil voice settings could not be read.".to_string();
    let value: serde_json::Value = serde_json::from_str(config).map_err(|_| bad())?;
    let map = value
        .get("phoneme_id_map")
        .and_then(|map| map.as_object())
        .ok_or_else(bad)?;
    let mut lines: Vec<(u64, String)> = Vec::new();
    for (symbol, ids) in map {
        let id = ids
            .as_array()
            .and_then(|ids| ids.first())
            .and_then(|id| id.as_u64())
            .ok_or_else(bad)?;
        lines.push((id, format!("{symbol} {id}\n")));
    }
    if lines.is_empty() {
        return Err(bad());
    }
    lines.sort_by_key(|(id, _)| *id);
    Ok(lines.into_iter().map(|(_, line)| line).collect())
}

fn put_varint(out: &mut Vec<u8>, mut value: usize) {
    while value >= 0x80 {
        out.push((value as u8 & 0x7f) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

fn put_text(out: &mut Vec<u8>, tag: u8, text: &str) {
    out.push(tag);
    put_varint(out, text.len());
    out.extend_from_slice(text.as_bytes());
}

/// Appends the metadata sherpa-onnx reads from a Piper model. An ONNX file is one
/// protobuf message, and a repeated field may simply be appended to the end of it
/// (`metadata_props` is field 14, each entry a key and a value).
pub fn with_piper_metadata(onnx: &[u8], config: &str) -> Result<Vec<u8>, String> {
    let bad = || "The Tamil voice settings could not be read.".to_string();
    let value: serde_json::Value = serde_json::from_str(config).map_err(|_| bad())?;
    let sample_rate = value
        .pointer("/audio/sample_rate")
        .and_then(|rate| rate.as_u64())
        .ok_or_else(bad)?;
    let speakers = value
        .get("num_speakers")
        .and_then(|count| count.as_u64())
        .unwrap_or(1);
    let voice = value
        .pointer("/espeak/voice")
        .and_then(|voice| voice.as_str())
        .unwrap_or("ta");
    let entries = [
        ("model_type", "vits".to_string()),
        ("comment", "piper".to_string()),
        ("language", "Tamil".to_string()),
        ("voice", voice.to_string()),
        ("has_espeak", "1".to_string()),
        ("n_speakers", speakers.to_string()),
        ("sample_rate", sample_rate.to_string()),
    ];
    let mut out = onnx.to_vec();
    for (key, text) in entries {
        let mut entry = Vec::new();
        put_text(&mut entry, 0x0a, key);
        put_text(&mut entry, 0x12, &text);
        out.push(0x72);
        put_varint(&mut out, entry.len());
        out.extend_from_slice(&entry);
    }
    Ok(out)
}

/// Downloads one voice and makes it ready for sherpa-onnx. The model is written last,
/// so a stopped download never looks installed.
pub fn fetch(
    models_dir: &Path,
    voice: TamilVoice,
    client: &reqwest::blocking::Client,
) -> Result<(), String> {
    if is_ready(models_dir, voice) {
        return Ok(());
    }
    let config = String::from_utf8(download_bytes(&voice.config_url(), client)?)
        .map_err(|_| "The Tamil voice settings could not be read.".to_string())?;
    let tokens = tokens_text(&config)?;
    let onnx = download_bytes(&voice.model_url(), client)?;
    let model = with_piper_metadata(&onnx, &config)?;
    let dir = voice_dir(models_dir, voice);
    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    fs::write(tokens_path(models_dir, voice), tokens).map_err(|error| error.to_string())?;
    let part = dir.join("model.onnx.part");
    fs::write(&part, model).map_err(|error| error.to_string())?;
    fs::rename(&part, model_path(models_dir, voice)).map_err(|error| error.to_string())?;
    if is_ready(models_dir, voice) {
        Ok(())
    } else {
        Err("The Tamil voice did not download completely.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIG: &str = r#"{
        "audio": {"sample_rate": 22050},
        "espeak": {"voice": "ta"},
        "num_speakers": 1,
        "phoneme_id_map": {"_": [0], "^": [1], "$": [2], " ": [3], "a": [14, 99]}
    }"#;

    #[test]
    fn lumi_is_the_girl_and_everyone_else_is_a_man() {
        assert_eq!(TamilVoice::for_personality(Personality::Cheerful), TamilVoice::HemaLatha);
        assert_eq!(TamilVoice::for_personality(Personality::Study), TamilVoice::Rasa);
        assert_eq!(TamilVoice::for_personality(Personality::Dad), TamilVoice::Valluvar);
        assert_eq!(TamilVoice::for_personality(Personality::Calm), TamilVoice::Rasa);
    }

    #[test]
    fn every_voice_downloads_from_https() {
        for voice in TamilVoice::ALL {
            assert!(voice.model_url().starts_with("https://huggingface.co/"));
            assert!(voice.model_url().ends_with(".onnx"));
            assert!(voice.config_url().ends_with(".onnx.json"));
        }
    }

    #[test]
    fn tokens_list_each_symbol_with_its_first_id_in_order() {
        let text = tokens_text(CONFIG).unwrap();
        assert_eq!(text, "_ 0\n^ 1\n$ 2\n  3\na 14\n");
    }

    #[test]
    fn broken_settings_are_refused() {
        assert!(tokens_text("not json").is_err());
        assert!(tokens_text(r#"{"phoneme_id_map": {}}"#).is_err());
        assert!(with_piper_metadata(b"", r#"{"audio": {}}"#).is_err());
    }

    #[test]
    fn metadata_is_appended_and_the_file_stays_a_valid_message() {
        let dir = std::env::temp_dir().join(format!("lumi-tamil-meta-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        // Field 1 (ir_version) = 8, then the appended entries.
        let model = with_piper_metadata(&[0x08, 0x08], CONFIG).unwrap();
        assert_eq!(&model[..2], &[0x08, 0x08]);
        let text = String::from_utf8_lossy(&model).into_owned();
        for expected in ["model_type", "vits", "comment", "piper", "language", "Tamil", "has_espeak", "n_speakers", "sample_rate", "22050"] {
            assert!(text.contains(expected), "missing {expected}");
        }
        let path = dir.join("model.onnx");
        fs::write(&path, &model).unwrap();
        assert!(onnx_is_complete(&path));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_voice_is_ready_only_when_both_files_are_whole() {
        let dir = std::env::temp_dir().join(format!("lumi-tamil-ready-{}", std::process::id()));
        let voice = TamilVoice::Rasa;
        assert!(!is_ready(&dir, voice));
        fs::create_dir_all(voice_dir(&dir, voice)).unwrap();
        fs::write(model_path(&dir, voice), with_piper_metadata(&[0x08, 0x08], CONFIG).unwrap()).unwrap();
        assert!(!is_ready(&dir, voice), "tokens are still missing");
        fs::write(tokens_path(&dir, voice), "_ 0\n").unwrap();
        assert!(is_ready(&dir, voice));
        fs::remove_dir_all(&dir).unwrap();
    }
}
