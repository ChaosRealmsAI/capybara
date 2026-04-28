use std::fs;
use std::path::PathBuf;

use serde_json::{Value, json};

pub(crate) fn load_canvas_context_packet(path: PathBuf) -> Result<Value, String> {
    let path = absolute_path(path)?;
    let text = fs::read_to_string(&path)
        .map_err(|err| format!("read canvas context packet failed: {err}"))?;
    let mut value: Value = serde_json::from_str(&text)
        .map_err(|err| format!("parse canvas context packet failed: {err}"))?;
    if value.get("context_json").is_none() {
        value["context_json"] = json!(path.display().to_string());
    }
    Ok(value)
}

pub(crate) fn prompt_with_canvas_context(prompt: &str, context: &Value) -> String {
    let context_id = context
        .get("context_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let kind = context
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("canvas_context");
    let source = context.get("source").unwrap_or(&Value::Null);
    let node = source.get("node").unwrap_or(&Value::Null);
    let node_id = node
        .get("id")
        .map(Value::to_string)
        .unwrap_or_else(|| "unknown".to_string());
    let title = node
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("untitled");
    let attachments = context
        .get("attachment_paths")
        .and_then(Value::as_array)
        .map(|paths| {
            paths
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    let region = context
        .get("geometry")
        .and_then(|geometry| geometry.get("region_world"))
        .filter(|value| !value.is_null())
        .map(Value::to_string)
        .unwrap_or_else(|| "none".to_string());
    format!(
        "{}\n\n[Canvas context packet]\ncontext_id={}\nkind={}\nsource_node_id={}\nsource_node_title={}\nregion_world={}\nattachment_paths:\n{}",
        prompt.trim(),
        context_id,
        kind,
        node_id,
        title,
        region,
        attachments
    )
}

fn absolute_path(path: PathBuf) -> Result<PathBuf, String> {
    if path.is_absolute() {
        return Ok(path);
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(path))
        .map_err(|err| format!("read cwd failed: {err}"))
}
