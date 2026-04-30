use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::{
    SelectedClip, apply_video_source_range, clip_temp_root, copy_project_components, error_json,
    project_root_for_composition, read_json, safe_id, safe_timeline_slug, selected_clip, value_str,
    value_u64, write_json,
};

pub(in crate::app::timeline_editor) fn write_clip_queue_proposal_composition(
    composition_path: &Path,
    params: &Value,
    job_id: &str,
) -> Result<PathBuf, String> {
    let queue = params
        .get("queue")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
        .ok_or_else(|| {
            error_json(
                "IPC_ERROR",
                "missing required parameter: queue",
                "next step · pass queue[] with clip_id, composition_path, start_ms and end_ms",
            )
        })?;
    let base_composition = read_json(composition_path)?;
    let source_root = project_root_for_composition(composition_path);
    let temp_root = clip_temp_root(params, &source_root, job_id)?;
    let temp_compositions = temp_root.join("compositions");
    fs::create_dir_all(&temp_compositions).map_err(|err| {
        error_json(
            "EXPORT_FAILED",
            format!("create clip queue proposal directory failed: {err}"),
            "next step · check export output permissions",
        )
    })?;
    copy_project_components(&source_root, &temp_root)?;

    let mut queued_clips = Vec::with_capacity(queue.len());
    let mut delivery_items = Vec::with_capacity(queue.len());
    let mut total_duration_ms = 0_u64;
    for (index, item) in queue.iter().enumerate() {
        let item_composition_path = queue_composition_path(item, composition_path);
        let composition = read_json(&item_composition_path)?;
        let clip = selected_clip(&composition, item)?;
        let start_ms = value_u64(item.get("start_ms")).unwrap_or(clip.start_ms);
        let end_ms = value_u64(item.get("end_ms")).unwrap_or(clip.end_ms);
        let duration_ms = end_ms.saturating_sub(start_ms).max(1);
        let sequence = value_u64(item.get("sequence")).unwrap_or((index + 1) as u64);
        let scene = queued_scene(item, &clip).to_string();
        let mut queued_clip = clip.value.clone();
        if let Some(object) = queued_clip.as_object_mut() {
            object.insert(
                "id".to_string(),
                json!(safe_timeline_slug(&format!(
                    "queue-{}-{}",
                    sequence,
                    safe_id(&clip.id)
                ))),
            );
            object.insert(
                "name".to_string(),
                json!(format!("{sequence:02} · {scene}")),
            );
            object.insert("duration".to_string(), json!(format!("{duration_ms}ms")));
            object.insert("duration_ms".to_string(), json!(duration_ms));
            apply_video_source_range(
                object,
                start_ms.saturating_sub(clip.start_ms),
                end_ms.saturating_sub(clip.start_ms),
            );
            object.insert(
                "source_range".to_string(),
                json!({
                    "sequence": sequence,
                    "clip_id": clip.id.clone(),
                    "scene": scene,
                    "start_ms": start_ms,
                    "end_ms": end_ms,
                    "duration_ms": duration_ms,
                    "source_composition_path": item_composition_path.display().to_string()
                }),
            );
        }
        queued_clips.push(queued_clip);
        let mut delivery_item = json!({
            "sequence": sequence,
            "clip_id": clip.id,
            "scene": scene,
            "start_ms": start_ms,
            "end_ms": end_ms,
            "duration_ms": duration_ms,
            "source_composition_path": item_composition_path.display().to_string()
        });
        if let Some(suggestion_id) = item.get("suggestion_id").and_then(Value::as_str) {
            delivery_item["suggestion_id"] = json!(suggestion_id);
        }
        if let Some(reason) = item.get("suggestion_reason").and_then(Value::as_str) {
            delivery_item["suggestion_reason"] = json!(reason);
        }
        if let Some(semantic_ref) = item.get("semantic_ref").and_then(Value::as_str) {
            delivery_item["semantic_ref"] = json!(semantic_ref);
        }
        if let Some(summary) = item.get("semantic_summary").and_then(Value::as_str) {
            delivery_item["semantic_summary"] = json!(summary);
        }
        if let Some(tags) = item.get("semantic_tags").and_then(Value::as_array) {
            delivery_item["semantic_tags"] = json!(tags);
        }
        if let Some(reason) = item.get("semantic_reason").and_then(Value::as_str) {
            delivery_item["semantic_reason"] = json!(reason);
        }
        delivery_items.push(delivery_item);
        total_duration_ms = total_duration_ms.saturating_add(duration_ms);
        let item_root = project_root_for_composition(&item_composition_path);
        if item_root != source_root {
            copy_project_components(&item_root, &temp_root)?;
        }
    }

    let mut queued = base_composition.clone();
    let object = queued.as_object_mut().ok_or_else(|| {
        error_json(
            "INVALID_COMPOSITION",
            "composition root must be an object",
            "next step · inspect composition JSON",
        )
    })?;
    object.insert(
        "id".to_string(),
        json!(safe_timeline_slug(&format!(
            "{}-clip-queue-delivery",
            value_str(&base_composition, "id").unwrap_or("composition")
        ))),
    );
    object.insert(
        "name".to_string(),
        json!(format!(
            "{} · 剪辑队列",
            value_str(&base_composition, "name").unwrap_or("Composition")
        )),
    );
    object.insert(
        "duration".to_string(),
        json!(format!("{total_duration_ms}ms")),
    );
    object.insert("duration_ms".to_string(), json!(total_duration_ms));
    object.insert("clips".to_string(), Value::Array(queued_clips));
    object.insert(
        "delivery".to_string(),
        json!({
            "kind": "video-clip-queue-proposal",
            "source_composition_path": composition_path.display().to_string(),
            "clip_count": queue.len(),
            "duration_ms": total_duration_ms,
            "items": delivery_items
        }),
    );

    let out = temp_compositions.join("main.json");
    write_json(&out, &queued)?;
    Ok(out)
}

fn queue_composition_path(item: &Value, fallback: &Path) -> PathBuf {
    item.get("composition_path")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| fallback.to_path_buf())
}

fn queued_scene<'a>(item: &'a Value, clip: &'a SelectedClip) -> &'a str {
    item.get("scene")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&clip.name)
}
