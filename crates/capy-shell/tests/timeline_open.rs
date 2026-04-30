use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

#[test]
fn timeline_open_registers_loopback_preview() -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_dir("open-preview")?;
    let path = write_composition(&dir, valid_composition())?;
    let state = capy_shell::app::ShellState::default();

    capy_shell::app::timeline::attach_node(
        &state,
        json!({"canvas_node_id": 0, "composition_path": path}),
    )?;
    let state_value = capy_shell::app::timeline::state_nodes(&state, json!({"canvas_node_id": 0}))?;
    assert_eq!(state_value["attachments"][0]["state"], "preview-ready");

    let open = capy_shell::app::timeline::open_node(&state, json!({"canvas_node_id": 0}))?;
    let preview_url = open["preview_url"]
        .as_str()
        .ok_or("preview_url should be a string")?;
    assert!(preview_url.starts_with("http://127.0.0.1:"));

    let response = http_get(preview_url)?;
    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains("data-capy-timeline-preview-slug"));
    assert!(response.contains("render_source.json"));

    fs::remove_dir_all(dir)?;
    Ok(())
}

fn http_get(url: &str) -> Result<String, Box<dyn std::error::Error>> {
    let url = url
        .strip_prefix("http://")
        .ok_or("preview URL should be http")?;
    let (host_port, path) = url
        .split_once('/')
        .ok_or("preview URL should include path")?;
    let mut stream = TcpStream::connect(host_port)?;
    stream.write_all(format!("GET /{path} HTTP/1.1\r\nHost: {host_port}\r\n\r\n").as_bytes())?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    Ok(response)
}

fn write_composition(dir: &Path, value: Value) -> Result<PathBuf, Box<dyn std::error::Error>> {
    fs::create_dir_all(dir.join("components"))?;
    let path = dir.join("composition.json");
    fs::write(&path, serde_json::to_string_pretty(&value)?)?;
    fs::write(
        dir.join("components").join("html.capy-poster.js"),
        "export function mount(root) { root.textContent = 'mounted'; }\nexport function update() {}\n",
    )?;
    Ok(path)
}

fn valid_composition() -> Value {
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
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis()
    )))
}
