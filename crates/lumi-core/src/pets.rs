use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PetView {
    pub name: String,
    pub kind: String,
    pub motion: String,
    pub leaving_after_reply: bool,
    /// Changes when the person asks for a trick or an eager look again.
    pub beat: u32,
}

struct Animal {
    name: &'static str,
    display: &'static str,
    kind: &'static str,
    tricks: &'static [&'static str],
}

const ROSTER: &[Animal] = &[
    Animal {
        name: "biscuit",
        display: "Biscuit",
        kind: "puppy",
        tricks: &["sit", "bark", "jump", "roll", "spin", "paw"],
    },
    Animal {
        name: "puddle",
        display: "Puddle",
        kind: "duck",
        tricks: &["quack", "hop", "flap", "waddle", "sit"],
    },
    Animal {
        name: "miso",
        display: "Miso",
        kind: "cat",
        tricks: &["meow", "sit", "loaf", "pounce", "stretch"],
    },
    Animal {
        name: "crumb",
        display: "Crumb",
        kind: "hamster",
        tricks: &["squeak", "sit", "pop", "spin", "stand"],
    },
];

#[derive(Clone, Debug)]
enum Notice {
    Empty,
    Arrived,
    Trick,
    Eager,
    Still,
    Bored,
    Swapped {
        previous: &'static str,
        with_trick: bool,
    },
}

#[derive(Clone, Debug)]
pub struct PetVisit {
    pet: Option<usize>,
    off_topic: u8,
    motion: String,
    leaving: bool,
    notice: Notice,
    beat: u32,
}

impl PetVisit {
    pub fn new() -> Self {
        Self {
            pet: None,
            off_topic: 0,
            motion: String::new(),
            leaving: false,
            notice: Notice::Empty,
            beat: 0,
        }
    }

    fn bump(&mut self) {
        self.beat = self.beat.wrapping_add(1);
    }

    pub fn note_user_line(&mut self, text: &str) {
        if text.trim().is_empty() {
            return;
        }
        if self.leaving {
            self.off_topic = 1;
            self.leaving = false;
        }
        let words = words(text);
        if let Some(index) = last_named(&words) {
            let previous = self.pet;
            let trick = last_trick(index, &words);
            let swapped = previous.is_some() && previous != Some(index);
            self.pet = Some(index);
            self.off_topic = 0;
            self.leaving = false;
            if let Some(trick) = trick {
                self.motion = trick.to_string();
                self.bump();
                self.notice = if swapped {
                    Notice::Swapped {
                        previous: ROSTER[previous.expect("swapped from a pet")].display,
                        with_trick: true,
                    }
                } else {
                    Notice::Trick
                };
            } else if previous == Some(index) {
                self.motion = "eager".to_string();
                self.bump();
                self.notice = Notice::Eager;
            } else {
                self.motion = "wander".to_string();
                self.notice = if swapped {
                    Notice::Swapped {
                        previous: ROSTER[previous.expect("swapped from a pet")].display,
                        with_trick: false,
                    }
                } else {
                    Notice::Arrived
                };
            }
            return;
        }
        let Some(index) = self.pet else {
            return;
        };
        if let Some(trick) = last_trick(index, &words) {
            self.motion = trick.to_string();
            self.off_topic = 0;
            self.leaving = false;
            self.bump();
            self.notice = Notice::Trick;
            return;
        }
        self.off_topic = self.off_topic.saturating_add(1);
        if self.off_topic >= 2 {
            self.off_topic = 2;
            self.leaving = true;
            self.motion = "bored".to_string();
            self.notice = Notice::Bored;
        } else {
            self.leaving = false;
            self.notice = Notice::Still;
        }
    }

    pub fn finish_reply(&mut self) {
        if self.leaving {
            self.clear();
        }
    }

    pub fn clear(&mut self) {
        *self = Self::new();
    }

    pub fn view(&self) -> Option<PetView> {
        let index = self.pet?;
        let animal = &ROSTER[index];
        Some(PetView {
            name: animal.name.to_string(),
            kind: animal.kind.to_string(),
            motion: self.motion.clone(),
            leaving_after_reply: self.leaving,
            beat: self.beat,
        })
    }

    pub fn scene_line(&self) -> String {
        let Some(index) = self.pet else {
            return "No pet is in the room.".to_string();
        };
        let animal = &ROSTER[index];
        match self.notice {
            Notice::Empty | Notice::Arrived => {
                format!("{} the {} is here.", animal.display, animal.kind)
            }
            Notice::Trick => format!(
                "{} the {} is here and just did this: {}.",
                animal.display, animal.kind, self.motion
            ),
            Notice::Eager => {
                format!("{} the {} is here and perked up.", animal.display, animal.kind)
            }
            Notice::Still => format!("{} the {} is still here.", animal.display, animal.kind),
            Notice::Bored => format!(
                "{} the {} is bored and leaves during this reply. You may say so in a few words.",
                animal.display, animal.kind
            ),
            Notice::Swapped {
                previous,
                with_trick: true,
            } => format!(
                "{previous} left. {} the {} is here and just did this: {}.",
                animal.display, animal.kind, self.motion
            ),
            Notice::Swapped {
                previous,
                with_trick: false,
            } => format!(
                "{previous} left. {} the {} is here.",
                animal.display, animal.kind
            ),
        }
    }
}

fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_ascii_alphabetic())
        .filter(|word| !word.is_empty())
        .map(|word| word.to_ascii_lowercase())
        .collect()
}

fn last_named(words: &[String]) -> Option<usize> {
    let mut found = None;
    for word in words {
        if let Some(index) = ROSTER.iter().position(|animal| animal.name == word) {
            found = Some(index);
        }
    }
    found
}

fn last_trick(index: usize, words: &[String]) -> Option<&'static str> {
    let mut found = None;
    for word in words {
        if let Some(trick) = ROSTER[index].tricks.iter().copied().find(|trick| *trick == word) {
            found = Some(trick);
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn biscuit_sit_enters_and_sits() {
        let mut visit = PetVisit::new();
        visit.note_user_line("BISCUIT!");
        let view = visit.view().unwrap();
        assert_eq!(view.name, "biscuit");
        assert_eq!(view.motion, "wander");
        assert_eq!(visit.scene_line(), "Biscuit the puppy is here.");
        visit.note_user_line("Biscuit, sit");
        assert_eq!(visit.view().unwrap().motion, "sit");
        assert_eq!(
            visit.scene_line(),
            "Biscuit the puppy is here and just did this: sit."
        );
    }

    #[test]
    fn embedded_words_do_not_match() {
        let mut visit = PetVisit::new();
        visit.note_user_line("biscuits in this situation might spawn");
        assert!(visit.view().is_none());
        assert_eq!(visit.scene_line(), "No pet is in the room.");
        visit.note_user_line("Biscuit");
        visit.note_user_line("this situation might spawn");
        assert_eq!(visit.view().unwrap().motion, "wander");
        assert_eq!(visit.scene_line(), "Biscuit the puppy is still here.");
    }

    #[test]
    fn last_name_and_last_known_trick_win() {
        let mut visit = PetVisit::new();
        visit.note_user_line("Biscuit and Miso, spin");
        let view = visit.view().unwrap();
        assert_eq!(view.name, "miso");
        assert_eq!(view.motion, "wander");
        assert_eq!(visit.scene_line(), "Miso the cat is here.");
        visit.note_user_line("Puddle, sit and flap");
        assert_eq!(visit.view().unwrap().name, "puddle");
        assert_eq!(visit.view().unwrap().motion, "flap");
        assert_eq!(
            visit.scene_line(),
            "Miso left. Puddle the duck is here and just did this: flap."
        );
    }

    #[test]
    fn every_listed_trick_is_recognized() {
        let lines = [
            ("Biscuit, bark", "biscuit", "bark"),
            ("Biscuit, jump", "biscuit", "jump"),
            ("Biscuit, roll", "biscuit", "roll"),
            ("Biscuit, spin", "biscuit", "spin"),
            ("Biscuit, paw", "biscuit", "paw"),
            ("Puddle, quack", "puddle", "quack"),
            ("Puddle, hop", "puddle", "hop"),
            ("Puddle, waddle", "puddle", "waddle"),
            ("Miso, meow", "miso", "meow"),
            ("Miso, loaf", "miso", "loaf"),
            ("Miso, pounce", "miso", "pounce"),
            ("Miso, stretch", "miso", "stretch"),
            ("Crumb, squeak", "crumb", "squeak"),
            ("Crumb, pop", "crumb", "pop"),
            ("Crumb, stand", "crumb", "stand"),
        ];
        for (line, name, motion) in lines {
            let mut visit = PetVisit::new();
            visit.note_user_line(line);
            let view = visit.view().unwrap();
            assert_eq!(view.name, name, "{line}");
            assert_eq!(view.motion, motion, "{line}");
        }
    }

    #[test]
    fn sit_alone_does_nothing_until_someone_is_here() {
        let mut visit = PetVisit::new();
        visit.note_user_line("sit");
        assert!(visit.view().is_none());
        visit.note_user_line("Puddle");
        visit.note_user_line("sit");
        assert_eq!(visit.view().unwrap().motion, "sit");
        assert!(!visit.view().unwrap().leaving_after_reply);
    }

    #[test]
    fn two_off_topic_lines_then_finish_clears() {
        let mut visit = PetVisit::new();
        visit.note_user_line("Biscuit, roll");
        visit.note_user_line("What should we make for dinner?");
        assert_eq!(visit.scene_line(), "Biscuit the puppy is still here.");
        assert!(!visit.view().unwrap().leaving_after_reply);
        visit.note_user_line("What movie?");
        assert_eq!(visit.view().unwrap().motion, "bored");
        assert!(visit.view().unwrap().leaving_after_reply);
        assert_eq!(
            visit.scene_line(),
            "Biscuit the puppy is bored and leaves during this reply. You may say so in a few words."
        );
        visit.finish_reply();
        assert!(visit.view().is_none());
        visit.note_user_line("sit");
        assert!(visit.view().is_none());
    }

    #[test]
    fn a_mention_resets_and_an_unfinished_leave_does_not_dismiss() {
        let mut visit = PetVisit::new();
        visit.note_user_line("Crumb, pop");
        visit.note_user_line("dinner");
        visit.note_user_line("Crumb");
        assert_eq!(visit.view().unwrap().motion, "eager");
        assert_eq!(
            visit.scene_line(),
            "Crumb the hamster is here and perked up."
        );
        visit.note_user_line("dinner");
        visit.note_user_line("movie");
        assert!(visit.view().unwrap().leaving_after_reply);
        visit.note_user_line("what time is it");
        assert_eq!(visit.view().unwrap().name, "crumb");
        assert!(visit.view().unwrap().leaving_after_reply);
    }

    #[test]
    fn a_repeated_trick_bumps_the_beat() {
        let mut visit = PetVisit::new();
        visit.note_user_line("Biscuit, bark");
        let first = visit.view().unwrap().beat;
        visit.note_user_line("bark");
        let second = visit.view().unwrap();
        assert_eq!(second.motion, "bark");
        assert!(second.beat > first);
        visit.note_user_line("dinner");
        assert_eq!(visit.view().unwrap().beat, second.beat);
        assert_eq!(visit.view().unwrap().motion, "bark");
        visit.note_user_line("Crumb");
        let arrived = visit.view().unwrap().beat;
        visit.note_user_line("Crumb");
        let eager = visit.view().unwrap();
        assert_eq!(eager.motion, "eager");
        assert!(eager.beat > arrived);
    }

    #[test]
    fn clear_empties_the_stage() {
        let mut visit = PetVisit::new();
        visit.note_user_line("Miso, loaf");
        visit.clear();
        assert!(visit.view().is_none());
        assert_eq!(visit.scene_line(), "No pet is in the room.");
    }
}
