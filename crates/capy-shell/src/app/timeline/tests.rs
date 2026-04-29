use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::{ExportJob, ExportJobStatus, open_node, state_nodes};
use crate::app::ShellState;

#[test]
fn open_happy_path_returns_preview_url_and_selectors() -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_dir("open-happy")?;
    let path = write_composition(&dir, compilable_composition())?;
    let state = ShellState::default();
    super::attach_node(
        &state,
        json!({"canvas_node_id": 0, "composition_path": path}),
    )?;

    let value = open_node(&state, json!({"canvas_node_id": 0}))?;

    assert_eq!(value["ok"], true);
    assert_eq!(value["stage"], "open");
    assert_eq!(value["canvas_node_id"], 0);
    assert!(
        value["preview_url"]
            .as_str()
            .unwrap_or("")
            .starts_with("http://127.0.0.1:")
    );
    assert_eq!(
        value["selectors"]["preview"],
        "[data-capy-component-kind='timeline-composition'][data-canvas-node-id='0'] [data-capy-timeline-preview]"
    );
    fs::remove_dir_all(dir)?;
    Ok(())
}

#[test]
fn open_rejects_not_preview_ready_node() -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_dir("open-not-ready")?;
    let path = write_composition(&dir, json!({"tracks": []}))?;
    let state = ShellState::default();
    let attach_result = super::attach_node(
        &state,
        json!({"canvas_node_id": 0, "composition_path": path}),
    );
    assert!(attach_result.is_err());

    let error = open_node(&state, json!({"canvas_node_id": 0}))
        .err()
        .ok_or("error-state attachment should not be preview-ready")?;
    let value: Value = serde_json::from_str(&error)?;

    assert_eq!(value["code"], "NOT_PREVIEW_READY");
    assert_eq!(
        state_nodes(&state, json!({"canvas_node_id": 0}))?["attachments"][0]["state"]["error"]["code"],
        "COMPOSITION_INVALID"
    );
    fs::remove_dir_all(dir)?;
    Ok(())
}

#[test]
fn open_reports_canvas_node_not_found() -> Result<(), Box<dyn std::error::Error>> {
    let state = ShellState::default();

    let error = open_node(&state, json!({"canvas_node_id": 99}))
        .err()
        .ok_or("unknown canvas node should fail")?;
    let value: Value = serde_json::from_str(&error)?;

    assert_eq!(value["code"], "CANVAS_NODE_NOT_FOUND");
    Ok(())
}

#[test]
fn export_status_and_cancel_read_tracked_jobs() -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_dir("export-job")?;
    let path = write_composition(&dir, compilable_composition())?;
    let state = ShellState::default();
    super::attach_node(
        &state,
        json!({"canvas_node_id": 0, "composition_path": path}),
    )?;
    let mut node = state
        .timeline_node(0)?
        .ok_or("attached node should be present")?;
    node.export_jobs.push(ExportJob {
        job_id: "exp-test".to_string(),
        status: ExportJobStatus::Running,
        progress: 50,
        output_path: Some(dir.join("out.mp4").display().to_string()),
        byte_size: None,
        started_at: "1970-01-01T00:00:00Z".to_string(),
    });
    state.attach_timeline_node(0, node)?;

    let status = super::export_status(&state, json!({"job_id": "exp-test"}))?;
    assert_eq!(status["job"]["status"], "running");
    let cancel = super::export_cancel(&state, json!({"job_id": "exp-test"}))?;
    assert_eq!(cancel["job"]["status"], "cancelled");

    fs::remove_dir_all(dir)?;
    Ok(())
}

fn write_composition(dir: &Path, value: Value) -> Result<PathBuf, Box<dyn std::error::Error>> {
    fs::create_dir_all(dir.join("components"))?;
    let path = dir.join("composition.json");
    fs::write(&path, serde_json::to_string_pretty(&value)?)?;
    fs::write(
        dir.join("components").join("html.capy-poster.js"),
        "export function mount(root) { root.textContent = 'ok'; }\nexport function update() {}\n",
    )?;
    Ok(path)
}

fn compilable_composition() -> Value {
    json!({
        "schema": "capy.timeline.composition.v1",
        "schema_version": "capy.composition.v1",
        "id": "poster-snapshot",
        "title": "Poster Snapshot",
        "name": "Poster Snapshot",
        "duration_ms": 1000,
        "duration": "1000ms",
        "viewport": {"w": 1920, "h": 1080, "ratio": "16:9"},
        "theme": "default",
        "tracks": [{
            "id": "track-poster",
            "kind": "component",
            "component": "html.capy-poster",
            "z": 10,
            "time": {"start": "0ms", "end": "1000ms"},
            "duration_ms": 1000,
            "params": {"poster": {
                "version": "capy-poster-v0.1",
                "type": "poster",
                "canvas": {"width": 1920, "height": 1080, "aspectRatio": "16:9", "background": "#fff"},
                "assets": {},
                "layers": [{"id": "title", "type": "text", "x": 10, "y": 10, "width": 400, "height": 100, "z": 1, "text": "Hello", "style": {"fontSize": 48, "color": "#111"}}]
            }}
        }],
        "assets": []
    })
}

fn unique_dir(label: &str) -> Result<PathBuf, std::time::SystemTimeError> {
    Ok(std::env::temp_dir().join(format!(
        "capy-shell-timeline-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis()
    )))
}
