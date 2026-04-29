fn compile_clip_composition_source(
    storage: &JsonStorage,
    project_slug: &str,
    object: &serde_json::Map<String, Value>,
    id: &str,
    name: &str,
) -> Result<SourceCompileResult, ProjectError> {
    let clips = object
        .get("clips")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            ProjectError::ValidationFailed("composition.clips must be an array".to_string())
        })?;
    if clips.is_empty() {
        return Err(ProjectError::ValidationFailed(
            "composition.clips must not be empty".to_string(),
        ));
    }

    let windows = clip_windows(object)?;
    let duration_ms = object
        .get("duration")
        .map(|value| time_value_ms(Some(value), &BTreeMap::new(), "duration"))
        .transpose()?
        .unwrap_or_else(|| windows.iter().map(|clip| clip.end_ms).max().unwrap_or(0));
    if duration_ms == 0 {
        return Err(ProjectError::ValidationFailed(
            "composition duration must be greater than zero".to_string(),
        ));
    }

    let viewport = composition_viewport(object.get("viewport"));
    let theme_id = object
        .get("theme")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or("default");
    let theme_css = load_theme_css(storage.root(), project_slug, theme_id)?;
    let mut warnings = Vec::new();
    let mut source_tracks = Vec::new();
    let mut components = serde_json::Map::new();
    let mut subtitle_timelines = BTreeMap::<String, Vec<Value>>::new();
    let mut has_visual = false;

    for (clip_index, clip_value) in clips.iter().enumerate() {
        let Some(clip) = clip_value.as_object() else {
            warnings.push(format!("ignored non-object clip at index {clip_index}"));
            continue;
        };
        let clip_window = &windows[clip_index];
        collect_clip_subtitle_timelines(
            storage,
            project_slug,
            clip,
            clip_window,
            &mut subtitle_timelines,
            &mut warnings,
        )?;
    }

    for (clip_index, clip_value) in clips.iter().enumerate() {
        let Some(clip) = clip_value.as_object() else {
            continue;
        };
        let clip_window = &windows[clip_index];
        let tracks = clip
            .get("tracks")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                ProjectError::ValidationFailed(format!(
                    "clip '{}' tracks must be an array",
                    clip_window.id
                ))
            })?;
        for (track_index, track_value) in tracks.iter().enumerate() {
            let Some(track) = track_value.as_object() else {
                warnings.push(format!(
                    "ignored non-object track at clip '{}' index {track_index}",
                    clip_window.id
                ));
                continue;
            };
            let kind = track
                .get("kind")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .unwrap_or("component");
            if kind == "subtitle_timeline" {
                continue;
            }
            let track_id = value_id(track, "track", track_index + 1);
            let items = normalized_track_items(track);
            for (item_index, item) in items.iter().enumerate() {
                let item_id = value_id(item, "item", item_index + 1);
                let source_track_id = format!("{}.{}", clip_window.id, track_id);
                let runtime_clip_id = format!("{}.{}.{}", clip_window.id, track_id, item_id);
                let (local_start, local_end) = item_time_ms(
                    item,
                    track,
                    &clip_window.anchors,
                    clip_window.end_ms - clip_window.start_ms,
                    &runtime_clip_id,
                )?;
                if local_end <= local_start {
                    return Err(ProjectError::ValidationFailed(format!(
                        "item '{runtime_clip_id}' end must be greater than start"
                    )));
                }
                let begin = clip_window.start_ms + local_start;
                let end = clip_window.start_ms + local_end;
                let mut params = serde_json::Map::new();
                let runtime_kind = match kind {
                    "component" => {
                        let component_id = item
                            .get("component")
                            .or_else(|| track.get("component"))
                            .and_then(Value::as_str)
                            .filter(|value| !value.is_empty())
                            .ok_or_else(|| {
                                ProjectError::ValidationFailed(format!(
                                    "component track '{source_track_id}' missing component"
                                ))
                            })?;
                        if !components.contains_key(component_id) {
                            let src =
                                load_component_js(storage.root(), project_slug, component_id)?;
                            components.insert(component_id.to_string(), Value::String(src));
                        }
                        params.insert("component".to_string(), json!(component_id));
                        params.insert(
                            "params".to_string(),
                            merge_json_objects(track.get("params"), item.get("params")),
                        );
                        params.insert(
                            "style".to_string(),
                            merge_json_objects(track.get("style"), item.get("style")),
                        );
                        has_visual = true;
                        "component"
                    }
                    "audio" | "tts" => {
                        let src = item
                            .get("src")
                            .or_else(|| item.get("audio"))
                            .or_else(|| track.get("src"))
                            .or_else(|| track.get("audio"))
                            .and_then(Value::as_str);
                        let Some(src) = src else {
                            warnings.push(format!(
                                "ignored audio item '{runtime_clip_id}' without src"
                            ));
                            continue;
                        };
                        params.insert(
                            "src".to_string(),
                            json!(composition_audio_src(storage.root(), project_slug, src)),
                        );
                        for key in ["from_ms", "to_ms", "volume"] {
                            if let Some(value) = item
                                .get(key)
                                .or_else(|| track.get(key))
                                .and_then(Value::as_f64)
                            {
                                params.insert(key.to_string(), json!(value));
                            }
                        }
                        if let Some(tts) = item
                            .get("tts")
                            .or_else(|| track.get("tts"))
                            .filter(|value| value.is_object())
                        {
                            params.insert("tts".to_string(), tts.clone());
                        }
                        "audio"
                    }
                    "subtitle" => {
                        let source_ref = item
                            .get("source")
                            .or_else(|| item.get("timeline"))
                            .or_else(|| track.get("source"))
                            .or_else(|| track.get("timeline"))
                            .and_then(Value::as_str)
                            .unwrap_or(&track_id);
                        let key = source_ref_key(&clip_window.id, source_ref);
                        let Some(words) = subtitle_timelines.get(&key).cloned().or_else(|| {
                            item.get("words")
                                .or_else(|| {
                                    item.get("params").and_then(|params| params.get("words"))
                                })
                                .and_then(subtitle_words)
                        }) else {
                            warnings.push(format!(
                                "ignored subtitle item '{runtime_clip_id}' without timeline words"
                            ));
                            continue;
                        };
                        params.insert("source".to_string(), json!({ "words": words }));
                        params.insert(
                            "style".to_string(),
                            merge_json_objects(track.get("style"), item.get("style")),
                        );
                        has_visual = true;
                        "subtitle"
                    }
                    other => {
                        params = merge_json_objects(track.get("params"), item.get("params"))
                            .as_object()
                            .cloned()
                            .unwrap_or_default();
                        if other != "audio" {
                            has_visual = true;
                        }
                        other
                    }
                };
                params.insert(
                    "track".to_string(),
                    json!({
                        "id": source_track_id,
                        "clip": clip_window.id,
                        "kind": kind
                    }),
                );
                source_tracks.push(json!({
                    "id": source_track_id,
                    "kind": runtime_kind,
                    "z": item.get("z").or_else(|| track.get("z")).cloned().unwrap_or_else(|| json!(track_index)),
                    "clips": [{
                        "id": runtime_clip_id,
                        "begin": begin,
                        "end": end,
                        "params": Value::Object(params)
                    }]
                }));
            }
        }
    }

    if !has_visual {
        return Err(ProjectError::ValidationFailed(
            "composition has no visual tracks".to_string(),
        ));
    }

    let source = json!({
        "meta": {
            "name": name,
            "project": project_slug,
            "composition": id,
            "version": "v3",
            "authoring": "clip-first",
            "export": object.get("export").cloned().unwrap_or_else(|| json!({ "resolution": "1080p" }))
        },
        "viewport": viewport,
        "duration": duration_ms,
        "anchors": {},
        "theme": {
            "id": theme_id,
            "css": theme_css
        },
        "components": Value::Object(components),
        "tracks": source_tracks
    });
    let source =
        finalize_render_source_v1(source, duration_ms, &theme_background_from_css(&theme_css));

    Ok(SourceCompileResult { source, warnings })
}

fn validate_clip_composition_components(
    storage: &JsonStorage,
    project_slug: &str,
    composition_id: &str,
    object: &serde_json::Map<String, Value>,
) -> Result<ComponentValidationReport, ProjectError> {
    let clips = object
        .get("clips")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            ProjectError::ValidationFailed("composition.clips must be an array".to_string())
        })?;
    let windows = clip_windows(object)?;
    let mut components: BTreeMap<String, ComponentValidationComponent> = BTreeMap::new();
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    for (clip_index, clip_value) in clips.iter().enumerate() {
        let Some(clip) = clip_value.as_object() else {
            warnings.push(format!("ignored non-object clip at index {clip_index}"));
            continue;
        };
        let clip_window = &windows[clip_index];
        let Some(tracks) = clip.get("tracks").and_then(Value::as_array) else {
            errors.push(format!("clip '{}' tracks must be an array", clip_window.id));
            continue;
        };
        for (track_index, track_value) in tracks.iter().enumerate() {
            let Some(track) = track_value.as_object() else {
                warnings.push(format!(
                    "ignored non-object track at clip '{}' index {track_index}",
                    clip_window.id
                ));
                continue;
            };
            let kind = track
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or("component");
            if kind != "component" {
                continue;
            }
            let track_id = value_id(track, "track", track_index + 1);
            let component_id = match track
                .get("component")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
            {
                Some(value) => value,
                None => {
                    errors.push(format!(
                        "component track '{}.{}' missing component",
                        clip_window.id, track_id
                    ));
                    continue;
                }
            };
            if let Err(err) = validate_component_id(component_id) {
                errors.push(format!("track '{}.{}' {err}", clip_window.id, track_id));
                continue;
            }
            let entry = components
                .entry(component_id.to_string())
                .or_insert_with(|| {
                    inspect_component_source(
                        storage.root(),
                        project_slug,
                        component_id,
                        &mut errors,
                    )
                });
            for (item_index, item) in normalized_track_items(track).iter().enumerate() {
                let item_id = value_id(item, "item", item_index + 1);
                let runtime_id = format!("{}.{}.{}", clip_window.id, track_id, item_id);
                let (local_start, local_end) = match item_time_ms(
                    item,
                    track,
                    &clip_window.anchors,
                    clip_window.end_ms - clip_window.start_ms,
                    &runtime_id,
                ) {
                    Ok(value) if value.1 > value.0 => value,
                    Ok(_) => {
                        errors.push(format!(
                            "item '{runtime_id}' end must be greater than start"
                        ));
                        (0, 0)
                    }
                    Err(err) => {
                        errors.push(err.to_string());
                        (0, 0)
                    }
                };
                entry.used_by.push(ComponentUsage {
                    track: runtime_id,
                    start_ms: clip_window.start_ms + local_start,
                    end_ms: clip_window.start_ms + local_end,
                });
                for source in [
                    track.get("params"),
                    item.get("params"),
                    track.get("style"),
                    item.get("style"),
                ] {
                    if let Some(params) = source.and_then(Value::as_object) {
                        for key in params.keys() {
                            if !entry.params.contains(key) {
                                entry.params.push(key.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    let mut component_list: Vec<_> = components.into_values().collect();
    for component in &mut component_list {
        component.params.sort();
        component
            .used_by
            .sort_by(|left, right| left.track.cmp(&right.track));
        if component.exists && !component.exports.mount {
            errors.push(format!(
                "component '{}' missing export function mount",
                component.id
            ));
        }
        if component.exists && !component.exports.update {
            errors.push(format!(
                "component '{}' missing export function update",
                component.id
            ));
        }
        if component.exports.imports || component.exports.dynamic_imports {
            errors.push(format!(
                "component '{}' must be single-file and cannot use import",
                component.id
            ));
        }
    }

    Ok(ComponentValidationReport {
        ok: errors.is_empty(),
        project: project_slug.to_string(),
        composition: composition_id.to_string(),
        available_components: list_project_components(storage.root(), project_slug)?,
        components: component_list,
        warnings,
        errors,
    })
}

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

