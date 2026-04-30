use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;

use super::report::{ValidationError, ValidationReport};
use super::structural::validate_assets;

pub(super) fn validate_clip_first_structure(
    raw: &Value,
    composition_dir: Option<&Path>,
    report: &mut ValidationReport,
) {
    let Some(object) = raw.as_object() else {
        report.push_error(ValidationError::new(
            "COMPOSITION_INVALID",
            "$",
            "composition must be an object",
            "next step · pass a valid composition.json document",
        ));
        return;
    };
    let schema_version = object
        .get("schema_version")
        .or_else(|| object.get("schema"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("capy.composition.v2");
    report.schema_version = schema_version.to_string();

    if object
        .get("id")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .is_none()
    {
        report.push_error(ValidationError::new(
            "COMPOSITION_INVALID",
            "$.id",
            "composition.id is required",
            "next step · set composition.id to a stable slug",
        ));
    }

    let Some(clips) = object.get("clips").and_then(Value::as_array) else {
        report.push_error(ValidationError::new(
            "EMPTY_CLIPS",
            "$.clips",
            "composition.clips must be an array",
            "next step · use a clip-first composition JSON",
        ));
        return;
    };
    if clips.is_empty() {
        report.push_error(ValidationError::new(
            "EMPTY_CLIPS",
            "$.clips",
            "composition must include at least one clip",
            "next step · add at least one clip with tracks",
        ));
    }

    let mut components = BTreeSet::new();
    let mut track_count = 0usize;
    for (clip_index, clip) in clips.iter().enumerate() {
        let Some(clip_object) = clip.as_object() else {
            report.push_error(ValidationError::new(
                "COMPOSITION_INVALID",
                format!("$.clips[{clip_index}]"),
                "clip must be an object",
                "next step · make every clip a JSON object",
            ));
            continue;
        };
        if clip_object
            .get("duration_ms")
            .or_else(|| clip_object.get("duration"))
            .or_else(|| clip_object.get("length"))
            .is_none()
        {
            report.push_error(ValidationError::new(
                "INVALID_DURATION",
                format!("$.clips[{clip_index}].duration"),
                "clip duration is required",
                "next step · set duration such as 2s or 2000ms",
            ));
        }
        let Some(tracks) = clip_object.get("tracks").and_then(Value::as_array) else {
            report.push_error(ValidationError::new(
                "EMPTY_TRACKS",
                format!("$.clips[{clip_index}].tracks"),
                "clip.tracks must be an array",
                "next step · add at least one track to each clip",
            ));
            continue;
        };
        validate_clip_tracks(tracks, clip_index, &mut components, report);
        track_count += tracks.len();
    }
    report.track_count = track_count;
    report.asset_count = raw
        .get("assets")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    report.components = components.into_iter().collect();
    validate_assets(raw, composition_dir, report);
}

fn validate_clip_tracks(
    tracks: &[Value],
    clip_index: usize,
    components: &mut BTreeSet<String>,
    report: &mut ValidationReport,
) {
    if tracks.is_empty() {
        report.push_error(ValidationError::new(
            "EMPTY_TRACKS",
            format!("$.clips[{clip_index}].tracks"),
            "clip must include at least one track",
            "next step · add at least one visual or audio track",
        ));
    }
    for (track_index, track) in tracks.iter().enumerate() {
        let Some(track_object) = track.as_object() else {
            report.push_error(ValidationError::new(
                "COMPOSITION_INVALID",
                format!("$.clips[{clip_index}].tracks[{track_index}]"),
                "track must be an object",
                "next step · make every track a JSON object",
            ));
            continue;
        };
        let kind = track_object
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("component");
        if kind != "component" {
            continue;
        }
        match track_object
            .get("component")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
        {
            Some(component) => {
                components.insert(component.to_string());
            }
            None => report.push_error(ValidationError::new(
                "COMPONENT_MISSING",
                format!("$.clips[{clip_index}].tracks[{track_index}].component"),
                "component track is missing component",
                "next step · set component to a registered component id",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use serde_json::json;

    use super::super::structural::validate_structure;

    #[test]
    fn accepts_clip_first_composition() -> Result<(), Box<dyn std::error::Error>> {
        let dir = unique_dir("clip-first")?;
        let path = dir.join("composition.json");
        fs::write(
            &path,
            serde_json::to_vec_pretty(&json!({
                "schema": "capy.timeline.composition.v2",
                "id": "video-demo",
                "name": "Video Demo",
                "clips": [{
                    "id": "intro",
                    "duration": "2s",
                    "tracks": [{
                        "id": "title",
                        "kind": "component",
                        "component": "html.capy-title",
                        "items": [{
                            "id": "headline",
                            "time": {"start": "in", "end": "out"},
                            "params": {"title": "Launch"}
                        }]
                    }]
                }]
            }))?,
        )?;

        let report = validate_structure(&path);

        assert!(report.ok);
        assert_eq!(report.schema_version, "capy.timeline.composition.v2");
        assert_eq!(report.track_count, 1);
        assert_eq!(report.components, vec!["html.capy-title"]);
        fs::remove_dir_all(dir)?;
        Ok(())
    }

    fn unique_dir(label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir().join(format!(
            "capy-timeline-validate-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_millis()
        ));
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }
}
