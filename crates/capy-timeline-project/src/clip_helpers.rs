fn clip_windows(object: &serde_json::Map<String, Value>) -> Result<Vec<ClipWindow>, ProjectError> {
    let clips = object
        .get("clips")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            ProjectError::ValidationFailed("composition.clips must be an array".to_string())
        })?;
    let mut cursor_ms = 0_u64;
    let mut windows = Vec::new();
    for (index, clip_value) in clips.iter().enumerate() {
        let clip = clip_value.as_object().ok_or_else(|| {
            ProjectError::ValidationFailed(format!("clip at index {index} must be an object"))
        })?;
        let id = value_id(clip, "clip", index + 1);
        let duration_ms = time_value_ms(
            clip.get("duration").or_else(|| clip.get("length")),
            &BTreeMap::new(),
            &format!("clip '{id}' duration"),
        )?;
        if duration_ms == 0 {
            return Err(ProjectError::ValidationFailed(format!(
                "clip '{id}' duration must be greater than zero"
            )));
        }
        let mut anchors = BTreeMap::new();
        anchors.insert("start".to_string(), 0.0);
        anchors.insert("in".to_string(), 0.0);
        anchors.insert("end".to_string(), duration_ms as f64 / 1000.0);
        anchors.insert("out".to_string(), duration_ms as f64 / 1000.0);
        if let Some(raw_anchors) = clip.get("anchors").and_then(Value::as_object) {
            for _ in 0..raw_anchors.len().max(1) {
                let mut changed = false;
                for (name, raw) in raw_anchors {
                    if anchors.contains_key(name) {
                        continue;
                    }
                    if let Ok(ms) =
                        time_value_ms(Some(raw), &anchors, &format!("clip '{id}' anchor '{name}'"))
                    {
                        anchors.insert(name.clone(), ms as f64 / 1000.0);
                        changed = true;
                    }
                }
                if !changed {
                    break;
                }
            }
        }
        windows.push(ClipWindow {
            id,
            start_ms: cursor_ms,
            end_ms: cursor_ms + duration_ms,
            anchors,
        });
        cursor_ms += duration_ms;
    }
    Ok(windows)
}

fn collect_clip_subtitle_timelines(
    storage: &JsonStorage,
    project_slug: &str,
    clip: &serde_json::Map<String, Value>,
    clip_window: &ClipWindow,
    subtitle_timelines: &mut BTreeMap<String, Vec<Value>>,
    warnings: &mut Vec<String>,
) -> Result<(), ProjectError> {
    let Some(tracks) = clip.get("tracks").and_then(Value::as_array) else {
        return Ok(());
    };
    for (track_index, track_value) in tracks.iter().enumerate() {
        let Some(track) = track_value.as_object() else {
            continue;
        };
        let kind = track
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("component");
        if kind != "subtitle_timeline" && kind != "tts" {
            continue;
        }
        let track_id = value_id(track, "track", track_index + 1);
        let mut words = None;
        if let Some(raw_words) = track
            .get("words")
            .or_else(|| track.get("params").and_then(|params| params.get("words")))
        {
            words = subtitle_words(raw_words);
        }
        if words.is_none() {
            if let Some(timeline) = track
                .get("timeline")
                .or_else(|| {
                    track
                        .get("params")
                        .and_then(|params| params.get("timeline"))
                })
                .and_then(Value::as_str)
            {
                words = load_timeline_words(storage.root(), project_slug, timeline)?;
            }
        }
        if let Some(words) = words {
            subtitle_timelines.insert(source_ref_key(&clip_window.id, &track_id), words);
        } else if kind == "subtitle_timeline" {
            warnings.push(format!(
                "subtitle timeline '{}.{}' has no words",
                clip_window.id, track_id
            ));
        }
    }
    Ok(())
}

fn normalized_track_items(
    track: &serde_json::Map<String, Value>,
) -> Vec<serde_json::Map<String, Value>> {
    track
        .get("items")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_object().cloned())
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty())
        .unwrap_or_else(|| vec![track.clone()])
}

fn item_time_ms(
    item: &serde_json::Map<String, Value>,
    track: &serde_json::Map<String, Value>,
    anchors: &BTreeMap<String, f64>,
    duration_ms: u64,
    item_id: &str,
) -> Result<(u64, u64), ProjectError> {
    let item_time = item.get("time").and_then(Value::as_object);
    let track_time = track.get("time").and_then(Value::as_object);
    let start_default = Value::String("start".to_string());
    let end_default = Value::String("end".to_string());
    let start_value = item_time
        .and_then(|value| value.get("start"))
        .or_else(|| item.get("start"))
        .or_else(|| track_time.and_then(|value| value.get("start")))
        .or_else(|| track.get("start"))
        .unwrap_or(&start_default);
    let end_value = item_time
        .and_then(|value| value.get("end"))
        .or_else(|| item.get("end"))
        .or_else(|| track_time.and_then(|value| value.get("end")))
        .or_else(|| track.get("end"))
        .unwrap_or(&end_default);
    let start = time_value_ms(
        Some(start_value),
        anchors,
        &format!("item '{item_id}' start"),
    )?;
    let end = time_value_ms(Some(end_value), anchors, &format!("item '{item_id}' end"))?;
    Ok((start.min(duration_ms), end.min(duration_ms)))
}

fn source_ref_key(clip_id: &str, source: &str) -> String {
    if source.contains('.') {
        source.to_string()
    } else {
        format!("{clip_id}.{source}")
    }
}

fn load_timeline_words(
    root: &Path,
    project_slug: &str,
    timeline: &str,
) -> Result<Option<Vec<Value>>, ProjectError> {
    let path = if Path::new(timeline).is_absolute() {
        PathBuf::from(timeline)
    } else {
        root.join(project_slug).join(timeline)
    };
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path).map_err(|err| {
        ProjectError::StorageFailed(format!("timeline read failed: {}: {err}", path.display()))
    })?;
    let value: Value = serde_json::from_str(&raw).map_err(|err| {
        ProjectError::StorageFailed(format!("timeline parse failed: {}: {err}", path.display()))
    })?;
    Ok(value
        .get("words")
        .or_else(|| {
            value
                .get("timeline")
                .and_then(|timeline| timeline.get("words"))
        })
        .and_then(subtitle_words))
}

fn merge_json_objects(primary: Option<&Value>, secondary: Option<&Value>) -> Value {
    let mut merged = primary
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    if let Some(extra) = secondary.and_then(Value::as_object) {
        for (key, value) in extra {
            merged.insert(key.clone(), value.clone());
        }
    }
    Value::Object(merged)
}

