use std::path::Path;

#[derive(Debug, Clone, serde::Serialize)]
pub struct RenderSourceSummary {
    pub schema_version: String,
    pub duration_ms: u64,
    pub viewport: (u32, u32),
    pub tracks: usize,
    pub clips: usize,
    pub components: usize,
    pub visual_tracks: usize,
    pub audio_tracks: usize,
    pub background: String,
    pub warnings: Vec<String>,
}

pub fn validate_render_source_file(source_path: &Path) -> Result<RenderSourceSummary, String> {
    let source_text = std::fs::read_to_string(source_path)
        .map_err(|err| format!("read source {}: {err}", source_path.display()))?;
    let source_json: serde_json::Value =
        serde_json::from_str(&source_text).map_err(|err| format!("source JSON: {err}"))?;
    validate_render_source(&source_json)
}

pub fn validate_render_source(
    source_json: &serde_json::Value,
) -> Result<RenderSourceSummary, String> {
    let root = source_json
        .as_object()
        .ok_or_else(|| "render source must be a JSON object".to_string())?;
    let schema_version = root
        .get("schema_version")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "schema_version is required".to_string())?;
    if schema_version != super::RENDER_SOURCE_SCHEMA_VERSION {
        return Err(format!(
            "schema_version must be {} (got {schema_version})",
            super::RENDER_SOURCE_SCHEMA_VERSION
        ));
    }
    let duration_ms = root
        .get("duration_ms")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "duration_ms is required".to_string())?;
    if duration_ms == 0 {
        return Err("duration_ms must be > 0".to_string());
    }
    let viewport = root
        .get("viewport")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| "viewport object is required".to_string())?;
    let vp_w = viewport
        .get("w")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "viewport.w is required".to_string())?;
    let vp_h = viewport
        .get("h")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "viewport.h is required".to_string())?;
    if vp_w == 0 || vp_h == 0 || vp_w > u64::from(u32::MAX) || vp_h > u64::from(u32::MAX) {
        return Err(format!(
            "viewport must be a positive u32 size (got {vp_w}x{vp_h})"
        ));
    }
    let tracks = root
        .get("tracks")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "tracks array is required".to_string())?;
    if tracks.is_empty() {
        return Err("tracks must not be empty".to_string());
    }
    let components = root
        .get("components")
        .and_then(serde_json::Value::as_object)
        .map(serde_json::Map::len)
        .unwrap_or(0);
    let background = super::html::resolve_stage_background(source_json);
    let mut warnings = Vec::new();
    let mut clips = 0_usize;
    let mut visual_tracks = 0_usize;
    let mut audio_tracks = 0_usize;

    for track in tracks {
        let track_id = track
            .get("id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("<missing>");
        let kind = track
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| format!("track '{track_id}' kind is required"))?;
        if kind == "audio" {
            audio_tracks += 1;
        } else {
            visual_tracks += 1;
        }
        let track_clips = track
            .get("clips")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| format!("track '{track_id}' clips array is required"))?;
        if track_clips.is_empty() {
            warnings.push(format!("track '{track_id}' has no clips"));
        }
        for clip in track_clips {
            clips += 1;
            let clip_id = clip
                .get("id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("<missing>");
            let begin = clip
                .get("begin_ms")
                .or_else(|| clip.get("begin"))
                .and_then(serde_json::Value::as_u64)
                .ok_or_else(|| format!("clip '{track_id}.{clip_id}' begin_ms is required"))?;
            let end = clip
                .get("end_ms")
                .or_else(|| clip.get("end"))
                .and_then(serde_json::Value::as_u64)
                .ok_or_else(|| format!("clip '{track_id}.{clip_id}' end_ms is required"))?;
            if end <= begin {
                return Err(format!(
                    "clip '{track_id}.{clip_id}' end_ms must be greater than begin_ms"
                ));
            }
            if end > duration_ms {
                warnings.push(format!(
                    "clip '{track_id}.{clip_id}' ends after source duration"
                ));
            }
        }
    }

    if visual_tracks == 0 {
        return Err("at least one non-audio visual track is required".to_string());
    }

    Ok(RenderSourceSummary {
        schema_version: schema_version.to_string(),
        duration_ms,
        viewport: (vp_w as u32, vp_h as u32),
        tracks: tracks.len(),
        clips,
        components,
        visual_tracks,
        audio_tracks,
        background,
        warnings,
    })
}
