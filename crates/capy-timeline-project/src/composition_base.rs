#[derive(Debug, Clone)]
pub struct SourceCompileResult {
    pub source: Value,
    pub warnings: Vec<String>,
}

const RENDER_SOURCE_SCHEMA_VERSION: &str = "capy.timeline.render_source.v1";

fn finalize_render_source_v1(mut source: Value, duration_ms: u64, background: &str) -> Value {
    let Some(root) = source.as_object_mut() else {
        return source;
    };
    root.insert(
        "schema_version".to_string(),
        json!(RENDER_SOURCE_SCHEMA_VERSION),
    );
    root.insert("duration_ms".to_string(), json!(duration_ms));
    root.entry("assets".to_string())
        .or_insert_with(|| json!([]));

    if let Some(meta) = root.get_mut("meta").and_then(Value::as_object_mut) {
        meta.insert("duration_ms".to_string(), json!(duration_ms));
        meta.insert(
            "render_source_schema".to_string(),
            json!(RENDER_SOURCE_SCHEMA_VERSION),
        );
    }

    let theme = root
        .entry("theme".to_string())
        .or_insert_with(|| json!({}))
        .as_object_mut();
    if let Some(theme) = theme {
        theme
            .entry("background".to_string())
            .or_insert_with(|| json!(background));
    }

    if let Some(tracks) = root.get_mut("tracks").and_then(Value::as_array_mut) {
        for track in tracks {
            let Some(clips) = track.get_mut("clips").and_then(Value::as_array_mut) else {
                continue;
            };
            for clip in clips {
                let Some(clip_object) = clip.as_object_mut() else {
                    continue;
                };
                if let Some(begin) = clip_object.get("begin").cloned() {
                    clip_object.entry("begin_ms".to_string()).or_insert(begin);
                }
                if let Some(end) = clip_object.get("end").cloned() {
                    clip_object.entry("end_ms".to_string()).or_insert(end);
                }
            }
        }
    }

    source
}

fn theme_background_from_css(css: &str) -> String {
    extract_css_custom_property(css, "--capy-timeline-bg")
        .and_then(sanitize_stage_background)
        .unwrap_or_else(|| "#000".to_string())
}

fn extract_css_custom_property<'a>(css: &'a str, name: &str) -> Option<&'a str> {
    let start = css.find(name)?;
    let rest = &css[start + name.len()..];
    let colon = rest.find(':')?;
    let rest = &rest[colon + 1..];
    let end = rest.find(';')?;
    Some(rest[..end].trim())
}

fn sanitize_stage_background(raw: &str) -> Option<String> {
    let value = raw.trim();
    if value.is_empty() || value.len() > 96 || value.contains([';', '{', '}']) {
        return None;
    }
    let lower = value.to_ascii_lowercase();
    let named = matches!(
        lower.as_str(),
        "black" | "white" | "transparent" | "canvas" | "currentcolor"
    );
    let functional = lower.starts_with("rgb(")
        || lower.starts_with("rgba(")
        || lower.starts_with("hsl(")
        || lower.starts_with("hsla(");
    let hex = lower.starts_with('#')
        && matches!(lower.len(), 4 | 5 | 7 | 9)
        && lower[1..].chars().all(|ch| ch.is_ascii_hexdigit());
    (named || functional || hex).then(|| value.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ComponentValidationReport {
    pub ok: bool,
    pub project: String,
    pub composition: String,
    pub available_components: Vec<String>,
    pub components: Vec<ComponentValidationComponent>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ComponentValidationComponent {
    pub id: String,
    pub path: String,
    pub exists: bool,
    pub bytes: usize,
    pub exports: ComponentExports,
    pub params: Vec<String>,
    pub used_by: Vec<ComponentUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ComponentExports {
    pub mount: bool,
    pub update: bool,
    pub destroy: bool,
    pub imports: bool,
    pub dynamic_imports: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ComponentUsage {
    pub track: String,
    pub start_ms: u64,
    pub end_ms: u64,
}

#[derive(Debug, Clone)]
struct ClipWindow {
    id: String,
    start_ms: u64,
    end_ms: u64,
    anchors: BTreeMap<String, f64>,
}

pub fn validate_composition_components(
    storage: &JsonStorage,
    project_slug: &str,
    composition: &Value,
) -> Result<ComponentValidationReport, ProjectError> {
    let object = composition.as_object().ok_or_else(|| {
        ProjectError::ValidationFailed("composition must be a JSON object".to_string())
    })?;
    let composition_id = object
        .get("id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ProjectError::ValidationFailed("composition.id is required".to_string()))?;
    if object.get("clips").and_then(Value::as_array).is_some() {
        return validate_clip_composition_components(storage, project_slug, composition_id, object);
    }
    let duration_ms = time_value_ms(object.get("duration"), &BTreeMap::new(), "duration")?;
    let anchors = resolve_composition_anchors(object.get("anchors"), duration_ms)?;
    let tracks = object
        .get("tracks")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            ProjectError::ValidationFailed("composition.tracks must be an array".to_string())
        })?;

    let mut components: BTreeMap<String, ComponentValidationComponent> = BTreeMap::new();
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    for (index, track_value) in tracks.iter().enumerate() {
        let Some(track) = track_value.as_object() else {
            warnings.push(format!("ignored non-object track at index {index}"));
            continue;
        };
        let track_id = track
            .get("id")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| format!("track-{}", index + 1));
        let kind = track
            .get("kind")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .unwrap_or("component");
        if kind != "component" {
            continue;
        }

        let component_id = match track
            .get("component")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
        {
            Some(value) => value,
            None => {
                errors.push(format!("component track '{track_id}' missing component"));
                continue;
            }
        };

        if let Err(err) = validate_component_id(component_id) {
            errors.push(format!("track '{track_id}' {err}"));
            continue;
        }

        let (start_ms, end_ms) = match track_time_ms(track, &anchors, duration_ms, &track_id) {
            Ok((start, end)) if end > start => (start, end),
            Ok((_start, _end)) => {
                errors.push(format!("track '{track_id}' end must be greater than start"));
                (0, 0)
            }
            Err(err) => {
                errors.push(err.to_string());
                (0, 0)
            }
        };

        let entry = components
            .entry(component_id.to_string())
            .or_insert_with(|| {
                inspect_component_source(storage.root(), project_slug, component_id, &mut errors)
            });
        entry.used_by.push(ComponentUsage {
            track: track_id,
            start_ms,
            end_ms,
        });
        if let Some(params) = track.get("params").and_then(Value::as_object) {
            for key in params.keys() {
                if !entry.params.contains(key) {
                    entry.params.push(key.to_string());
                }
            }
        }
        if let Some(style) = track.get("style").and_then(Value::as_object) {
            for key in style.keys().filter(|key| *key == "x" || *key == "y") {
                if !entry.params.contains(key) {
                    entry.params.push(key.to_string());
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

pub fn compile_composition_source(
    storage: &JsonStorage,
    project_slug: &str,
    composition: &Value,
) -> Result<SourceCompileResult, ProjectError> {
    let object = composition.as_object().ok_or_else(|| {
        ProjectError::ValidationFailed("composition must be a JSON object".to_string())
    })?;
    let id = object
        .get("id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ProjectError::ValidationFailed("composition.id is required".to_string()))?;
    validate_slug(id)?;
    let name = object
        .get("name")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or(id);
    if object.get("clips").and_then(Value::as_array).is_some() {
        return compile_clip_composition_source(storage, project_slug, object, id, name);
    }
    let duration_ms = time_value_ms(object.get("duration"), &BTreeMap::new(), "duration")?;
    if duration_ms == 0 {
        return Err(ProjectError::ValidationFailed(
            "composition.duration must be greater than zero".to_string(),
        ));
    }
    let anchors = resolve_composition_anchors(object.get("anchors"), duration_ms)?;
    let viewport = composition_viewport(object.get("viewport"));
    let theme_id = object
        .get("theme")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or("default");
    let theme_css = load_theme_css(storage.root(), project_slug, theme_id)?;
    let tracks = object
        .get("tracks")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            ProjectError::ValidationFailed("composition.tracks must be an array".to_string())
        })?;

    let mut warnings = Vec::new();
    let mut source_tracks = Vec::new();
    let mut components = serde_json::Map::new();
    let mut has_visual = false;

    for (index, track_value) in tracks.iter().enumerate() {
        let Some(track) = track_value.as_object() else {
            warnings.push(format!("ignored non-object track at index {index}"));
            continue;
        };
        let track_id = track
            .get("id")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| format!("track-{}", index + 1));
        let kind = track
            .get("kind")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .unwrap_or("component");
        let (begin, end) = track_time_ms(track, &anchors, duration_ms, &track_id)?;
        if end <= begin {
            return Err(ProjectError::ValidationFailed(format!(
                "track '{track_id}' end must be greater than start"
            )));
        }

        let clip_id = track
            .get("clip_id")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .unwrap_or(&track_id);
        let mut params = serde_json::Map::new();
        match kind {
            "component" => {
                let component_id = track
                    .get("component")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| {
                        ProjectError::ValidationFailed(format!(
                            "component track '{track_id}' missing component"
                        ))
                    })?;
                if !components.contains_key(component_id) {
                    let src = load_component_js(storage.root(), project_slug, component_id)?;
                    components.insert(component_id.to_string(), Value::String(src));
                }
                params.insert("component".to_string(), json!(component_id));
                let mut component_params =
                    track.get("params").cloned().unwrap_or_else(|| json!({}));
                if let (Some(target), Some(style)) = (
                    component_params.as_object_mut(),
                    track.get("style").and_then(Value::as_object),
                ) {
                    for key in ["x", "y"] {
                        if let Some(value) = style.get(key) {
                            target.insert(key.to_string(), value.clone());
                        }
                    }
                }
                params.insert("params".to_string(), component_params);
                params.insert(
                    "style".to_string(),
                    track.get("style").cloned().unwrap_or_else(|| json!({})),
                );
                params.insert(
                    "track".to_string(),
                    json!({
                        "id": track_id,
                        "z": track.get("z").and_then(Value::as_i64).unwrap_or(index as i64),
                        "kind": kind
                    }),
                );
                has_visual = true;
            }
            "audio" => {
                let Some(src) = track.get("src").and_then(Value::as_str) else {
                    warnings.push(format!("ignored audio track '{track_id}' without src"));
                    continue;
                };
                params.insert(
                    "src".to_string(),
                    json!(composition_audio_src(storage.root(), project_slug, src)),
                );
                copy_number_param(track, &mut params, "from_ms");
                copy_number_param(track, &mut params, "to_ms");
                copy_number_param(track, &mut params, "volume");
                if let Some(tts) = track.get("tts").filter(|value| value.is_object()) {
                    params.insert("tts".to_string(), tts.clone());
                }
            }
            "subtitle" => {
                let Some(words) = subtitle_words(
                    track
                        .get("words")
                        .or_else(|| track.get("params").and_then(|p| p.get("words")))
                        .unwrap_or(&Value::Null),
                ) else {
                    warnings.push(format!("ignored subtitle track '{track_id}' without words"));
                    continue;
                };
                params.insert("source".to_string(), json!({ "words": words }));
                params.insert(
                    "style".to_string(),
                    track.get("style").cloned().unwrap_or_else(|| json!({})),
                );
                has_visual = true;
            }
            other => {
                params = track
                    .get("params")
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default();
                if other != "audio" {
                    has_visual = true;
                }
            }
        }

        source_tracks.push(json!({
            "id": track_id,
            "kind": kind,
            "z": track.get("z").cloned().unwrap_or_else(|| json!(index)),
            "clips": [{
                "id": clip_id,
                "begin": begin,
                "end": end,
                "params": Value::Object(params)
            }]
        }));
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
            "version": "v2",
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

