use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

#[test]
fn attach_happy_path_records_transition_history() -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_dir("happy")?;
    let path = write_composition(&dir, compilable_composition())?;
    let state = capy_shell::app::ShellState::default();

    let value = capy_shell::app::nextframe::attach_node(
        &state,
        json!({"canvas_node_id": 0, "composition_path": path}),
    )?;

    assert_eq!(value["report"]["ok"], true);
    assert_eq!(value["report"]["node_state"], "preview-ready");
    assert_eq!(value["node"]["kind"], "nextframe-composition");
    assert_eq!(value["node"]["composition_ref"]["track_count"], 1);
    assert_eq!(value["node"]["history"].as_array().map(Vec::len), Some(3));
    assert_eq!(value["node"]["history"][0]["from"], "draft");
    assert_eq!(value["node"]["history"][2]["to"], "preview-ready");
    fs::remove_dir_all(dir)?;
    Ok(())
}

#[test]
fn attach_reports_canvas_node_not_found() -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_dir("missing-node")?;
    let path = write_composition(&dir, compilable_composition())?;
    let state = capy_shell::app::ShellState::default();

    let error = capy_shell::app::nextframe::attach_node(
        &state,
        json!({"canvas_node_id": 42, "composition_path": path}),
    )
    .expect_err("missing node should fail");
    let value: Value = serde_json::from_str(&error)?;

    assert_eq!(value["code"], "CANVAS_NODE_NOT_FOUND");
    fs::remove_dir_all(dir)?;
    Ok(())
}

#[test]
fn attach_reports_invalid_composition() -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_dir("invalid")?;
    let path = write_composition(&dir, json!({"tracks": []}))?;
    let state = capy_shell::app::ShellState::default();

    let error = capy_shell::app::nextframe::attach_node(
        &state,
        json!({"canvas_node_id": 0, "composition_path": path}),
    )
    .expect_err("invalid composition should fail");
    let value: Value = serde_json::from_str(&error)?;
    let state_value =
        capy_shell::app::nextframe::state_nodes(&state, json!({"canvas_node_id": 0}))?;

    assert_eq!(value["code"], "INVALID_COMPOSITION");
    assert_eq!(
        state_value["attachments"][0]["state"]["error"]["code"],
        "COMPOSITION_INVALID"
    );
    fs::remove_dir_all(dir)?;
    Ok(())
}

#[test]
fn attach_lands_compile_failure_as_error() -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_dir("compile-error")?;
    let path = write_composition(&dir, compilable_composition())?;
    fs::create_dir(dir.join("render_source.json"))?;
    let state = capy_shell::app::ShellState::default();

    let error = capy_shell::app::nextframe::attach_node(
        &state,
        json!({"canvas_node_id": 0, "composition_path": path}),
    )
    .expect_err("compile failure should fail attach");
    let value: Value = serde_json::from_str(&error)?;
    let state_value =
        capy_shell::app::nextframe::state_nodes(&state, json!({"canvas_node_id": 0}))?;

    assert_eq!(value["code"], "COMPILE_FAILED");
    assert!(
        state_value["attachments"][0]["state"]["error"]["code"]
            .as_str()
            .is_some_and(|code| code != "COMPOSITION_INVALID")
    );
    assert_eq!(
        state_value["attachments"][0]["history"]
            .as_array()
            .map(Vec::len),
        Some(2)
    );
    fs::remove_dir_all(dir)?;
    Ok(())
}

#[test]
fn state_nodes_returns_single_attachment() -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_dir("state-single")?;
    let path = write_composition(&dir, compilable_composition())?;
    let state = capy_shell::app::ShellState::default();
    capy_shell::app::nextframe::attach_node(
        &state,
        json!({"canvas_node_id": 0, "composition_path": path}),
    )?;

    let value = capy_shell::app::nextframe::state_nodes(&state, json!({"canvas_node_id": 0}))?;

    assert_eq!(value["ok"], true);
    assert_eq!(value["stage"], "state");
    assert_eq!(value["attachments"].as_array().map(Vec::len), Some(1));
    assert_eq!(value["attachments"][0]["state"], "preview-ready");
    fs::remove_dir_all(dir)?;
    Ok(())
}

fn write_composition(dir: &PathBuf, value: Value) -> Result<PathBuf, Box<dyn std::error::Error>> {
    fs::create_dir_all(dir.join("components"))?;
    let path = dir.join("composition.json");
    fs::write(&path, serde_json::to_string_pretty(&value)?)?;
    fs::write(
        dir.join("components").join("html.capy-poster.js"),
        "export function mount(root) { root.textContent = 'ok'; }\nexport function update() {}\n",
    )?;
    Ok(path)
}

fn structurally_valid_uncompilable_composition() -> Value {
    json!({
        "schema": "nextframe.composition.v2",
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
            "params": {"poster": {"type": "poster"}}
        }],
        "assets": []
    })
}

fn compilable_composition() -> Value {
    let mut value = structurally_valid_uncompilable_composition();
    value["tracks"][0]["params"]["poster"] = json!({
        "version": "capy-poster-v0.1",
        "type": "poster",
        "canvas": {
            "width": 1920,
            "height": 1080,
            "aspectRatio": "16:9",
            "background": "#ffffff"
        },
        "assets": {},
        "layers": [{
            "id": "title",
            "type": "text",
            "x": 10,
            "y": 10,
            "width": 400,
            "height": 100,
            "z": 1,
            "text": "Hello",
            "style": {"fontSize": 48, "color": "#111111"}
        }]
    });
    value
}

fn unique_dir(label: &str) -> Result<PathBuf, std::time::SystemTimeError> {
    Ok(std::env::temp_dir().join(format!(
        "capy-shell-nextframe-{label}-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis()
    )))
}
