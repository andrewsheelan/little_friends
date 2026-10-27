use bzip2::read::BzDecoder;
use std::fs;
use std::io::{Read, Seek};
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
            label: "Whisper Small English",
            url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-small.en.tar.bz2",
            required: &[
                "sherpa-onnx-whisper-small.en/small.en-encoder.int8.onnx",
                "sherpa-onnx-whisper-small.en/small.en-decoder.int8.onnx",
                "sherpa-onnx-whisper-small.en/small.en-tokens.txt",
            ],
        },
        SpeechPack {
            id: "tamil-listen",
            label: "Tamil Listen",
            url: "https://huggingface.co/parismitaglobalsolutions/indicconformer-sherpa-onnx/resolve/main/ta/model.int8.onnx",
            required: &[
                "indicconformer-ta/model.int8.onnx",
                "indicconformer-ta/tokens.txt",
            ],
        },
        SpeechPack {
            id: "kokoro",
            label: "Kokoro English",
            url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/kokoro-en-v0_19.tar.bz2",
            required: &[
                "kokoro-en-v0_19/model.onnx",
                "kokoro-en-v0_19/voices.bin",
                "kokoro-en-v0_19/tokens.txt",
                "kokoro-en-v0_19/espeak-ng-data/phontab",
            ],
        },
    ];
    PACKS
}

pub fn missing_files(models_dir: &Path, pack: &SpeechPack) -> Vec<PathBuf> {
    pack.required
        .iter()
        .map(|relative| models_dir.join(relative))
        .filter(|path| !model_file_ready(path))
        .collect()
}

fn model_file_ready(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    if path.extension().and_then(|ext| ext.to_str()) == Some("onnx") {
        return onnx_is_complete(path);
    }
    true
}

/// An ONNX file is one protobuf message. A download that stops early still
/// leaves a file on disk, and loading that file aborts the process.
pub fn onnx_is_complete(path: &Path) -> bool {
    let Ok(mut file) = fs::File::open(path) else {
        return false;
    };
    let Ok(len) = file.metadata().map(|meta| meta.len()) else {
        return false;
    };
    if len == 0 {
        return false;
    }
    let mut pos = 0u64;
    while pos < len {
        let Some(key) = read_varint(&mut file, &mut pos, len) else {
            return false;
        };
        let ok = match key & 7 {
            0 => read_varint(&mut file, &mut pos, len).is_some(),
            1 => advance(&mut file, &mut pos, len, 8),
            5 => advance(&mut file, &mut pos, len, 4),
            2 => read_varint(&mut file, &mut pos, len)
                .is_some_and(|size| advance(&mut file, &mut pos, len, size)),
            _ => false,
        };
        if !ok {
            return false;
        }
    }
    pos == len
}

fn read_varint(file: &mut fs::File, pos: &mut u64, len: u64) -> Option<u64> {
    let mut value = 0u64;
    let mut shift = 0;
    loop {
        if *pos >= len || shift > 63 {
            return None;
        }
        let mut byte = [0u8; 1];
        file.seek(std::io::SeekFrom::Start(*pos)).ok()?;
        if file.read(&mut byte).ok()? != 1 {
            return None;
        }
        *pos += 1;
        value |= u64::from(byte[0] & 0x7f) << shift;
        if byte[0] & 0x80 == 0 {
            return Some(value);
        }
        shift += 7;
    }
}

fn advance(file: &mut fs::File, pos: &mut u64, len: u64, size: u64) -> bool {
    if size > len.saturating_sub(*pos) {
        return false;
    }
    *pos += size;
    file.seek(std::io::SeekFrom::Start(*pos)).is_ok()
}

pub fn extract_tar_bz2(bytes: &[u8], dest_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dest_dir).map_err(|error| error.to_string())?;
    let decoder = BzDecoder::new(bytes);
    let mut archive = tar::Archive::new(decoder);
    archive.unpack(dest_dir).map_err(|error| error.to_string())
}

pub fn download_bytes(url: &str, client: &reqwest::blocking::Client) -> Result<Vec<u8>, String> {
    let response = client.get(url).send().map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("download failed: {}", response.status()));
    }
    // `bytes` applies the client timeout to the whole body, so a stall cannot
    // reset the clock on every chunk.
    let bytes = response.bytes().map_err(|error| error.to_string())?;
    Ok(bytes.to_vec())
}

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
            "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-small.en.tar.bz2"
        );
        assert!(packs[0]
            .required
            .contains(&"sherpa-onnx-whisper-small.en/small.en-encoder.int8.onnx"));
        assert!(packs[0]
            .required
            .contains(&"sherpa-onnx-whisper-small.en/small.en-decoder.int8.onnx"));
        assert!(packs[0]
            .required
            .contains(&"sherpa-onnx-whisper-small.en/small.en-tokens.txt"));
        assert!(packs[1].url.contains("indicconformer-sherpa-onnx"));
        assert!(packs[1].url.contains("/ta/model.int8.onnx"));
        assert!(packs[1]
            .required
            .contains(&"indicconformer-ta/model.int8.onnx"));
        assert!(packs[1].required.contains(&"indicconformer-ta/tokens.txt"));
        assert!(packs[2].url.contains("kokoro-en-v0_19.tar.bz2"));
        assert!(packs[2].required.contains(&"kokoro-en-v0_19/model.onnx"));
        assert!(packs[2].required.contains(&"kokoro-en-v0_19/voices.bin"));
        assert!(packs[2]
            .required
            .contains(&"kokoro-en-v0_19/espeak-ng-data/phontab"));
    }

    #[test]
    fn missing_files_lists_only_what_is_absent() {
        let dir = std::env::temp_dir().join(format!("lumi-packs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sherpa-onnx-whisper-small.en")).unwrap();
        std::fs::write(
            dir.join("sherpa-onnx-whisper-small.en/small.en-tokens.txt"),
            "tok",
        )
        .unwrap();
        let missing = missing_files(&dir, &speech_packs()[0]);
        assert_eq!(missing.len(), 2);
        assert!(missing
            .iter()
            .all(|path| !path.ends_with("small.en-tokens.txt")));
    }

    #[test]
    fn a_short_onnx_file_is_not_a_model() {
        let dir = std::env::temp_dir().join(format!("lumi-onnx-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let root = dir.join("sherpa-onnx-whisper-small.en");
        std::fs::create_dir_all(&root).unwrap();
        // A one-field protobuf: ir_version = 7.
        std::fs::write(root.join("small.en-encoder.int8.onnx"), [0x08, 0x07]).unwrap();
        // Length says 10 more bytes, and only one is present.
        std::fs::write(root.join("small.en-decoder.int8.onnx"), [0x12, 0x0a, 0x01]).unwrap();
        std::fs::write(root.join("small.en-tokens.txt"), "tok").unwrap();
        let missing = missing_files(&dir, &speech_packs()[0]);
        assert!(missing
            .iter()
            .any(|path| path.ends_with("small.en-decoder.int8.onnx")));
        assert!(!missing
            .iter()
            .any(|path| path.ends_with("small.en-encoder.int8.onnx")));
        assert!(!onnx_is_complete(
            &root.join("small.en-decoder.int8.onnx")
        ));
        assert!(onnx_is_complete(&root.join("small.en-encoder.int8.onnx")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn extract_tar_bz2_writes_the_archive_entries() {
        let mut builder = tar::Builder::new(Vec::new());
        let mut header = tar::Header::new_gnu();
        let bytes = b"hello";
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, "demo/file.txt", &bytes[..])
            .unwrap();
        let tar_bytes = builder.into_inner().unwrap();
        let mut compressed = Vec::new();
        {
            let mut encoder =
                bzip2::write::BzEncoder::new(&mut compressed, bzip2::Compression::fast());
            encoder.write_all(&tar_bytes).unwrap();
            encoder.finish().unwrap();
        }
        let dest = std::env::temp_dir().join(format!("lumi-extract-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dest);
        extract_tar_bz2(&compressed, &dest).unwrap();
        assert_eq!(
            std::fs::read_to_string(dest.join("demo/file.txt")).unwrap(),
            "hello"
        );
    }
}
