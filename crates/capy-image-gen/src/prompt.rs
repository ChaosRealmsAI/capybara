use crate::{ImageGenError, Result};

pub const REQUIRED_PROMPT_SECTIONS: &[&str] = &[
    "Scene:",
    "Subject:",
    "Important details:",
    "Use case:",
    "Constraints:",
];

const CUTOUT_REQUIREMENTS: &[(&str, &[&[&str]])] = &[
    (
        "neutral matte gray background (#E0E0E0 default, #E8E8E8 for dark subjects, #B8BEC3 for light subjects)",
        &[&["#e0e0e0"], &["#e8e8e8"], &["#b8bec3"]],
    ),
    (
        "single isolated foreground subject",
        &[&["single"], &["one"]],
    ),
    (
        "fully visible uncropped subject",
        &[&["fully visible"], &["not cropped"], &["uncropped"]],
    ),
    (
        "clean separated silhouette edges",
        &[
            &["clean silhouette"],
            &["clear edges"],
            &["edge separation"],
            &["separated edges"],
            &["strong separation"],
        ],
    ),
    (
        "no extra objects",
        &[&["no extra objects"], &["no other objects"]],
    ),
    ("no text", &[&["no text"]]),
    ("no watermark", &[&["no watermark"]]),
    ("no green screen", &[&["no green"]]),
    ("no blue screen", &[&["no blue"]]),
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

pub fn missing_cutout_prompt_requirements(prompt: &str) -> Vec<&'static str> {
    let lower = prompt.to_lowercase();
    CUTOUT_REQUIREMENTS
        .iter()
        .filter_map(|(label, alternatives)| {
            let ok = alternatives
                .iter()
                .any(|terms| terms.iter().all(|term| lower.contains(term)));
            if ok { None } else { Some(*label) }
        })
        .collect()
}

pub fn validate_cutout_prompt(prompt: &str) -> Result<()> {
    validate_prompt_sections(prompt)?;
    let missing = missing_cutout_prompt_requirements(prompt);
    if missing.is_empty() {
        return Ok(());
    }
    Err(ImageGenError::Message(format!(
        "cutout-ready image prompt is missing requirements: {}",
        missing.join(", ")
    )))
}
