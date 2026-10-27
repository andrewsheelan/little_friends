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

pub struct StreamedReply {
    pub text: String,
    pub result: Result<(), ModelError>,
}

/// Streams one reply. Tokens already accepted are kept. Cancel, mid-stream
/// failure, and the voice-loaded status are decided by `outcome_of`, which is
/// the same function `run_turn` and the window both use.
pub fn stream_reply(
    client: &dyn ModelClient,
    model: &str,
    messages: &[ChatMessage],
    cancel: &AtomicBool,
    on_token: &mut dyn FnMut(&str),
) -> StreamedReply {
    let mut text = String::new();
    let result = client.stream(model, messages, &mut |token| {
        if cancel.load(Ordering::SeqCst) {
            return Control::Cancel;
        }
        on_token(token);
        text.push_str(token);
        if cancel.load(Ordering::SeqCst) {
            Control::Cancel
        } else {
            Control::Continue
        }
    });
    StreamedReply { text, result }
}

/// Maps a finished stream to what the window should keep, speak, and show.
pub fn outcome_of(
    assistant_text: String,
    voice_loaded: bool,
    result: Result<(), ModelError>,
) -> TurnOutcome {
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
        Err(ModelError::Cancelled) => TurnOutcome {
            assistant_text,
            speak: false,
            status: None,
            cancelled: true,
            called_model: true,
        },
        Err(ModelError::Failed(message)) => TurnOutcome {
            assistant_text,
            speak: false,
            status: Some(message),
            cancelled: false,
            called_model: true,
        },
    }
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
    let streamed = stream_reply(client, model, &messages, cancel, &mut |token| {
        conversation.push_assistant_token(token);
    });

    let assistant_text = conversation
        .messages()
        .last()
        .filter(|message| message.role == crate::conversation::Role::Assistant)
        .map(|message| message.content.clone())
        .unwrap_or_default();

    if matches!(streamed.result, Err(ModelError::Cancelled)) && assistant_text.is_empty() {
        conversation.messages_mut().pop();
    }
    outcome_of(assistant_text, voice_loaded, streamed.result)
}

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
            messages: &[crate::conversation::ChatMessage],
            on_token: &mut dyn FnMut(&str) -> Control,
        ) -> Result<(), ModelError> {
            assert!(!messages.last().is_some_and(|message| {
                message.role == crate::conversation::Role::Assistant && message.content.is_empty()
            }));
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
