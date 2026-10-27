use crate::settings::VoiceId;
use reqwest::blocking::Client;
use std::time::Duration;

/// The Ollama speech model that voices Lumi, Milo, and Hank.
pub const SPEECH_MODEL: &str = "phonon";

/// The Phonon voice this character speaks with.
pub fn voice_name(voice: VoiceId) -> &'static str {
    match voice {
        VoiceId::Adam => "Easton",
        VoiceId::George => "Hugh",
        _ => "Freya",
    }
}

/// Reads a 16-bit mono or stereo PCM WAV into mono samples.
pub fn decode_wav(bytes: &[u8]) -> Result<(Vec<f32>, u32), String> {
    let bad = || "The voice sent audio that could not be read.".to_string();
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(bad());
    }
    let mut index = 12;
    let mut rate = 0u32;
    let mut channels = 1usize;
    while index + 8 <= bytes.len() {
        let id = &bytes[index..index + 4];
        let size = u32::from_le_bytes(bytes[index + 4..index + 8].try_into().unwrap()) as usize;
        let body = index + 8;
        if id == b"fmt " && body + 16 <= bytes.len() {
            if u16::from_le_bytes([bytes[body], bytes[body + 1]]) != 1
                || u16::from_le_bytes([bytes[body + 14], bytes[body + 15]]) != 16
            {
                return Err(bad());
            }
            channels = usize::from(u16::from_le_bytes([bytes[body + 2], bytes[body + 3]])).max(1);
            rate = u32::from_le_bytes(bytes[body + 4..body + 8].try_into().unwrap());
        }
        if id == b"data" {
            let end = body.saturating_add(size).min(bytes.len());
            let frames = bytes[body.min(end)..end].chunks_exact(2 * channels);
            let samples: Vec<f32> = frames
                .map(|frame| {
                    let sum: f32 = frame
                        .chunks_exact(2)
                        .map(|pair| i16::from_le_bytes([pair[0], pair[1]]) as f32 / 32_768.0)
                        .sum();
                    sum / channels as f32
                })
                .collect();
            if samples.is_empty() || rate == 0 {
                return Err(bad());
            }
            return Ok((samples, rate));
        }
        index = body.saturating_add(size).saturating_add(size % 2);
    }
    Err(bad())
}

/// Talks to Ollama's speech endpoint, which streams the voice of the Phonon model.
pub struct VoiceClient {
    base: String,
    http: Client,
}

impl VoiceClient {
    pub fn new(base: &str) -> Self {
        Self {
            base: base.trim_end_matches('/').to_string(),
            http: Client::builder().build().expect("http client"),
        }
    }

    /// True when Ollama answers and the Phonon model has been pulled.
    pub fn ready(&self) -> bool {
        let Ok(response) = self
            .http
            .get(format!("{}/api/tags", self.base))
            .timeout(Duration::from_secs(2))
            .send()
        else {
            return false;
        };
        let Ok(body) = response.json::<serde_json::Value>() else {
            return false;
        };
        body.get("models")
            .and_then(|models| models.as_array())
            .is_some_and(|models| {
                models.iter().any(|model| {
                    model
                        .get("name")
                        .and_then(|name| name.as_str())
                        .is_some_and(|name| {
                            name == SPEECH_MODEL || name.starts_with(&format!("{SPEECH_MODEL}:"))
                        })
                })
            })
    }

    pub fn speak(&self, text: &str, voice: VoiceId) -> Result<(Vec<f32>, u32), String> {
        let response = self
            .http
            .post(format!("{}/v1/audio/speech", self.base))
            .timeout(Duration::from_secs(60))
            .json(&serde_json::json!({
                "model": SPEECH_MODEL,
                "voice": voice_name(voice),
                "input": text,
                "response_format": "wav",
            }))
            .send()
            .map_err(|error| error.to_string())?;
        if !response.status().is_success() {
            return Err(format!("The voice returned {}", response.status()));
        }
        let bytes = response.bytes().map_err(|error| error.to_string())?;
        decode_wav(&bytes)
    }
}

/// Splits a reply into pieces to voice one after another, so the first words
/// are heard long before the last are made. The first piece is one sentence
/// (the quickest to make); later pieces pack whole sentences up to `limit`.
pub fn speech_chunks(text: &str, limit: usize) -> Vec<String> {
    let mut sentences: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut chars = text.trim().chars().peekable();
    while let Some(c) = chars.next() {
        current.push(c);
        let ends = matches!(c, '.' | '!' | '?' | '…');
        if ends && chars.peek().is_none_or(|next| next.is_whitespace()) {
            sentences.push(std::mem::take(&mut current).trim().to_string());
        }
    }
    if !current.trim().is_empty() {
        sentences.push(current.trim().to_string());
    }
    sentences.retain(|sentence| !sentence.is_empty());

    let mut pieces: Vec<String> = Vec::new();
    let mut group = String::new();
    for (index, sentence) in sentences.into_iter().enumerate() {
        let alone = index == 0;
        if alone {
            pieces.push(sentence);
            continue;
        }
        if !group.is_empty() && group.len() + 1 + sentence.len() > limit {
            pieces.push(std::mem::take(&mut group));
        }
        if !group.is_empty() {
            group.push(' ');
        }
        group.push_str(&sentence);
    }
    if !group.is_empty() {
        pieces.push(group);
    }
    pieces
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};
    use std::thread;

    fn wav(samples: &[i16], rate: u32, channels: u16) -> Vec<u8> {
        let data: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&channels.to_le_bytes());
        out.extend_from_slice(&rate.to_le_bytes());
        out.extend_from_slice(&(rate * 2 * u32::from(channels)).to_le_bytes());
        out.extend_from_slice(&(2 * channels).to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&data);
        out
    }

    fn serve(status: u16, kind: &'static str, body: Vec<u8>, seen: Arc<Mutex<String>>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut buf = [0u8; 4096];
            let mut header_end = 0;
            while header_end == 0 {
                let read = socket.read(&mut buf).unwrap();
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&buf[..read]);
                if let Some(pos) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                    header_end = pos + 4;
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
            *seen.lock().unwrap() = String::from_utf8_lossy(&request).into_owned();
            let head = format!(
                "HTTP/1.1 {status} X\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            socket.write_all(head.as_bytes()).unwrap();
            socket.write_all(&body).unwrap();
        });
        format!("http://127.0.0.1:{port}")
    }

    #[test]
    fn each_character_has_its_own_voice_name() {
        assert_eq!(voice_name(VoiceId::Sky), "Freya");
        assert_eq!(voice_name(VoiceId::Adam), "Easton");
        assert_eq!(voice_name(VoiceId::George), "Hugh");
    }

    #[test]
    fn a_wav_becomes_samples_and_a_rate() {
        let (samples, rate) = decode_wav(&wav(&[0, 16384, -16384], 24000, 1)).unwrap();
        assert_eq!(rate, 24000);
        assert_eq!(samples, vec![0.0, 0.5, -0.5]);
    }

    #[test]
    fn stereo_is_averaged_to_mono() {
        let (samples, _) = decode_wav(&wav(&[16384, 0, -16384, -16384], 24000, 2)).unwrap();
        assert_eq!(samples, vec![0.25, -0.5]);
    }

    #[test]
    fn junk_and_empty_audio_are_refused() {
        assert!(decode_wav(b"not a wav at all, not even close to one").is_err());
        assert!(decode_wav(&wav(&[], 24000, 1)).is_err());
    }

    #[test]
    fn ready_means_ollama_has_the_phonon_model() {
        let tags = |names: &str| format!(r#"{{"models":[{names}]}}"#).into_bytes();
        let seen = Arc::new(Mutex::new(String::new()));
        let base = serve(200, "application/json", tags(r#"{"name":"phonon:latest"}"#), seen.clone());
        assert!(VoiceClient::new(&base).ready());
        assert!(seen.lock().unwrap().starts_with("GET /api/tags"));
        let seen = Arc::new(Mutex::new(String::new()));
        let base = serve(200, "application/json", tags(r#"{"name":"gemma4:latest"}"#), seen);
        assert!(!VoiceClient::new(&base).ready());
        assert!(!VoiceClient::new("http://127.0.0.1:9").ready());
    }

    #[test]
    fn speak_sends_the_text_and_the_voice_and_returns_the_audio() {
        let seen = Arc::new(Mutex::new(String::new()));
        let base = serve(200, "audio/wav", wav(&[100, 200], 24000, 1), seen.clone());
        let (samples, rate) = VoiceClient::new(&base)
            .speak("Hello there.", VoiceId::George)
            .unwrap();
        assert_eq!(rate, 24000);
        assert_eq!(samples.len(), 2);
        let request = seen.lock().unwrap().clone();
        assert!(request.starts_with("POST /v1/audio/speech"));
        assert!(request.contains("\"input\":\"Hello there.\""));
        assert!(request.contains("\"voice\":\"Hugh\""));
        assert!(request.contains("\"model\":\"phonon\""));
        assert!(request.contains("\"response_format\":\"wav\""));
    }

    #[test]
    fn a_refused_voice_is_an_error_not_silence() {
        let seen = Arc::new(Mutex::new(String::new()));
        let base = serve(400, "application/json", br#"{"error":"unknown voice"}"#.to_vec(), seen);
        assert!(VoiceClient::new(&base).speak("Hi.", VoiceId::Sky).is_err());
    }

    #[test]
    fn the_first_sentence_is_voiced_alone_and_the_rest_are_packed() {
        let pieces = speech_chunks("Hi there. I like mud! Do you? Yes. Me too.", 30);
        assert_eq!(
            pieces,
            vec!["Hi there.", "I like mud! Do you? Yes.", "Me too."]
        );
    }

    #[test]
    fn a_short_reply_is_one_piece_and_empty_text_is_none() {
        assert_eq!(speech_chunks("  Hello friend  ", 200), vec!["Hello friend"]);
        assert!(speech_chunks("   ", 200).is_empty());
    }

    #[test]
    fn a_number_with_a_dot_is_not_a_sentence_end() {
        assert_eq!(speech_chunks("It is 3.5 miles. Walk.", 200), vec!["It is 3.5 miles.", "Walk."]);
    }
}
