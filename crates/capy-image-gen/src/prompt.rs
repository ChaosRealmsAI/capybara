use crate::{ImageGenError, Result};

pub const REQUIRED_PROMPT_SECTIONS: &[&str] = &[
    "Scene:",
    "Subject:",
    "Important details:",
    "Use case:",
    "Constraints:",
];

pub fn missing_prompt_sections(prompt: &str) -> Vec<&'static str> {
    let lower = prompt.to_lowercase();
    REQUIRED_PROMPT_SECTIONS
        .iter()
        .copied()
        .filter(|section| !lower.contains(&section.to_lowercase()))
        .collect()
}

pub fn validate_prompt_sections(prompt: &str) -> Result<()> {
    if prompt.trim().is_empty() {
        return Err(ImageGenError::Message(
            "image prompt cannot be empty".to_string(),
        ));
    }
    let missing = missing_prompt_sections(prompt);
    if missing.is_empty() {
        return Ok(());
    }
    Err(ImageGenError::Message(format!(
        "image prompt is missing required sections: {}",
        missing.join(", ")
    )))
}
