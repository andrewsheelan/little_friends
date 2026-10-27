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
    /// Absent on an error line such as `{"error":"model unloaded"}`.
    #[serde(default)]
    done: bool,
    error: Option<String>,
}

#[derive(Deserialize)]
struct ErrorBody {
    error: Option<String>,
}

fn ollama_error_text(body: &str, fallback: String) -> String {
    serde_json::from_str::<ErrorBody>(body)
        .ok()
        .and_then(|parsed| parsed.error)
        .filter(|text| !text.is_empty())
        .unwrap_or(fallback)
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

    /// One short answer, not a streamed chat turn.
    pub fn complete(&self, model: &str, messages: &[ChatMessage]) -> Result<String, String> {
        let payload = serde_json::json!({
            "model": model,
            "stream": false,
            // Spoken replies must start at once: no hidden reasoning before the first word.
            "think": false,
            "options": { "num_ctx": 8192, "num_predict": 80, "temperature": 0.8 },
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
            .map_err(|error| error.to_string())?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            return Err(ollama_error_text(&body, format!("Ollama returned {status}")));
        }
        let parsed: StreamLine = response.json().map_err(|error| error.to_string())?;
        if let Some(error) = parsed.error.filter(|text| !text.is_empty()) {
            return Err(error);
        }
        Ok(parsed.message.map(|message| message.content).unwrap_or_default())
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
            // Spoken replies must start at once: no hidden reasoning before the first word.
            "think": false,
            "options": { "num_ctx": 8192 },
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
            let status = response.status();
            let body = response.text().unwrap_or_default();
            return Err(ModelError::Failed(ollama_error_text(
                &body,
                format!("Ollama returned {status}"),
            )));
        }
        let reader = BufReader::new(response);
        for line in reader.lines() {
            let line = line.map_err(|error| ModelError::Failed(error.to_string()))?;
            if line.trim().is_empty() {
                continue;
            }
            let parsed: StreamLine = serde_json::from_str(&line)
                .map_err(|error| ModelError::Failed(error.to_string()))?;
            if let Some(error) = parsed.error.filter(|text| !text.is_empty()) {
                return Err(ModelError::Failed(error));
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversation::{ChatMessage, Role};
    use crate::turn::{Control, ModelClient, ModelError};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};
    use std::thread;

    fn serve(body: &'static str, captured: Arc<Mutex<String>>) -> String {
        serve_status(200, "OK", body, captured)
    }

    fn serve_status(
        status: u16,
        reason: &'static str,
        body: &'static str,
        captured: Arc<Mutex<String>>,
    ) -> String {
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
                "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
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
            ChatMessage {
                role: Role::System,
                content: "sys".into(),
            },
            ChatMessage {
                role: Role::User,
                content: "Hi".into(),
            },
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
        assert!(request.contains("\"num_ctx\":8192"));
        assert!(request.contains("\"think\":false"));
    }

    #[test]
    fn error_line_fails_with_ollama_text() {
        let captured = Arc::new(Mutex::new(String::new()));
        let base = serve("{\"error\":\"model unloaded\"}\n", captured);
        let client = OllamaClient::new(&base);
        let error = client
            .stream("gemma4:latest", &[], &mut |_| Control::Continue)
            .unwrap_err();
        assert_eq!(error, ModelError::Failed("model unloaded".into()));
    }

    #[test]
    fn http_error_body_uses_the_error_field() {
        let captured = Arc::new(Mutex::new(String::new()));
        let base = serve_status(500, "Error", "{\"error\":\"model unloaded\"}", captured);
        let client = OllamaClient::new(&base);
        let error = client
            .stream("gemma4:latest", &[], &mut |_| Control::Continue)
            .unwrap_err();
        assert_eq!(error, ModelError::Failed("model unloaded".into()));
    }

    #[test]
    fn complete_reads_one_short_answer() {
        let captured = Arc::new(Mutex::new(String::new()));
        let base = serve(
            r#"{"message":{"role":"assistant","content":"Tell me a story\nWant to draw?"},"done":true}"#,
            captured.clone(),
        );
        let text = OllamaClient::new(&base)
            .complete(
                "gemma4:latest",
                &[ChatMessage {
                    role: Role::User,
                    content: "Hi".into(),
                }],
            )
            .unwrap();
        assert_eq!(text, "Tell me a story\nWant to draw?");
        let request = captured.lock().unwrap().clone();
        assert!(request.contains("\"stream\":false"));
        assert!(request.contains("\"num_predict\":80"));
        assert!(request.contains("\"think\":false"));
        assert!(request.contains("\"num_ctx\":8192"));
    }
}
