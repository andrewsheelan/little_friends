//! Copies conversations into a folder the person chooses: Google Drive, a
//! USB stick, or any other disk. The app never talks to a cloud API.

use crate::settings::{PersonMemories, Personality, StoredMessage};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The markdown file for one character.
pub fn person_file(personality: Personality) -> &'static str {
    match personality {
        Personality::Cheerful => "Lumi.md",
        Personality::Calm => "Pip.md",
        Personality::Study => "Milo.md",
        Personality::Dad => "Hank.md",
    }
}

pub fn person_name(personality: Personality) -> &'static str {
    match personality {
        Personality::Cheerful => "Lumi",
        Personality::Calm => "Pip",
        Personality::Study => "Milo",
        Personality::Dad => "Hank",
    }
}

/// Last folder name, for the button. Empty means backup is off.
pub fn backup_label(dir: &str) -> String {
    let dir = dir.trim();
    if dir.is_empty() {
        return "Off".into();
    }
    Path::new(dir)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(dir)
        .to_string()
}

/// Turns one person's saved lines into a readable note.
pub fn transcript_markdown(name: &str, messages: &[StoredMessage]) -> String {
    let mut out = format!("# {name}\n");
    for message in messages {
        let who = match message.role.as_str() {
            "user" => "You",
            "assistant" => name,
            _ => continue,
        };
        out.push_str("\n");
        out.push_str(who);
        out.push_str(": ");
        out.push_str(message.content.trim());
        out.push('\n');
    }
    out
}

/// Writes `memories.json` and one markdown file per person. Does nothing when
/// `dir` is empty so an unset backup never creates a folder.
pub fn sync_backup(dir: &str, memories: &PersonMemories) -> io::Result<()> {
    let dir = dir.trim();
    if dir.is_empty() {
        return Ok(());
    }
    write_backup(Path::new(dir), memories)
}

fn write_backup(dir: &Path, memories: &PersonMemories) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let json = serde_json::to_string_pretty(memories).expect("memories serialize");
    fs::write(dir.join("memories.json"), json)?;
    let people = [
        (Personality::Cheerful, &memories.cheerful),
        (Personality::Calm, &memories.calm),
        (Personality::Study, &memories.study),
        (Personality::Dad, &memories.dad),
    ];
    for (personality, messages) in people {
        let name = person_name(personality);
        fs::write(
            dir.join(person_file(personality)),
            transcript_markdown(name, messages),
        )?;
    }
    Ok(())
}

/// A sensible place to start the folder picker: Google Drive if it is mounted.
pub fn backup_picker_start(home: &Path) -> PathBuf {
    let cloud = home.join("Library/CloudStorage");
    if let Ok(entries) = fs::read_dir(&cloud) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            if name.to_string_lossy().starts_with("GoogleDrive") {
                return entry.path();
            }
        }
    }
    home.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(role: &str, content: &str) -> StoredMessage {
        StoredMessage {
            role: role.into(),
            content: content.into(),
        }
    }

    #[test]
    fn each_person_has_a_named_note() {
        assert_eq!(person_file(Personality::Cheerful), "Lumi.md");
        assert_eq!(person_file(Personality::Calm), "Pip.md");
        assert_eq!(person_file(Personality::Study), "Milo.md");
        assert_eq!(person_file(Personality::Dad), "Hank.md");
    }

    #[test]
    fn the_button_shows_off_or_the_folder_name() {
        assert_eq!(backup_label(""), "Off");
        assert_eq!(backup_label("  "), "Off");
        assert_eq!(
            backup_label("/Users/me/Library/CloudStorage/GoogleDrive-me/My Drive/Lumi"),
            "Lumi"
        );
    }

    #[test]
    fn a_chat_is_written_as_you_and_the_person() {
        let text = transcript_markdown(
            "Lumi",
            &[line("user", "Hello"), line("assistant", "Hi there")],
        );
        assert_eq!(text, "# Lumi\n\nYou: Hello\n\nLumi: Hi there\n");
    }

    #[test]
    fn an_empty_chat_is_just_the_heading() {
        assert_eq!(transcript_markdown("Pip", &[]), "# Pip\n");
    }

    #[test]
    fn an_empty_folder_does_not_create_files() {
        let dir = std::env::temp_dir().join(format!("lumi-backup-empty-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        sync_backup("", &PersonMemories::default()).unwrap();
        assert!(!dir.exists());
    }

    #[test]
    fn write_backup_writes_json_and_each_person() {
        let dir = std::env::temp_dir().join(format!("lumi-backup-write-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let memories = PersonMemories {
            cheerful: vec![line("user", "Hello Lumi")],
            calm: vec![line("assistant", "Pip here")],
            study: Vec::new(),
            dad: Vec::new(),
        };
        sync_backup(dir.to_str().unwrap(), &memories).unwrap();
        assert_eq!(load_pretty(&dir.join("memories.json")), memories);
        assert!(fs::read_to_string(dir.join("Lumi.md"))
            .unwrap()
            .contains("You: Hello Lumi"));
        assert!(fs::read_to_string(dir.join("Pip.md"))
            .unwrap()
            .contains("Pip: Pip here"));
        assert_eq!(fs::read_to_string(dir.join("Milo.md")).unwrap(), "# Milo\n");
        assert_eq!(fs::read_to_string(dir.join("Hank.md")).unwrap(), "# Hank\n");
        fs::remove_dir_all(&dir).unwrap();
    }

    fn load_pretty(path: &Path) -> PersonMemories {
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn the_picker_starts_in_google_drive_when_it_is_there() {
        let home = std::env::temp_dir().join(format!("lumi-backup-home-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        let drive = home.join("Library/CloudStorage/GoogleDrive-me");
        fs::create_dir_all(&drive).unwrap();
        assert_eq!(backup_picker_start(&home), drive);
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn the_picker_starts_at_home_when_drive_is_missing() {
        let home = std::env::temp_dir().join(format!("lumi-backup-nodrive-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        fs::create_dir_all(&home).unwrap();
        assert_eq!(backup_picker_start(&home), home);
        fs::remove_dir_all(&home).unwrap();
    }
}
