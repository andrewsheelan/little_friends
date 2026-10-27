/// Embedding models and speech models (Phonon voices the replies) are not chat models.
pub fn visible_models(tags: &[String]) -> Vec<String> {
    tags.iter()
        .filter(|name| !name.contains("embed") && !name.starts_with("phonon"))
        .cloned()
        .collect()
}

pub fn choose_model(saved: &str, tags: &[String]) -> Option<String> {
    let visible = visible_models(tags);
    if visible.iter().any(|name| name == saved) {
        return Some(saved.to_string());
    }
    if let Some(tag) = visible.iter().find(|name| name.as_str() == "gemma4:latest") {
        return Some(tag.clone());
    }
    if let Some(tag) = visible.iter().find(|name| name.starts_with("gemma4")) {
        return Some(tag.clone());
    }
    visible.first().cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embed_tags_are_hidden() {
        let tags = vec![
            "gemma4:latest".into(),
            "nomic-embed-text:latest".into(),
            "my-embed-model".into(),
        ];
        assert_eq!(visible_models(&tags), vec!["gemma4:latest".to_string()]);
    }

    #[test]
    fn the_speech_model_is_never_offered_as_a_chat_model() {
        let tags = vec!["phonon:latest".into(), "gemma4:latest".into()];
        assert_eq!(visible_models(&tags), vec!["gemma4:latest".to_string()]);
        assert_eq!(choose_model("phonon:latest", &tags), Some("gemma4:latest".into()));
        assert_eq!(choose_model("gone:latest", &["phonon:latest".into()]), None);
    }

    #[test]
    fn missing_saved_model_falls_through_the_spec_order() {
        assert_eq!(
            choose_model(
                "gone:latest",
                &["gemma4:latest".into(), "qwen3.6:latest".into()]
            ),
            Some("gemma4:latest".into())
        );
        assert_eq!(
            choose_model(
                "gone:latest",
                &["qwen3.6:latest".into(), "gemma4:e4b".into()]
            ),
            Some("gemma4:e4b".into())
        );
        assert_eq!(
            choose_model(
                "gone:latest",
                &[
                    "nomic-embed-text:latest".into(),
                    "qwen3.6:latest".into()
                ]
            ),
            Some("qwen3.6:latest".into())
        );
        assert_eq!(
            choose_model("gone:latest", &["nomic-embed-text:latest".into()]),
            None
        );
    }

    #[test]
    fn saved_visible_model_wins() {
        assert_eq!(
            choose_model(
                "qwen3.6:latest",
                &["gemma4:latest".into(), "qwen3.6:latest".into()]
            ),
            Some("qwen3.6:latest".into())
        );
    }
}
