use crate::conversation::{ChatMessage, Role};
use crate::speech_text::drop_filler;

pub const SUGGEST_ASK: &str = "Offer three short things I could say next, one on each line. Do not number them. Do not say oh or hey. Use the same language as our chat. Do not answer the chat.";

/// The chat so far, plus one instruction that is not saved in the transcript.
pub fn suggestion_messages(history: Vec<ChatMessage>) -> Vec<ChatMessage> {
    let mut messages = history;
    messages.push(ChatMessage {
        role: Role::User,
        content: SUGGEST_ASK.to_string(),
    });
    messages
}

/// Keeps up to three short lines a person could tap.
pub fn parse_suggestions(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    for raw in text.lines() {
        let cleaned = drop_filler(&strip_bullet(raw));
        let cleaned = cleaned.trim().trim_matches('"').trim().to_string();
        if cleaned.is_empty() || cleaned.chars().count() > 90 {
            continue;
        }
        if lines
            .iter()
            .any(|line: &String| line.eq_ignore_ascii_case(&cleaned))
        {
            continue;
        }
        lines.push(cleaned);
        if lines.len() == 3 {
            break;
        }
    }
    lines
}

fn strip_bullet(line: &str) -> String {
    let line = line.trim().trim_start_matches(['-', '*', '•']).trim();
    let rest = line
        .find(|c: char| !c.is_ascii_digit())
        .map(|index| {
            let (digits, tail) = line.split_at(index);
            if digits.is_empty() {
                line.to_string()
            } else if let Some(tail) = tail.strip_prefix(['.', ')', ':']) {
                tail.trim().to_string()
            } else {
                line.to_string()
            }
        })
        .unwrap_or_else(|| line.to_string());
    rest
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_lines_become_taps_and_filler_is_dropped() {
        let lines = parse_suggestions("1. Oh, tell me a story\n- Want to draw, hey?\n* Want to draw?\n3) What is the sky like?");
        assert_eq!(
            lines,
            vec![
                "Tell me a story".to_string(),
                "Want to draw?".to_string(),
                "What is the sky like?".to_string(),
            ]
        );
    }

    #[test]
    fn the_ask_is_not_part_of_the_saved_chat() {
        let history = vec![ChatMessage {
            role: Role::User,
            content: "Hi".into(),
        }];
        let request = suggestion_messages(history.clone());
        assert_eq!(history.len(), 1);
        assert!(request.last().unwrap().content.contains("three short"));
        assert_eq!(request.len(), 2);
    }
}
