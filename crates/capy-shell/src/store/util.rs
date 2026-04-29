use std::env;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use uuid::Uuid;

pub(super) fn app_support_dir() -> Result<PathBuf, String> {
    if cfg!(target_os = "macos") {
        let home = env::var_os("HOME").ok_or_else(|| "HOME is not set".to_string())?;
        return Ok(PathBuf::from(home).join("Library/Application Support/Capybara"));
    }
    let home = env::var_os("HOME").ok_or_else(|| "HOME is not set".to_string())?;
    Ok(PathBuf::from(home).join(".capybara"))
}

pub(super) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

pub(super) fn new_id(prefix: &str) -> String {
    format!("{prefix}_{}", Uuid::new_v4().simple())
}

pub(super) fn normalize_config(value: Value) -> Value {
    if value.is_object() { value } else { json!({}) }
}

pub(super) fn title_from_prompt(prompt: &str) -> String {
    let mut title = prompt
        .split_whitespace()
        .take(10)
        .collect::<Vec<_>>()
        .join(" ");
    if title.chars().count() > 60 {
        title = title.chars().take(60).collect();
    }
    if title.is_empty() {
        "Untitled conversation".to_string()
    } else {
        title
    }
}
