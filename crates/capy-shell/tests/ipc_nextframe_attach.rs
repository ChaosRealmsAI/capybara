use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

#[test]
fn nextframe_attach_contract_updates_shell_state() -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_dir("contract")?;
    fs::create_dir_all(&dir)?;
    let composition = dir.join("composition.json");
    fs::write(&composition, valid_composition_text())?;
    let state = capy_shell::app::ShellState::default();

    let value = capy_shell::app::nextframe::attach_node(
        &state,
        json!({
            "canvas_node_id": 0,
            "composition_path": composition
        }),
    )
    .map_err(std::io::Error::other)?;

    assert_eq!(value["report"]["ok"], true);
    assert_eq!(value["event"]["canvas_node_id"], 0);
    assert_eq!(value["event"]["composition_ref"]["track_count"], 1);

    let request = capy_shell::ipc::IpcRequest {
        req_id: "state-test".to_string(),
        op: "state-query".to_string(),
        params: json!({"key": "nextframe.attachments"}),
    };
    let response = state.state_query(request);
    assert!(response.ok);
    let data = response.data.ok_or("state response should include data")?;
    let attachments: Value = data["value"].clone();
    assert_eq!(attachments["0"]["kind"], "nextframe-composition");
    assert_eq!(attachments["0"]["state"], "preview-ready");
    assert_eq!(
        attachments["0"]["history"].as_array().map(Vec::len),
        Some(3)
    );

    let state_request = capy_shell::ipc::IpcRequest {
        req_id: "nextframe-state-test".to_string(),
        op: "nextframe-state".to_string(),
        params: json!({"canvas_node_id": 0}),
    };
    let state_response = state.nextframe_state_query(state_request);
    assert!(state_response.ok);
    let state_data = state_response
        .data
        .ok_or("state response should include data")?;
    assert_eq!(state_data["attachments"][0]["state"], "preview-ready");

    fs::remove_dir_all(dir)?;
    Ok(())
}

fn valid_composition_text() -> &'static str {
    r##"{"schema":"nextframe.composition.v2","schema_version":"capy.composition.v1","id":"poster-snapshot","title":"Poster Snapshot","name":"Poster Snapshot","duration_ms":1000,"duration":"1000ms","viewport":{"w":1920,"h":1080,"ratio":"16:9"},"theme":"default","tracks":[{"id":"track-poster","kind":"component","component":"html.capy-poster","z":10,"time":{"start":"0ms","end":"1000ms"},"duration_ms":1000,"params":{"poster":{"version":"capy-poster-v0.1","type":"poster","canvas":{"width":1920,"height":1080,"aspectRatio":"16:9","background":"#fff"},"assets":{},"layers":[{"id":"title","type":"text","x":10,"y":10,"width":400,"height":100,"z":1,"text":"Hello","style":{"fontSize":48,"color":"#111"}}]}}}],"assets":[]}"##
}

fn unique_dir(label: &str) -> Result<PathBuf, std::time::SystemTimeError> {
    Ok(std::env::temp_dir().join(format!(
        "capy-shell-ipc-nextframe-{label}-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis()
    )))
}
