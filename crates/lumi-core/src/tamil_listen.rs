//! Tamil listening: AI4Bharat's IndicConformer, a Tamil-only CTC model.
//!
//! The community sherpa-onnx export is two Hugging Face files, not a tar pack.

use crate::packs::{download_bytes, onnx_is_complete};
use std::fs;
use std::path::{Path, PathBuf};

pub const DIR: &str = "indicconformer-ta";
pub const MODEL_FILE: &str = "model.int8.onnx";
pub const TOKENS_FILE: &str = "tokens.txt";

const REPO: &str = "https://huggingface.co/parismitaglobalsolutions/indicconformer-sherpa-onnx/resolve/main";

pub fn model_url() -> String {
    format!("{REPO}/ta/{MODEL_FILE}")
}

pub fn tokens_url() -> String {
    format!("{REPO}/{TOKENS_FILE}")
}

pub fn listen_dir(models_dir: &Path) -> PathBuf {
    models_dir.join(DIR)
}

pub fn model_path(models_dir: &Path) -> PathBuf {
    listen_dir(models_dir).join(MODEL_FILE)
}

pub fn tokens_path(models_dir: &Path) -> PathBuf {
    listen_dir(models_dir).join(TOKENS_FILE)
}

/// True when the Tamil listener is fully installed.
pub fn is_ready(models_dir: &Path) -> bool {
    onnx_is_complete(&model_path(models_dir)) && tokens_path(models_dir).is_file()
}

/// Downloads the Tamil listener. The model is written last, so a stopped
/// download never looks installed.
pub fn fetch(models_dir: &Path, client: &reqwest::blocking::Client) -> Result<(), String> {
    if is_ready(models_dir) {
        return Ok(());
    }
    let tokens = download_bytes(&tokens_url(), client)?;
    let model = download_bytes(&model_url(), client)?;
    let dir = listen_dir(models_dir);
    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    fs::write(tokens_path(models_dir), tokens).map_err(|error| error.to_string())?;
    let part = dir.join(format!("{MODEL_FILE}.part"));
    fs::write(&part, model).map_err(|error| error.to_string())?;
    fs::rename(&part, model_path(models_dir)).map_err(|error| error.to_string())?;
    if is_ready(models_dir) {
        Ok(())
    } else {
        Err("The Tamil listener did not download completely.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_listener_downloads_the_tamil_indicconformer() {
        assert!(model_url().contains("indicconformer-sherpa-onnx"));
        assert!(model_url().ends_with("/ta/model.int8.onnx"));
        assert!(tokens_url().ends_with("/tokens.txt"));
        assert!(tokens_url().starts_with("https://huggingface.co/"));
    }

    #[test]
    fn the_listener_is_ready_only_when_both_files_are_whole() {
        let dir = std::env::temp_dir().join(format!("lumi-tamil-listen-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        assert!(!is_ready(&dir));
        fs::create_dir_all(listen_dir(&dir)).unwrap();
        fs::write(model_path(&dir), [0x08, 0x08]).unwrap();
        assert!(!is_ready(&dir), "tokens are still missing");
        fs::write(tokens_path(&dir), "a 0\n").unwrap();
        assert!(is_ready(&dir));
        fs::remove_dir_all(&dir).unwrap();
    }
}
