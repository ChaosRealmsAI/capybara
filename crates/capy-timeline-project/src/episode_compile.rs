fn value_id(
    object: &serde_json::Map<String, Value>,
    fallback_prefix: &str,
    index: usize,
) -> String {
    object
        .get("id")
        .or_else(|| object.get("slug"))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("{fallback_prefix}-{index}"))
}

pub fn compile_episode_source(
    project_slug: &str,
    episode: &Episode,
) -> Result<SourceCompileResult, ProjectError> {
    let mut warnings = Vec::new();
    let duration_ms = seconds_to_ms(episode.duration, "episode.duration")?;
    let mut scene_clips = Vec::new();
    let mut text_clips = Vec::new();
    let mut subtitle_clips = Vec::new();
    let mut overlay_clips = Vec::new();
    let mut audio_clips = Vec::new();
    let mut ignored_tracks = BTreeMap::<String, usize>::new();

    for clip in &episode.clips {
        let Some(object) = clip.as_object() else {
            warnings.push("ignored non-object clip".to_string());
            continue;
        };
        let track = object
            .get("track")
            .and_then(Value::as_str)
            .or_else(|| object.get("kind").and_then(Value::as_str))
            .unwrap_or("scene");
        let normalized_track = match track {
            "scene" => "scene",
            "text" => "text",
            "subtitle" => "subtitle",
            "overlay" => "overlay",
            "audio" => "audio",
            other => {
                *ignored_tracks.entry(other.to_string()).or_insert(0) += 1;
                continue;
            }
        };

        let id = object
            .get("slug")
            .or_else(|| object.get("id"))
            .or_else(|| object.get("clip"))
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| ProjectError::ValidationFailed("scene clip missing slug".to_string()))?;
        let title = object
            .get("label")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .or_else(|| object.get("title").and_then(Value::as_str))
            .unwrap_or(id);
        let subtitle = object
            .get("subtitle")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .unwrap_or(id);
        let layout = object
            .get("layout")
            .and_then(Value::as_str)
            .filter(|value| matches!(*value, "hero" | "stat" | "split" | "quote"))
            .unwrap_or("hero");
        let accent = object
            .get("accent_color")
            .and_then(Value::as_str)
            .filter(|value| is_hex_color(value))
            .unwrap_or("#5eead4");
        let bg_color = object
            .get("bg_color")
            .and_then(Value::as_str)
            .filter(|value| is_hex_color(value))
            .unwrap_or("#07080d");
        let position = clip_position(object);
        let start_ms = resolve_time_ms(object.get("start"), &episode.anchors, "start")?;
        let end_ms = resolve_time_ms(object.get("end"), &episode.anchors, "end")?;
        if end_ms <= start_ms {
            return Err(ProjectError::ValidationFailed(format!(
                "clip '{id}' end must be greater than start"
            )));
        }
        if end_ms > duration_ms {
            warnings.push(format!(
                "clip '{id}' ends after episode duration and will still be exported"
            ));
        }

        let params = match normalized_track {
            "scene" => scene_params(object, title, subtitle, layout, accent, bg_color, position),
            "text" => text_params(object, title, accent, position),
            "subtitle" => match subtitle_params(object, accent) {
                Some(params) => params,
                None => {
                    warnings.push(format!("ignored subtitle clip '{id}' without valid words"));
                    continue;
                }
            },
            "overlay" => overlay_params(object, title, accent, position),
            "audio" => match audio_params(object) {
                Some(params) => params,
                None => {
                    warnings.push(format!(
                        "ignored audio clip '{id}' without file:// or data: src"
                    ));
                    continue;
                }
            },
            _ => {
                *ignored_tracks
                    .entry(normalized_track.to_string())
                    .or_insert(0) += 1;
                continue;
            }
        };

        let compiled_clip = json!({
            "id": id,
            "begin": start_ms,
            "end": end_ms,
            "params": Value::Object(params)
        });
        if normalized_track == "scene" {
            scene_clips.push(compiled_clip);
        } else if normalized_track == "text" {
            text_clips.push(compiled_clip);
        } else if normalized_track == "subtitle" {
            subtitle_clips.push(compiled_clip);
        } else if normalized_track == "audio" {
            audio_clips.push(compiled_clip);
        } else {
            overlay_clips.push(compiled_clip);
        }
    }

    for (track, count) in ignored_tracks {
        warnings.push(format!(
            "ignored {count} clip(s) on unsupported export track '{track}'"
        ));
    }

    if scene_clips.is_empty()
        && text_clips.is_empty()
        && subtitle_clips.is_empty()
        && overlay_clips.is_empty()
    {
        return Err(ProjectError::ValidationFailed(
            "episode has no visual clips to export".to_string(),
        ));
    }

    let mut tracks = Vec::new();
    if !scene_clips.is_empty() {
        tracks.push(json!({
            "id": "scene-main",
            "kind": "scene",
            "clips": scene_clips
        }));
    }
    if !text_clips.is_empty() {
        tracks.push(json!({
            "id": "text-main",
            "kind": "text",
            "clips": text_clips
        }));
    }
    if !subtitle_clips.is_empty() {
        tracks.push(json!({
            "id": "subtitle-main",
            "kind": "subtitle",
            "clips": subtitle_clips
        }));
    }
    if !overlay_clips.is_empty() {
        tracks.push(json!({
            "id": "overlay-main",
            "kind": "overlay",
            "clips": overlay_clips
        }));
    }
    if !audio_clips.is_empty() {
        tracks.push(json!({
            "id": "audio-main",
            "kind": "audio",
            "clips": audio_clips
        }));
    }

    let source = json!({
        "meta": {
            "name": episode.name,
            "project": project_slug,
            "episode": episode.slug,
            "version": "v0.5",
            "export": {
                "resolution": "1080p"
            }
        },
        "viewport": {
            "ratio": "16:9",
            "w": 1920,
            "h": 1080
        },
        "duration": duration_ms,
        "anchors": {},
        "theme": {
            "id": "episode-default",
            "background": "#000"
        },
        "tracks": tracks
    });
    let source = finalize_render_source_v1(source, duration_ms, "#000");

    Ok(SourceCompileResult { source, warnings })
}
