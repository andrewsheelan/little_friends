use crate::settings::Personality;

/// One piece of a spoken reply. Pauses are silence, not words for the model to read.
#[derive(Clone, Debug, PartialEq)]
pub enum SpokenBeat {
    Words(String),
    PauseMs(u32),
}

/// Drops the words oh and hey. A question mark or exclamation that belonged
/// to one of those words stays on the sentence before it.
pub fn drop_filler(text: &str) -> String {
    let mut kept: Vec<String> = Vec::new();
    for token in text.split_whitespace() {
        let (word, punct) = peel(token);
        if word.eq_ignore_ascii_case("oh") || word.eq_ignore_ascii_case("hey") {
            if let Some(mark) = punct.chars().find(|c| *c == '?' || *c == '!') {
                if let Some(last) = kept.last_mut() {
                    while last.ends_with([',', ';', ':']) {
                        last.pop();
                    }
                    if !last.ends_with(['.', '!', '?']) {
                        last.push(mark);
                    }
                }
            }
            continue;
        }
        kept.push(token.to_string());
    }
    capitalize_first(&kept.join(" "))
}

fn peel(token: &str) -> (&str, &str) {
    let cut = token
        .find(|c: char| !c.is_ascii_alphabetic())
        .unwrap_or(token.len());
    (&token[..cut], &token[cut..])
}

fn capitalize_first(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() => format!("{}{}", c.to_ascii_uppercase(), chars.as_str()),
        Some(c) => format!("{c}{}", chars.as_str()),
        None => String::new(),
    }
}

/// True when the reply has Tamil letters, so the Tamil voice should speak it.
/// An English reply in Tamil mode is spoken with the English voice instead.
pub fn spoken_in_tamil(text: &str) -> bool {
    text.chars().any(|c| ('\u{0B80}'..='\u{0BFF}').contains(&c))
}

/// The reply as plain sentences for a voice that paces itself. Filler words,
/// markdown marks, and held ellipses are gone. Nothing is added.
pub fn natural_speech(text: &str) -> String {
    soften_holds(drop_filler(&strip_markup(text)))
}

/// Turns a finished reply into phrases Kokoro can say. The words stay as
/// written. The only added silence is a short beat between sentences.
pub fn spoken_beats(personality: Personality, text: &str) -> Vec<SpokenBeat> {
    let clean = natural_speech(text);
    if clean.is_empty() {
        return Vec::new();
    }
    let mut cheer_left = 2u8;
    let mut sentences = split_sentences(&clean);
    if sentences.is_empty() {
        sentences.push(clean.clone());
    }
    let mut shaped = Vec::new();
    for sentence in &sentences {
        if let Some(line) = shape_sentence(personality, sentence, &mut cheer_left) {
            shaped.push(line);
        }
    }
    if shaped.is_empty() {
        return Vec::new();
    }
    let mut beats = Vec::new();
    for (index, line) in shaped.into_iter().enumerate() {
        if index > 0 {
            push_pause(&mut beats, sentence_gap(personality, &line));
        }
        push_words(&mut beats, line);
    }
    beats
}

fn shape_sentence(personality: Personality, sentence: &str, cheer_left: &mut u8) -> Option<String> {
    let (body, terminator) = split_terminator(sentence);
    let body = body.trim();
    if body.is_empty() {
        return None;
    }
    let words = word_count(body);
    let terminator = spoken_terminator(personality, words, terminator, sentence, cheer_left);
    Some(format!("{}{terminator}", with_breath(body)))
}

/// A comma inside the sentence, so the breath stays in one spoken line.
fn with_breath(body: &str) -> String {
    if body.contains([',', ';', ':']) || body.contains(" - ") {
        return body.to_string();
    }
    let words: Vec<&str> = body.split_whitespace().collect();
    if words.len() < 14 {
        return body.to_string();
    }
    let mut cut = None;
    for (index, word) in words.iter().enumerate() {
        if index < 4 || index + 3 > words.len() {
            continue;
        }
        if is_join(word) {
            cut = Some(index);
            break;
        }
    }
    let cut = cut.unwrap_or_else(|| snap_cut(&words, words.len() / 2));
    if cut == 0 || cut >= words.len() {
        return body.to_string();
    }
    format!("{}, {}", words[..cut].join(" "), words[cut..].join(" "))
}

fn soften_holds(text: String) -> String {
    text.replace("...", ",")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn spoken_terminator(
    personality: Personality,
    words: usize,
    terminator: &str,
    sentence: &str,
    cheer_left: &mut u8,
) -> String {
    if personality == Personality::Cheerful
        && terminator == "."
        && *cheer_left > 0
        && (words <= 8 || mix(sentence) % 2 == 0)
    {
        *cheer_left -= 1;
        return "!".to_string();
    }
    terminator.to_string()
}

fn snap_cut(words: &[&str], mut cut: usize) -> usize {
    while cut > 4 && is_small(words[cut - 1]) {
        cut -= 1;
    }
    cut
}

fn is_join(word: &str) -> bool {
    matches!(
        bare(word).as_str(),
        "and" | "but" | "so" | "because" | "when" | "while" | "if" | "or"
    )
}

fn is_small(word: &str) -> bool {
    matches!(
        bare(word).as_str(),
        "a" | "an" | "the" | "to" | "of" | "in" | "on" | "for" | "with"
    )
}

fn bare(word: &str) -> String {
    word.trim_matches(|c: char| !c.is_ascii_alphabetic())
        .to_ascii_lowercase()
}

fn sentence_gap(personality: Personality, sentence: &str) -> u32 {
    let base = match personality {
        Personality::Cheerful => 140,
        Personality::Study => 180,
        Personality::Calm => 220,
        Personality::Dad => 260,
    };
    jitter(sentence, base, 20)
}

fn push_words(beats: &mut Vec<SpokenBeat>, text: String) {
    let text = text.trim().to_string();
    if !text.is_empty() {
        beats.push(SpokenBeat::Words(text));
    }
}

fn push_pause(beats: &mut Vec<SpokenBeat>, ms: u32) {
    if ms > 0 && matches!(beats.last(), Some(SpokenBeat::Words(_))) {
        beats.push(SpokenBeat::PauseMs(ms));
    }
}

fn word_count(text: &str) -> usize {
    text.split_whitespace().filter(|word| !word.is_empty()).count()
}

fn split_terminator(sentence: &str) -> (&str, &str) {
    let trimmed = sentence.trim_end();
    let end = trimmed.trim_end_matches(['.', '!', '?']);
    if end.len() == trimmed.len() {
        return (trimmed, "");
    }
    (end.trim_end(), &trimmed[end.len()..])
}

fn split_sentences(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut sentences = Vec::new();
    let mut start = 0;
    let mut index = 0;
    while index < chars.len() {
        if !ends_sentence(&chars, index) {
            index += 1;
            continue;
        }
        let mut end = index + 1;
        if chars[index] == '.' {
            while end < chars.len() && chars[end] == '.' {
                end += 1;
            }
        }
        while end < chars.len() && chars[end].is_whitespace() {
            end += 1;
        }
        let sentence: String = chars[start..end].iter().collect();
        let sentence = sentence.trim().to_string();
        if !sentence.is_empty() {
            sentences.push(sentence);
        }
        start = end;
        index = end;
    }
    if start < chars.len() {
        let rest: String = chars[start..].iter().collect();
        let rest = rest.trim().to_string();
        if !rest.is_empty() {
            sentences.push(rest);
        }
    }
    sentences
}

fn ends_sentence(chars: &[char], index: usize) -> bool {
    let mark = chars[index];
    if mark != '.' && mark != '!' && mark != '?' {
        return false;
    }
    if mark == '.' && is_abbreviation(chars, index) {
        return false;
    }
    if mark == '.'
        && index > 0
        && chars[index - 1].is_ascii_digit()
        && index + 1 < chars.len()
        && chars[index + 1].is_ascii_digit()
    {
        return false;
    }
    index + 1 == chars.len()
        || chars[index + 1].is_whitespace()
        || (mark == '.' && chars[index + 1] == '.')
}

fn is_abbreviation(chars: &[char], period: usize) -> bool {
    let mut start = period;
    while start > 0 && chars[start - 1].is_ascii_alphabetic() {
        start -= 1;
    }
    let word: String = chars[start..period].iter().collect();
    matches!(
        word.to_ascii_lowercase().as_str(),
        "dr" | "mr" | "mrs" | "ms" | "st" | "jr" | "sr" | "etc" | "vs"
    )
}

fn strip_markup(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        if matches!(ch, '*' | '_' | '#' | '`' | '>') {
            out.push(' ');
        } else {
            out.push(ch);
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn jitter(seed: &str, base: u32, spread: u32) -> u32 {
    let span = spread * 2 + 1;
    let pick = (mix(seed) % u64::from(span)) as u32;
    base + pick - spread
}

fn mix(text: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(beats: &[SpokenBeat]) -> Vec<&str> {
        beats
            .iter()
            .filter_map(|beat| match beat {
                SpokenBeat::Words(text) => Some(text.as_str()),
                SpokenBeat::PauseMs(_) => None,
            })
            .collect()
    }

    fn pauses(beats: &[SpokenBeat]) -> Vec<u32> {
        beats
            .iter()
            .filter_map(|beat| match beat {
                SpokenBeat::PauseMs(ms) => Some(*ms),
                SpokenBeat::Words(_) => None,
            })
            .collect()
    }

    #[test]
    fn natural_speech_keeps_the_words_and_drops_the_marks() {
        assert_eq!(
            natural_speech("Oh, **hello** there... how are you, hey?"),
            "Hello there, how are you?"
        );
        assert_eq!(natural_speech("  Hey hey.  "), "");
        assert_eq!(natural_speech("It is 3.5 miles."), "It is 3.5 miles.");
    }

    #[test]
    fn blank_text_says_nothing() {
        assert!(spoken_beats(Personality::Cheerful, "  \n").is_empty());
    }

    #[test]
    fn two_sentences_keep_a_pause_between_them() {
        let beats = spoken_beats(Personality::Calm, "I can help. Tell me the next part.");
        let said = words(&beats).join(" ");
        assert!(said.contains("help"));
        assert!(said.contains("next part"));
        assert!(pauses(&beats).iter().any(|ms| (200..=240).contains(ms)));
        assert!(!matches!(beats.last(), Some(SpokenBeat::PauseMs(_))));
    }

    #[test]
    fn oh_and_hey_are_taken_out_of_a_reply() {
        assert_eq!(drop_filler("Oh, you are welcome!"), "You are welcome!");
        assert_eq!(
            drop_filler("What should we talk about now, hey?"),
            "What should we talk about now?"
        );
        assert_eq!(
            drop_filler("Hey, I am Lumi! I am your friend, hey!"),
            "I am Lumi! I am your friend!"
        );
        assert_eq!(drop_filler("Ohio is a place."), "Ohio is a place.");
        assert_eq!(drop_filler("Hey, hey, hey, yeah."), "Yeah.");
        assert_eq!(drop_filler("Hey hey hey."), "");
        let beats = spoken_beats(Personality::Cheerful, "Oh, the park is open today, hey!");
        let said = words(&beats).join(" ");
        assert!(!said.to_ascii_lowercase().contains(" oh"));
        assert!(!said.to_ascii_lowercase().contains("hey"));
        assert!(said.contains("park is open today"));
    }

    #[test]
    fn a_flat_line_is_not_given_an_oh_or_hey() {
        let text = "The park is open today.";
        let beats = spoken_beats(Personality::Cheerful, text);
        let said = words(&beats);
        assert_eq!(said.len(), 1);
        assert!(pauses(&beats).is_empty());
        assert!(said[0].ends_with("park is open today!"));
        assert!(!said[0].starts_with("Oh,") && !said[0].starts_with("Hey,"));
    }

    #[test]
    fn an_existing_reaction_is_not_stacked() {
        let beats = spoken_beats(Personality::Study, "Hmm, I think the answer is four.");
        let said = words(&beats).join(" ");
        assert!(said.starts_with("Hmm,"));
        assert_eq!(said.matches("Hmm").count(), 1);
    }

    #[test]
    fn a_long_line_breathes_before_and() {
        let beats = spoken_beats(
            Personality::Study,
            "I went down to the park and I saw a huge red kite over the trees.",
        );
        let said = words(&beats).join(" ");
        assert!(said.contains("park, and I saw a huge red kite over the trees."));
        assert!(
            pauses(&beats).is_empty(),
            "a breath stays inside the sentence"
        );
    }

    #[test]
    fn calm_does_not_turn_a_period_into_an_exclamation() {
        let beats = spoken_beats(Personality::Calm, "The park is open today.");
        let said = words(&beats).join(" ");
        assert!(said.contains("park is open today."));
        assert!(!said.contains("park is open today!"));
    }

    #[test]
    fn questions_and_markup_and_decimals_stay_intact() {
        let beats = spoken_beats(Personality::Cheerful, "Is **pi** about 3.14 today?");
        let said = words(&beats).join(" ");
        assert!(said.contains("pi about 3.14 today?"));
        assert!(!said.contains('*'));
    }

    #[test]
    fn a_tamil_reply_is_spoken_in_tamil_and_an_english_one_is_not() {
        assert!(spoken_in_tamil("வணக்கம், நான் லூமி."));
        assert!(spoken_in_tamil("OK, நன்றி."));
        assert!(!spoken_in_tamil("Hello, I am Lumi."));
        assert!(!spoken_in_tamil(""));
        assert!(!spoken_in_tamil("123!"));
    }
}
