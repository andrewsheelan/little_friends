use crate::pets::{PetView, PetVisit};
use crate::prompts::{system_prompt, TAMIL_NUDGE, TAMIL_REPLY};
use crate::settings::{Language, Personality};

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
    language: Language,
    messages: Vec<ChatMessage>,
    tools: Vec<Tool>,
    pets: PetVisit,
}

impl Conversation {
    pub fn new(personality: Personality) -> Self {
        Self {
            personality,
            language: Language::English,
            messages: Vec::new(),
            tools: Vec::new(),
            pets: PetVisit::new(),
        }
    }

    pub fn note_pet_line(&mut self, text: &str) {
        self.pets.note_user_line(text);
    }

    pub fn finish_pet_reply(&mut self) {
        self.pets.finish_reply();
    }

    pub fn clear_pet(&mut self) {
        self.pets.clear();
    }

    pub fn pet_view(&self) -> Option<PetView> {
        self.pets.view()
    }

    pub fn set_language(&mut self, language: Language) {
        self.language = language;
    }

    pub fn set_personality(&mut self, personality: Personality) {
        self.personality = personality;
    }

    pub fn take_messages(&mut self) -> Vec<ChatMessage> {
        std::mem::take(&mut self.messages)
    }

    pub fn replace_messages(&mut self, messages: Vec<ChatMessage>) {
        self.messages = messages;
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
        if matches!(
            self.messages.last().map(|message| message.role),
            Some(Role::Assistant)
        ) {
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
            content: {
                let mut prompt = system_prompt(self.personality).to_string();
                prompt.push(' ');
                prompt.push_str(&self.pets.scene_line());
                if self.language == Language::Tamil {
                    prompt.push_str(TAMIL_REPLY);
                }
                prompt
            },
        }];
        messages.extend(self.messages.iter().map(|message| ChatMessage {
            role: message.role,
            content: if message.role == Role::Assistant {
                crate::speech_text::drop_filler(&message.content)
            } else {
                message.content.clone()
            },
        }));
        if self.language == Language::Tamil {
            if let Some(newest) = messages.iter_mut().rev().find(|m| m.role == Role::User) {
                newest.content.push_str(TAMIL_NUDGE);
            }
        }
        messages
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompts::{system_prompt, PET_PARAGRAPH};
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
        assert_eq!(
            request[0].content,
            format!(
                "{} {}",
                system_prompt(Personality::Calm),
                "No pet is in the room."
            )
        );
        assert_eq!(request[1].content, "Hi");
        assert_eq!(request[2].content, "Hello");
        assert!(conversation.tools().is_empty());
    }

    #[test]
    fn an_old_reply_does_not_teach_the_next_one_to_say_hey() {
        let mut conversation = Conversation::new(Personality::Cheerful);
        conversation.push_user("Hi").unwrap();
        conversation.push_assistant_token("Oh, hello, hey!");
        let request = conversation.request_messages();
        assert_eq!(request[2].content, "Hello!");
        assert_eq!(conversation.messages()[1].content, "Oh, hello, hey!");
    }

    #[test]
    fn tamil_adds_a_tamil_reply_rule_without_erasing_the_personality() {
        let mut conversation = Conversation::new(Personality::Cheerful);
        conversation.set_language(Language::Tamil);
        let request = conversation.request_messages();
        assert!(request[0].content.starts_with(system_prompt(Personality::Cheerful)));
        assert!(request[0].content.contains("spoken Tamil"));
        assert!(request[0].content.contains("வணக்கம்"));
    }

    #[test]
    fn tamil_reminds_the_model_on_the_newest_user_message_only_in_the_request() {
        let mut conversation = Conversation::new(Personality::Calm);
        conversation.push_user("What is a robot?").unwrap();
        conversation.push_assistant_token("A machine that follows steps.");
        conversation.set_language(Language::Tamil);
        conversation.push_user("Thanks").unwrap();
        conversation.start_assistant();
        let request = conversation.request_messages();
        assert_eq!(request[1].content, "What is a robot?");
        assert_eq!(request[3].content, format!("Thanks{TAMIL_NUDGE}"));
        assert_eq!(conversation.messages()[2].content, "Thanks");
    }

    #[test]
    fn english_requests_have_no_tamil_reminder() {
        let mut conversation = Conversation::new(Personality::Calm);
        conversation.push_user("Thanks").unwrap();
        assert_eq!(conversation.request_messages()[1].content, "Thanks");
    }

    #[test]
    fn tamil_keeps_the_scene_sentence_ahead_of_the_tamil_rule() {
        let mut conversation = Conversation::new(Personality::Cheerful);
        conversation.set_language(Language::Tamil);
        conversation.note_pet_line("Biscuit, bark");
        let content = &conversation.request_messages()[0].content;
        let scene = content
            .find("Biscuit the puppy is here and just did this: bark.")
            .unwrap();
        let tamil = content.find("Reply only in spoken Tamil").unwrap();
        assert!(scene < tamil);
        assert!(content.contains(PET_PARAGRAPH));
    }
}
