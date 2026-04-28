use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;

use interprocess::local_socket::tokio::prelude::*;
use interprocess::local_socket::{GenericFilePath, ListenerOptions, ToFsName};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[test]
fn nextframe_state_reports_shell_unavailable_json() -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_dir("state-no-shell")?;
    let socket = dir.join("missing.sock");

    let output = capy_command()?
        .env("CAPYBARA_SOCKET", &socket)
        .args(["nextframe", "state"])
        .output()?;

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());
    let value: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(value["ok"], false);
    assert_eq!(value["stage"], "state");
    assert_eq!(value["code"], "SHELL_UNAVAILABLE");
    assert_eq!(value["errors"][0]["code"], "SHELL_UNAVAILABLE");
    fs::remove_dir_all(dir)?;
    Ok(())
}

#[test]
fn nextframe_state_reports_all_attachments_json() -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_dir("state-happy")?;
    let socket = short_socket_path("happy")?;
    let _server = fake_state_shell(&socket, state_response(None))?;

    let output = capy_command()?
        .env("CAPYBARA_SOCKET", &socket)
        .args(["nextframe", "state"])
        .output()?;

    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(value["ok"], true);
    assert_eq!(value["stage"], "state");
    assert_eq!(value["attachments"].as_array().map(Vec::len), Some(1));
    assert_eq!(value["attachments"][0]["canvas_node_id"], 0);
    assert_eq!(value["attachments"][0]["state"], "preview-ready");
    let _cleanup = fs::remove_file(socket);
    fs::remove_dir_all(dir)?;
    Ok(())
}

#[test]
fn nextframe_state_sends_single_node_query() -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_dir("state-single")?;
    let socket = short_socket_path("single")?;
    let _server = fake_state_shell(&socket, state_response(Some(7)))?;

    let output = capy_command()?
        .env("CAPYBARA_SOCKET", &socket)
        .args(["nextframe", "state", "--canvas-node", "7"])
        .output()?;

    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(value["ok"], true);
    assert_eq!(value["attachments"][0]["canvas_node_id"], 7);
    assert_eq!(
        value["attachments"][0]["composition_path"],
        "/tmp/composition.json"
    );
    let _cleanup = fs::remove_file(socket);
    fs::remove_dir_all(dir)?;
    Ok(())
}

fn capy_command() -> Result<Command, Box<dyn std::error::Error>> {
    let path = std::env::var("CARGO_BIN_EXE_capy")?;
    Ok(Command::new(path))
}

fn unique_dir(label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = std::env::temp_dir().join(format!(
        "capy-nextframe-state-cli-{label}-{}-{}",
        std::process::id(),
        monotonic_millis()?
    ));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn monotonic_millis() -> Result<u128, std::time::SystemTimeError> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis())
}

fn short_socket_path(label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(PathBuf::from(format!(
        "/tmp/capy-state-{label}-{}-{}.sock",
        std::process::id(),
        monotonic_millis()?
    )))
}

fn fake_state_shell(
    socket: &Path,
    response_data: Value,
) -> Result<std::thread::JoinHandle<()>, Box<dyn std::error::Error>> {
    let socket = socket.to_path_buf();
    let (ready_tx, ready_rx) = mpsc::channel();
    let handle = std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test socket runtime should build");
        runtime.block_on(async move {
            let name = socket
                .to_str()
                .expect("test socket path should be UTF-8")
                .to_fs_name::<GenericFilePath>()
                .expect("test socket name should be valid");
            let listener = ListenerOptions::new()
                .name(name)
                .create_tokio()
                .expect("test socket listener should bind");
            ready_tx.send(()).expect("test should wait for readiness");
            let conn = listener.accept().await.expect("test socket should accept");
            let (read_half, mut write_half) = conn.split();
            let mut reader = BufReader::new(read_half);
            let mut line = String::new();
            reader
                .read_line(&mut line)
                .await
                .expect("test socket should read request");
            let request: Value =
                serde_json::from_str(line.trim_end()).expect("request should be JSON");
            assert_eq!(request["op"], "nextframe-state");
            let response = json!({
                "req_id": request["req_id"],
                "ok": true,
                "data": response_data
            });
            let mut payload = serde_json::to_string(&response).expect("response should serialize");
            payload.push('\n');
            write_half
                .write_all(payload.as_bytes())
                .await
                .expect("test socket should write response");
            write_half.flush().await.expect("test socket should flush");
        });
    });
    ready_rx.recv()?;
    Ok(handle)
}

fn state_response(canvas_node_id: Option<u64>) -> Value {
    let id = canvas_node_id.unwrap_or(0);
    json!({
        "ok": true,
        "trace_id": "state-test",
        "stage": "state",
        "attachments": [{
            "canvas_node_id": id,
            "composition_path": "/tmp/composition.json",
            "state": "preview-ready",
            "schema_version": "capy.composition.v1",
            "track_count": 1,
            "asset_count": 0,
            "history": [{
                "from": "compiled",
                "to": "preview-ready",
                "at": "2026-04-28T00:00:00Z",
                "reason": "test"
            }]
        }]
    })
}
