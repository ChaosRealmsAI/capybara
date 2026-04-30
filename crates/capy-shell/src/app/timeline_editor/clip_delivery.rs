use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

mod clip_queue_delivery;
pub(super) use clip_queue_delivery::write_clip_queue_proposal_composition;

pub(super) fn write_clip_proposal_composition(
    composition_path: &Path,
    params: &Value,
    job_id: &str,
) -> Result<PathBuf, String> {
    let range = params.get("range").ok_or_else(|| {
        error_json(
            "IPC_ERROR",
            "missing required parameter: range",
            "next step · pass range.start_ms and range.end_ms",
        )
    })?;
    let composition = read_json(composition_path)?;
    let clip = selected_clip(&composition, range)?;
    let start_ms = value_u64(range.get("start_ms")).unwrap_or(clip.start_ms);
    let end_ms = value_u64(range.get("end_ms")).unwrap_or(clip.end_ms);
    let duration_ms = end_ms.saturating_sub(start_ms).max(1);
    let mut clipped = composition.clone();
    let object = clipped.as_object_mut().ok_or_else(|| {
        error_json(
            "INVALID_COMPOSITION",
            "composition root must be an object",
            "next step · inspect composition JSON",
        )
    })?;
    object.insert(
        "id".to_string(),
        json!(safe_timeline_slug(&format!(
            "{}-{}-delivery",
            value_str(&composition, "id").unwrap_or("composition"),
            safe_id(&clip.id)
        ))),
    );
    object.insert(
        "name".to_string(),
        json!(format!(
            "{} · {} 片段",
            value_str(&composition, "name").unwrap_or("Composition"),
            clip.name
        )),
    );
    object.insert("duration".to_string(), json!(format!("{duration_ms}ms")));
    object.insert("duration_ms".to_string(), json!(duration_ms));
    object.insert("clips".to_string(), Value::Array(vec![clip.value.clone()]));
    if let Some(Value::Array(clips)) = object.get_mut("clips") {
        if let Some(Value::Object(selected)) = clips.first_mut() {
            selected.insert("duration".to_string(), json!(format!("{duration_ms}ms")));
            selected.insert("duration_ms".to_string(), json!(duration_ms));
            apply_video_source_range(
                selected,
                start_ms.saturating_sub(clip.start_ms),
                end_ms.saturating_sub(clip.start_ms),
            );
            selected.insert(
                "source_range".to_string(),
                json!({
                    "clip_id": clip.id.clone(),
                    "start_ms": start_ms,
                    "end_ms": end_ms,
                    "source_composition_path": composition_path.display().to_string()
                }),
            );
        }
    }
    object.insert(
        "delivery".to_string(),
        json!({
            "kind": "video-clip-proposal",
            "source_composition_path": composition_path.display().to_string(),
            "source_clip_id": clip.id.clone(),
            "start_ms": start_ms,
            "end_ms": end_ms,
            "duration_ms": duration_ms
        }),
    );

    let source_root = project_root_for_composition(composition_path);
    let temp_root = clip_temp_root(params, &source_root, job_id)?;
    let temp_compositions = temp_root.join("compositions");
    fs::create_dir_all(&temp_compositions).map_err(|err| {
        error_json(
            "EXPORT_FAILED",
            format!("create clip proposal directory failed: {err}"),
            "next step · check export output permissions",
        )
    })?;
    copy_project_components(&source_root, &temp_root)?;
    let out = temp_compositions.join("main.json");
    write_json(&out, &clipped)?;
    Ok(out)
}

#[derive(Debug, Clone)]
struct SelectedClip {
    id: String,
    name: String,
    start_ms: u64,
    end_ms: u64,
    value: Value,
}

fn selected_clip(composition: &Value, range: &Value) -> Result<SelectedClip, String> {
    let requested_id = range
        .get("clip_id")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty());
    let requested_start = value_u64(range.get("start_ms")).unwrap_or(0);
    let clips = composition
        .get("clips")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            error_json(
                "INVALID_COMPOSITION",
                "composition.clips must be an array for clip proposal export",
                "next step · use a clip-first composition JSON",
            )
        })?;
    let mut cursor = 0_u64;
    let mut fallback = None;
    for (index, clip) in clips.iter().enumerate() {
        let id = value_str(clip, "id")
            .map(str::to_string)
            .unwrap_or_else(|| format!("clip-{}", index + 1));
        let duration = clip_duration_ms(clip).unwrap_or(1000);
        let end = cursor.saturating_add(duration);
        let selected = requested_id == Some(id.as_str())
            || (requested_id.is_none() && requested_start >= cursor && requested_start < end);
        let item = SelectedClip {
            id: id.clone(),
            name: value_str(clip, "name").unwrap_or(&id).to_string(),
            start_ms: cursor,
            end_ms: end,
            value: clip.clone(),
        };
        if selected {
            return Ok(item);
        }
        if fallback.is_none() {
            fallback = Some(item);
        }
        cursor = end;
    }
    fallback.ok_or_else(|| {
        error_json(
            "INVALID_COMPOSITION",
            "composition.clips must not be empty",
            "next step · inspect composition JSON",
        )
    })
}

fn project_root_for_composition(composition_path: &Path) -> PathBuf {
    let composition_dir = composition_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    if composition_dir.file_name().and_then(|name| name.to_str()) == Some("compositions") {
        composition_dir
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or(composition_dir)
    } else {
        composition_dir
    }
}

fn clip_temp_root(params: &Value, source_root: &Path, job_id: &str) -> Result<PathBuf, String> {
    if let Some(out) = params
        .get("out")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        let output = if Path::new(out).is_absolute() {
            PathBuf::from(out)
        } else {
            std::env::current_dir()
                .map_err(|err| {
                    error_json(
                        "IPC_ERROR",
                        format!("read cwd failed: {err}"),
                        "next step · retry from a valid workspace",
                    )
                })?
                .join(out)
        };
        return Ok(output
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(".clip-proposals")
            .join(job_id));
    }
    Ok(source_root
        .join("exports")
        .join("clip-proposals")
        .join(job_id))
}

fn copy_project_components(source_root: &Path, temp_root: &Path) -> Result<(), String> {
    let source = source_root.join("components");
    let target = temp_root.join("components");
    if !source.is_dir() {
        return Ok(());
    }
    copy_dir_recursive(&source, &target)
}

fn apply_video_source_range(clip: &mut serde_json::Map<String, Value>, start_ms: u64, end_ms: u64) {
    let Some(tracks) = clip.get_mut("tracks").and_then(Value::as_array_mut) else {
        return;
    };
    for track in tracks {
        let Some(track_object) = track.as_object_mut() else {
            continue;
        };
        if track_object.get("kind").and_then(Value::as_str) != Some("video") {
            continue;
        }
        set_video_range_params(track_object, start_ms, end_ms);
        if let Some(items) = track_object.get_mut("items").and_then(Value::as_array_mut) {
            for item in items {
                if let Some(item_object) = item.as_object_mut() {
                    set_video_range_params(item_object, start_ms, end_ms);
                }
            }
        }
    }
}

fn set_video_range_params(target: &mut serde_json::Map<String, Value>, start_ms: u64, end_ms: u64) {
    let params = target
        .entry("params".to_string())
        .or_insert_with(|| json!({}));
    let Some(object) = params.as_object_mut() else {
        *params = json!({});
        let Some(object) = params.as_object_mut() else {
            return;
        };
        object.insert("source_start_ms".to_string(), json!(start_ms));
        object.insert("source_end_ms".to_string(), json!(end_ms));
        return;
    };
    object.insert("source_start_ms".to_string(), json!(start_ms));
    object.insert("source_end_ms".to_string(), json!(end_ms));
}

fn copy_dir_recursive(source: &Path, target: &Path) -> Result<(), String> {
    fs::create_dir_all(target).map_err(|err| {
        error_json(
            "EXPORT_FAILED",
            format!("create component directory failed: {err}"),
            "next step · check export output permissions",
        )
    })?;
    for entry in fs::read_dir(source).map_err(|err| {
        error_json(
            "EXPORT_FAILED",
            format!("read component directory failed: {err}"),
            "next step · check composition project components/",
        )
    })? {
        let entry = entry.map_err(|err| {
            error_json(
                "EXPORT_FAILED",
                format!("read component directory entry failed: {err}"),
                "next step · check composition project components/",
            )
        })?;
        let file_type = entry.file_type().map_err(|err| {
            error_json(
                "EXPORT_FAILED",
                format!("read component file type failed: {err}"),
                "next step · check composition project components/",
            )
        })?;
        let dest = target.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &dest)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), &dest).map_err(|err| {
                error_json(
                    "EXPORT_FAILED",
                    format!("copy component source failed: {err}"),
                    "next step · check composition project components/",
                )
            })?;
        }
    }
    Ok(())
}

fn clip_duration_ms(clip: &Value) -> Option<u64> {
    clip.get("duration_ms")
        .and_then(Value::as_u64)
        .or_else(|| time_value_ms(clip.get("duration").or_else(|| clip.get("length"))))
}

fn time_value_ms(value: Option<&Value>) -> Option<u64> {
    match value {
        Some(Value::Number(number)) => number.as_f64().map(|seconds| (seconds * 1000.0) as u64),
        Some(Value::String(raw)) => {
            let trimmed = raw.trim();
            if let Some(ms) = trimmed.strip_suffix("ms") {
                ms.trim()
                    .parse::<f64>()
                    .ok()
                    .map(|value| value.round() as u64)
            } else if let Some(seconds) = trimmed.strip_suffix('s') {
                seconds
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .map(|value| (value * 1000.0).round() as u64)
            } else {
                trimmed
                    .parse::<f64>()
                    .ok()
                    .map(|value| (value * 1000.0).round() as u64)
            }
        }
        _ => None,
    }
}

fn value_u64(value: Option<&Value>) -> Option<u64> {
    value
        .and_then(Value::as_u64)
        .or_else(|| value.and_then(Value::as_str)?.parse::<u64>().ok())
}

fn value_str<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn safe_id(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if sanitized.is_empty() {
        "clip".to_string()
    } else {
        sanitized
    }
}

fn safe_timeline_slug(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            let lower = ch.to_ascii_lowercase();
            if lower.is_ascii_alphanumeric() || lower == '-' || lower == '.' {
                lower
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    let slug = if sanitized
        .chars()
        .next()
        .map(|ch| ch.is_ascii_alphabetic())
        .unwrap_or(false)
    {
        sanitized
    } else {
        format!("clip-{sanitized}")
    };
    slug.chars().take(64).collect()
}

fn read_json(path: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(path).map_err(|err| {
        error_json(
            "COMPOSITION_READ_FAILED",
            format!("read JSON failed: {err}"),
            "next step · check file permissions",
        )
    })?;
    serde_json::from_str(&text).map_err(|err| {
        error_json(
            "COMPOSITION_INVALID",
            format!("JSON parse failed: {err}"),
            "next step · fix composition JSON",
        )
    })
}

fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    let mut text = serde_json::to_string_pretty(value).map_err(|err| {
        error_json(
            "COMPOSITION_INVALID",
            format!("serialize JSON failed: {err}"),
            "next step · inspect composition state",
        )
    })?;
    text.push('\n');
    fs::write(path, text).map_err(|err| {
        error_json(
            "COMPOSITION_WRITE_FAILED",
            format!("write JSON failed: {err}"),
            "next step · check file permissions",
        )
    })
}

fn error_json(code: &str, message: impl Into<String>, hint: &str) -> String {
    json!({
        "code": code,
        "message": message.into(),
        "hint": hint
    })
    .to_string()
}

#[cfg(test)]
#[path = "clip_delivery_tests.rs"]
mod clip_delivery_tests;
