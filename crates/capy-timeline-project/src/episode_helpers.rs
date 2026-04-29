fn scene_params(
    object: &serde_json::Map<String, Value>,
    title: &str,
    subtitle: &str,
    layout: &str,
    accent: &str,
    bg_color: &str,
    position: (f64, f64),
) -> serde_json::Map<String, Value> {
    let mut params = serde_json::Map::new();
    params.insert("layout".to_string(), json!(layout));
    params.insert("title".to_string(), json!(title));
    params.insert("subtitle".to_string(), json!(subtitle));
    params.insert("accent_color".to_string(), json!(accent));
    params.insert("bg_color".to_string(), json!(bg_color));
    params.insert("title_x".to_string(), json!(position.0));
    params.insert("title_y".to_string(), json!(position.1));
    copy_string_param(object, &mut params, "eyebrow");
    copy_string_param(object, &mut params, "description");
    copy_string_param(object, &mut params, "big_number");
    copy_string_param(object, &mut params, "label");
    copy_string_param(object, &mut params, "sublabel");
    params
}

fn text_params(
    object: &serde_json::Map<String, Value>,
    label: &str,
    accent: &str,
    position: (f64, f64),
) -> serde_json::Map<String, Value> {
    let mut params = serde_json::Map::new();
    let text = object
        .get("text")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or(label);
    params.insert("text".to_string(), json!(text));
    params.insert("x".to_string(), json!(position.0));
    params.insert("y".to_string(), json!(position.1));
    params.insert("accent_color".to_string(), json!(accent));
    copy_string_param(object, &mut params, "style");
    copy_string_param(object, &mut params, "color");
    copy_string_param(object, &mut params, "align");
    if let Some(size) = object.get("size_px").and_then(Value::as_f64) {
        params.insert("size_px".to_string(), json!(size));
    }
    params
}

fn overlay_params(
    object: &serde_json::Map<String, Value>,
    label: &str,
    accent: &str,
    position: (f64, f64),
) -> serde_json::Map<String, Value> {
    let mut params = serde_json::Map::new();
    let variant = object
        .get("variant")
        .and_then(Value::as_str)
        .filter(|value| matches!(*value, "badge" | "progress"))
        .unwrap_or("badge");
    let text = object
        .get("text")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or(label);
    params.insert("variant".to_string(), json!(variant));
    params.insert("text".to_string(), json!(text));
    params.insert("x".to_string(), json!(position.0));
    params.insert("y".to_string(), json!(position.1));
    params.insert("accent_color".to_string(), json!(accent));
    if let Some(progress) = object.get("progress").and_then(Value::as_f64) {
        params.insert("progress".to_string(), json!(progress.clamp(0.0, 1.0)));
    }
    params
}

fn subtitle_params(
    object: &serde_json::Map<String, Value>,
    accent: &str,
) -> Option<serde_json::Map<String, Value>> {
    let words = subtitle_words(object.get("words")?)?;
    let mut style = serde_json::Map::new();
    style.insert("active_color".to_string(), json!(accent));
    style.insert("position".to_string(), json!("bottom"));
    style.insert("size_px".to_string(), json!(38));
    style.insert("padding".to_string(), json!(58));
    if let Some(size) = object.get("size_px").and_then(Value::as_f64) {
        style.insert("size_px".to_string(), json!(size));
    }
    copy_string_param(object, &mut style, "active_color");
    copy_string_param(object, &mut style, "position");

    let mut source = serde_json::Map::new();
    source.insert("words".to_string(), Value::Array(words));

    let mut params = serde_json::Map::new();
    params.insert("source".to_string(), Value::Object(source));
    params.insert("style".to_string(), Value::Object(style));
    if let Some(tts) = object.get("tts").filter(|value| value.is_object()) {
        params.insert("tts".to_string(), tts.clone());
    }
    Some(params)
}

fn subtitle_words(value: &Value) -> Option<Vec<Value>> {
    let words = value.as_array()?;
    let normalized: Vec<Value> = words
        .iter()
        .filter_map(|word| {
            let object = word.as_object()?;
            let text = object.get("text").and_then(Value::as_str)?.trim();
            let start_ms = object.get("start_ms").and_then(Value::as_f64)?;
            let end_ms = object.get("end_ms").and_then(Value::as_f64)?;
            if text.is_empty() || end_ms < start_ms {
                return None;
            }
            Some(json!({
                "text": text,
                "start_ms": start_ms,
                "end_ms": end_ms
            }))
        })
        .collect();
    (!normalized.is_empty()).then_some(normalized)
}

fn audio_params(object: &serde_json::Map<String, Value>) -> Option<serde_json::Map<String, Value>> {
    let src = object
        .get("src")
        .and_then(Value::as_str)
        .filter(|value| value.starts_with("file://") || value.starts_with("data:"))?;
    let mut params = serde_json::Map::new();
    params.insert("src".to_string(), json!(src));
    for key in ["from_ms", "to_ms", "volume"] {
        if let Some(value) = object.get(key).and_then(Value::as_f64) {
            params.insert(key.to_string(), json!(value));
        }
    }
    if let Some(tts) = object.get("tts").filter(|value| value.is_object()) {
        params.insert("tts".to_string(), tts.clone());
    }
    Some(params)
}

fn composition_audio_src(root: &Path, project_slug: &str, src: &str) -> String {
    if src.starts_with("file://") || src.starts_with("data:") {
        return src.to_string();
    }
    let path = if Path::new(src).is_absolute() {
        PathBuf::from(src)
    } else {
        root.join(project_slug).join(src)
    };
    file_url(&path)
}

fn file_url(path: &Path) -> String {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };
    let mut encoded = String::new();
    for byte in absolute.to_string_lossy().as_bytes() {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                encoded.push(char::from(*byte))
            }
            other => encoded.push_str(&format!("%{other:02X}")),
        }
    }
    format!("file://{encoded}")
}

fn copy_string_param(
    source: &serde_json::Map<String, Value>,
    target: &mut serde_json::Map<String, Value>,
    key: &str,
) {
    if let Some(value) = source
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
    {
        target.insert(key.to_string(), json!(value));
    }
}

fn is_hex_color(value: &str) -> bool {
    value.len() == 7
        && value.starts_with('#')
        && value.chars().skip(1).all(|ch| ch.is_ascii_hexdigit())
}

fn clip_position(object: &serde_json::Map<String, Value>) -> (f64, f64) {
    let position = object.get("position").and_then(Value::as_object);
    let x = position
        .and_then(|value| value.get("x"))
        .and_then(Value::as_f64)
        .unwrap_or(50.0)
        .clamp(5.0, 95.0);
    let y = position
        .and_then(|value| value.get("y"))
        .and_then(Value::as_f64)
        .unwrap_or(50.0)
        .clamp(5.0, 95.0);
    (x, y)
}

fn resolve_time_ms(
    value: Option<&Value>,
    anchors: &BTreeMap<String, f64>,
    field: &str,
) -> Result<u64, ProjectError> {
    match value {
        Some(Value::Number(number)) => {
            let seconds = number.as_f64().ok_or_else(|| {
                ProjectError::ValidationFailed(format!("{field} must be a finite number"))
            })?;
            seconds_to_ms(seconds, field)
        }
        Some(Value::String(raw)) => resolve_time_expr_ms(raw, anchors, field),
        _ => Err(ProjectError::ValidationFailed(format!(
            "{field} must be a number, anchor, or simple anchor +/- seconds expression"
        ))),
    }
}

fn resolve_time_expr_ms(
    raw: &str,
    anchors: &BTreeMap<String, f64>,
    field: &str,
) -> Result<u64, ProjectError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ProjectError::ValidationFailed(format!("{field} is empty")));
    }
    if let Ok(seconds) = trimmed.parse::<f64>() {
        return seconds_to_ms(seconds, field);
    }
    if let Some(seconds) = anchors.get(trimmed).copied() {
        return seconds_to_ms(seconds, field);
    }

    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.len() == 3 && (parts[1] == "+" || parts[1] == "-") {
        let base = anchors.get(parts[0]).copied().ok_or_else(|| {
            ProjectError::ValidationFailed(format!("unknown anchor '{}' in {field}", parts[0]))
        })?;
        let delta = parts[2].parse::<f64>().map_err(|err| {
            ProjectError::ValidationFailed(format!("invalid seconds offset in {field}: {err}"))
        })?;
        let seconds = if parts[1] == "+" {
            base + delta
        } else {
            base - delta
        };
        return seconds_to_ms(seconds, field);
    }

    Err(ProjectError::ValidationFailed(format!(
        "unsupported time expression for {field}: '{trimmed}'"
    )))
}

fn seconds_to_ms(seconds: f64, field: &str) -> Result<u64, ProjectError> {
    if !seconds.is_finite() || seconds < 0.0 {
        return Err(ProjectError::ValidationFailed(format!(
            "{field} must be a non-negative finite number"
        )));
    }
    let ms = (seconds * 1000.0).round();
    if ms > u64::MAX as f64 {
        return Err(ProjectError::ValidationFailed(format!(
            "{field} is too large"
        )));
    }
    Ok(ms as u64)
}

fn composition_viewport(value: Option<&Value>) -> Value {
    let object = value.and_then(Value::as_object);
    let w = object
        .and_then(|item| item.get("w"))
        .and_then(Value::as_u64)
        .filter(|value| *value > 0)
        .unwrap_or(1920);
    let h = object
        .and_then(|item| item.get("h"))
        .and_then(Value::as_u64)
        .filter(|value| *value > 0)
        .unwrap_or(1080);
    let ratio = object
        .and_then(|item| item.get("ratio"))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or("16:9");
    json!({ "w": w, "h": h, "ratio": ratio })
}

fn resolve_composition_anchors(
    value: Option<&Value>,
    duration_ms: u64,
) -> Result<BTreeMap<String, f64>, ProjectError> {
    let mut anchors = BTreeMap::new();
    anchors.insert("start".to_string(), 0.0);
    anchors.insert("end".to_string(), duration_ms as f64 / 1000.0);
    let Some(object) = value.and_then(Value::as_object) else {
        return Ok(anchors);
    };

    for _ in 0..object.len().max(1) {
        let mut changed = false;
        for (name, raw) in object {
            if anchors.contains_key(name) {
                continue;
            }
            match time_value_ms(Some(raw), &anchors, &format!("anchor '{name}'")) {
                Ok(ms) => {
                    anchors.insert(name.clone(), ms as f64 / 1000.0);
                    changed = true;
                }
                Err(ProjectError::ValidationFailed(_)) => {}
                Err(err) => return Err(err),
            }
        }
        if !changed {
            break;
        }
    }

    let unresolved: Vec<String> = object
        .keys()
        .filter(|name| !anchors.contains_key(*name))
        .cloned()
        .collect();
    if !unresolved.is_empty() {
        return Err(ProjectError::ValidationFailed(format!(
            "unresolved composition anchors: {}",
            unresolved.join(", ")
        )));
    }
    Ok(anchors)
}

fn track_time_ms(
    track: &serde_json::Map<String, Value>,
    anchors: &BTreeMap<String, f64>,
    duration_ms: u64,
    track_id: &str,
) -> Result<(u64, u64), ProjectError> {
    let time = track.get("time").and_then(Value::as_object);
    let start_default = Value::String("start".to_string());
    let end_default = Value::String("end".to_string());
    let start_value = time
        .and_then(|value| value.get("start"))
        .or_else(|| track.get("start"))
        .unwrap_or(&start_default);
    let end_value = time
        .and_then(|value| value.get("end"))
        .or_else(|| track.get("end"))
        .unwrap_or(&end_default);
    let start = time_value_ms(
        Some(start_value),
        anchors,
        &format!("track '{track_id}' start"),
    )?;
    let end = time_value_ms(Some(end_value), anchors, &format!("track '{track_id}' end"))?;
    Ok((start.min(duration_ms), end.min(duration_ms)))
}

fn time_value_ms(
    value: Option<&Value>,
    anchors: &BTreeMap<String, f64>,
    field: &str,
) -> Result<u64, ProjectError> {
    match value {
        Some(Value::Number(number)) => {
            let seconds = number.as_f64().ok_or_else(|| {
                ProjectError::ValidationFailed(format!("{field} must be a finite number"))
            })?;
            seconds_to_ms(seconds, field)
        }
        Some(Value::String(raw)) => time_expr_ms(raw, anchors, field),
        _ => Err(ProjectError::ValidationFailed(format!(
            "{field} must be a number or time expression"
        ))),
    }
}

fn time_expr_ms(
    raw: &str,
    anchors: &BTreeMap<String, f64>,
    field: &str,
) -> Result<u64, ProjectError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ProjectError::ValidationFailed(format!("{field} is empty")));
    }
    let without_unit = trimmed
        .strip_suffix("ms")
        .and_then(|value| value.trim().parse::<f64>().ok())
        .map(|value| value / 1000.0)
        .or_else(|| {
            trimmed
                .strip_suffix('s')
                .and_then(|value| value.trim().parse::<f64>().ok())
        });
    if let Some(seconds) = without_unit {
        return seconds_to_ms(seconds, field);
    }
    if let Ok(seconds) = trimmed.parse::<f64>() {
        return seconds_to_ms(seconds, field);
    }
    if let Some(seconds) = anchors.get(trimmed).copied() {
        return seconds_to_ms(seconds, field);
    }

    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.len() == 3 && (parts[1] == "+" || parts[1] == "-") {
        let base = anchors.get(parts[0]).copied().ok_or_else(|| {
            ProjectError::ValidationFailed(format!("unknown anchor '{}' in {field}", parts[0]))
        })?;
        let delta_ms = time_expr_ms(parts[2], anchors, field)?;
        let delta = delta_ms as f64 / 1000.0;
        let seconds = if parts[1] == "+" {
            base + delta
        } else {
            base - delta
        };
        return seconds_to_ms(seconds, field);
    }

    Err(ProjectError::ValidationFailed(format!(
        "unsupported time expression for {field}: '{trimmed}'"
    )))
}

