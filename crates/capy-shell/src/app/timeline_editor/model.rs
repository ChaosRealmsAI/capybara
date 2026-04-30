use serde_json::{Map, Value, json};

pub(super) fn editor_summary(composition: &Value, render_source: Option<&Value>) -> Value {
    let duration_ms = render_source
        .and_then(|source| source.get("duration_ms").and_then(Value::as_u64))
        .unwrap_or_else(|| composition_duration_ms(composition));
    let clips = clip_summaries(composition);
    let tracks = track_summaries(composition);
    json!({
        "id": composition.get("id").and_then(Value::as_str).unwrap_or("composition"),
        "name": composition
            .get("name")
            .or_else(|| composition.get("title"))
            .and_then(Value::as_str)
            .unwrap_or("Composition"),
        "duration_ms": duration_ms,
        "viewport": composition.get("viewport").cloned().unwrap_or_else(|| json!({"w": 1920, "h": 1080, "ratio": "16:9"})),
        "source_video": source_video_summary(composition),
        "clips": clips,
        "tracks": tracks,
        "render_source_tracks": render_source
            .and_then(|source| source.get("tracks").and_then(Value::as_array))
            .map(Vec::len)
            .unwrap_or(0)
    })
}

fn source_video_summary(composition: &Value) -> Option<Value> {
    let source_artifact = composition.get("source_artifact").cloned();
    let clips = composition.get("clips").and_then(Value::as_array)?;
    for clip in clips {
        let tracks = clip.get("tracks").and_then(Value::as_array)?;
        for track in tracks {
            if track.get("kind").and_then(Value::as_str) != Some("video") {
                continue;
            }
            let params = track.get("params").unwrap_or(&Value::Null);
            return Some(json!({
                "filename": params
                    .get("filename")
                    .and_then(Value::as_str)
                    .or_else(|| clip.get("name").and_then(Value::as_str))
                    .unwrap_or("video"),
                "duration_ms": params
                    .get("duration_ms")
                    .and_then(Value::as_u64)
                    .or_else(|| clip_duration_ms(clip)),
                "width": params.get("width").and_then(Value::as_u64),
                "height": params.get("height").and_then(Value::as_u64),
                "src": params.get("src").and_then(Value::as_str),
                "source_path": params.get("source_path").and_then(Value::as_str),
                "artifact": source_artifact
            }));
        }
    }
    None
}

pub(super) fn patch_track_field(
    composition: &mut Value,
    track_id: &str,
    field: &str,
    value: Value,
) -> bool {
    if let Some(clips) = composition.get_mut("clips").and_then(Value::as_array_mut) {
        for clip in clips {
            let clip_id = value_string(clip, "id").unwrap_or_default();
            let Some(tracks) = clip.get_mut("tracks").and_then(Value::as_array_mut) else {
                continue;
            };
            for track in tracks {
                let local_id = value_string(track, "id").unwrap_or_default();
                if track_id != local_id && track_id != format!("{clip_id}.{local_id}") {
                    continue;
                }
                set_track_field(track, field, value);
                return true;
            }
        }
    }
    if let Some(tracks) = composition.get_mut("tracks").and_then(Value::as_array_mut) {
        for track in tracks {
            if value_string(track, "id").as_deref() == Some(track_id) {
                set_track_field(track, field, value);
                return true;
            }
        }
    }
    false
}

fn clip_summaries(composition: &Value) -> Vec<Value> {
    let mut cursor = 0_u64;
    composition
        .get("clips")
        .and_then(Value::as_array)
        .map(|clips| {
            clips
                .iter()
                .enumerate()
                .map(|(index, clip)| {
                    let duration = clip_duration_ms(clip).unwrap_or(1000);
                    let id = value_string(clip, "id").unwrap_or_else(|| format!("clip-{}", index + 1));
                    let item = json!({
                        "id": id,
                        "name": value_string(clip, "name").unwrap_or_else(|| id.clone()),
                        "start_ms": cursor,
                        "duration_ms": duration,
                        "end_ms": cursor + duration,
                        "track_count": clip.get("tracks").and_then(Value::as_array).map(Vec::len).unwrap_or(0)
                    });
                    cursor += duration;
                    item
                })
                .collect()
        })
        .unwrap_or_else(|| {
            vec![json!({
                "id": "composition",
                "name": value_string(composition, "name").unwrap_or_else(|| "Composition".to_string()),
                "start_ms": 0,
                "duration_ms": composition_duration_ms(composition),
                "end_ms": composition_duration_ms(composition),
                "track_count": composition.get("tracks").and_then(Value::as_array).map(Vec::len).unwrap_or(0)
            })]
        })
}

fn track_summaries(composition: &Value) -> Vec<Value> {
    if let Some(clips) = composition.get("clips").and_then(Value::as_array) {
        let mut cursor = 0_u64;
        let mut out = Vec::new();
        for (clip_index, clip) in clips.iter().enumerate() {
            let clip_id =
                value_string(clip, "id").unwrap_or_else(|| format!("clip-{}", clip_index + 1));
            let clip_duration = clip_duration_ms(clip).unwrap_or(1000);
            if let Some(tracks) = clip.get("tracks").and_then(Value::as_array) {
                for (track_index, track) in tracks.iter().enumerate() {
                    let track_id = value_string(track, "id")
                        .unwrap_or_else(|| format!("track-{}", track_index + 1));
                    let fields = editable_fields(track);
                    out.push(json!({
                        "id": format!("{clip_id}.{track_id}"),
                        "clip_id": clip_id,
                        "local_id": track_id,
                        "label": track_label(track, &fields),
                        "kind": value_string(track, "kind").unwrap_or_else(|| "component".to_string()),
                        "component": value_string(track, "component"),
                        "z": track.get("z").and_then(Value::as_i64).unwrap_or(track_index as i64),
                        "start_ms": cursor,
                        "duration_ms": clip_duration,
                        "end_ms": cursor + clip_duration,
                        "fields": fields
                    }));
                }
            }
            cursor += clip_duration;
        }
        return out;
    }

    composition
        .get("tracks")
        .and_then(Value::as_array)
        .map(|tracks| {
            tracks
                .iter()
                .enumerate()
                .map(|(index, track)| {
                    let id =
                        value_string(track, "id").unwrap_or_else(|| format!("track-{}", index + 1));
                    let fields = editable_fields(track);
                    json!({
                        "id": id,
                        "clip_id": "composition",
                        "local_id": id,
                        "label": track_label(track, &fields),
                        "kind": value_string(track, "kind").unwrap_or_else(|| "component".to_string()),
                        "component": value_string(track, "component"),
                        "z": track.get("z").and_then(Value::as_i64).unwrap_or(index as i64),
                        "start_ms": 0,
                        "duration_ms": composition_duration_ms(composition),
                        "end_ms": composition_duration_ms(composition),
                        "fields": fields
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn editable_fields(track: &Value) -> Vec<Value> {
    let mut fields = Vec::new();
    collect_fields(track.get("params"), "", &mut fields);
    if let Some(items) = track.get("items").and_then(Value::as_array) {
        if let Some(first) = items.first() {
            collect_fields(first.get("params"), "", &mut fields);
        }
    }
    fields.sort_by(|left, right| {
        left.get("field")
            .and_then(Value::as_str)
            .cmp(&right.get("field").and_then(Value::as_str))
    });
    fields.dedup_by(|left, right| left.get("field") == right.get("field"));
    fields
}

fn collect_fields(value: Option<&Value>, prefix: &str, fields: &mut Vec<Value>) {
    let Some(object) = value.and_then(Value::as_object) else {
        return;
    };
    for (key, value) in object {
        let field = if prefix.is_empty() {
            key.to_string()
        } else {
            format!("{prefix}.{key}")
        };
        if value.is_object() {
            collect_fields(Some(value), &field, fields);
        } else if value.is_string() || value.is_number() || value.is_boolean() {
            fields.push(json!({
                "field": field,
                "value": value,
                "kind": if value.is_number() { "number" } else if value.is_boolean() { "boolean" } else { "text" }
            }));
        }
    }
}

fn track_label(track: &Value, fields: &[Value]) -> String {
    for key in ["title", "text", "label", "eyebrow", "subtitle"] {
        if let Some(value) = field_value(fields, key).and_then(Value::as_str) {
            return value.to_string();
        }
    }
    value_string(track, "id").unwrap_or_else(|| "track".to_string())
}

fn field_value<'a>(fields: &'a [Value], name: &str) -> Option<&'a Value> {
    fields
        .iter()
        .find(|field| field.get("field").and_then(Value::as_str) == Some(name))
        .and_then(|field| field.get("value"))
}

fn set_track_field(track: &mut Value, field: &str, value: Value) {
    if let Some(items) = track.get_mut("items").and_then(Value::as_array_mut) {
        if let Some(first) = items.first_mut() {
            set_nested_param(first, field, value);
            return;
        }
    }
    set_nested_param(track, field, value);
}

fn set_nested_param(target: &mut Value, field: &str, value: Value) {
    let object = ensure_object(target);
    let params = object
        .entry("params".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    set_nested_value(params, field, value);
}

fn set_nested_value(root: &mut Value, field: &str, value: Value) {
    let mut current = root;
    let parts: Vec<&str> = field.split('.').filter(|part| !part.is_empty()).collect();
    for part in parts.iter().take(parts.len().saturating_sub(1)) {
        let object = ensure_object(current);
        current = object
            .entry((*part).to_string())
            .or_insert_with(|| Value::Object(Map::new()));
    }
    if let Some(last) = parts.last() {
        let object = ensure_object(current);
        object.insert((*last).to_string(), value);
    }
}

fn ensure_object(value: &mut Value) -> &mut Map<String, Value> {
    loop {
        match value {
            Value::Object(object) => return object,
            _ => *value = Value::Object(Map::new()),
        }
    }
}

fn composition_duration_ms(composition: &Value) -> u64 {
    composition
        .get("duration_ms")
        .and_then(Value::as_u64)
        .or_else(|| time_value_ms(composition.get("duration")))
        .or_else(|| {
            composition
                .get("clips")
                .and_then(Value::as_array)
                .map(|clips| clips.iter().filter_map(clip_duration_ms).sum())
        })
        .unwrap_or(1000)
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

fn value_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|item| !item.trim().is_empty())
        .map(ToString::to_string)
}
