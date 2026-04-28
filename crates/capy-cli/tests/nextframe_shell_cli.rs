use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;

use interprocess::local_socket::tokio::prelude::*;
use interprocess::local_socket::{GenericFilePath, ListenerOptions, ToFsName};
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[test]
fn nextframe_attach_reports_shell_unavailable_json() -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_dir("attach-no-shell")?;
    let composition = dir.join("composition.json");
    fs::write(&composition, valid_composition_text())?;
    let socket = dir.join("missing.sock");

    let output = capy_command()?
        .args([
            "nextframe",
            "attach",
            "--canvas-node",
            "0",
            "--composition",
            &composition.display().to_string(),
            "--socket",
            &socket.display().to_string(),
        ])
        .output()?;

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(value["ok"], false);
    assert_eq!(value["stage"], "attach");
    assert_eq!(value["code"], "SHELL_UNAVAILABLE");
    assert_eq!(value["errors"][0]["code"], "SHELL_UNAVAILABLE");
    assert_eq!(value["canvas_node_id"], 0);
    assert_eq!(value["ipc_socket"], socket.display().to_string());
    fs::remove_dir_all(dir)?;
    Ok(())
}

#[test]
fn nextframe_open_reports_shell_unavailable_json() -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_dir("open-no-shell")?;
    let socket = dir.join("missing.sock");

    let output = capy_command()?
        .args([
            "nextframe",
            "open",
            "--canvas-node",
            "0",
            "--socket",
            &socket.display().to_string(),
        ])
        .output()?;

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(value["ok"], false);
    assert_eq!(value["stage"], "open");
    assert_eq!(value["code"], "SHELL_UNAVAILABLE");
    assert_eq!(value["errors"][0]["code"], "SHELL_UNAVAILABLE");
    assert_eq!(value["canvas_node_id"], 0);
    fs::remove_dir_all(dir)?;
    Ok(())
}

#[test]
fn nextframe_open_reports_not_preview_ready_json() -> Result<(), Box<dyn std::error::Error>> {
    let socket = short_socket_path("open-not-ready")?;
    let _server = fake_open_shell(&socket, false)?;

    let output = capy_command()?
        .args([
            "nextframe",
            "open",
            "--canvas-node",
            "7",
            "--socket",
            &socket.display().to_string(),
        ])
        .output()?;

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(value["ok"], false);
    assert_eq!(value["stage"], "open");
    assert_eq!(value["code"], "NOT_PREVIEW_READY");
    assert_eq!(value["canvas_node_id"], 7);
    let _cleanup = fs::remove_file(socket);
    Ok(())
}

fn capy_command() -> Result<Command, Box<dyn std::error::Error>> {
    let path = std::env::var("CARGO_BIN_EXE_capy")?;
    Ok(Command::new(path))
}

fn unique_dir(label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = std::env::temp_dir().join(format!(
        "capy-nextframe-shell-cli-{label}-{}-{}",
        std::process::id(),
        monotonic_millis()?
    ));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn short_socket_path(label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(PathBuf::from(format!(
        "/tmp/capy-open-{label}-{}-{}.sock",
        std::process::id(),
        monotonic_millis()?
    )))
}

fn fake_open_shell(
    socket: &Path,
    ok: bool,
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
            let request: serde_json::Value =
                serde_json::from_str(line.trim_end()).expect("request should be JSON");
            assert_eq!(request["op"], "nextframe-open");
            assert_eq!(request["params"]["canvas_node_id"], 7);
            let response = if ok {
                json!({
                    "req_id": request["req_id"],
                    "ok": true,
                    "data": {
                        "ok": true,
                        "trace_id": "open-test",
                        "stage": "open",
                        "canvas_node_id": 7,
                        "preview_url": "http://127.0.0.1:1/node-7/index.html",
                        "selectors": {}
                    }
                })
            } else {
                json!({
                    "req_id": request["req_id"],
                    "ok": false,
                    "error": {
                        "code": "NOT_PREVIEW_READY",
                        "message": "canvas node 7 NextFrame state is compiled",
                        "hint": "next step · run capy nextframe attach"
                    }
                })
            };
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

fn monotonic_millis() -> Result<u128, std::time::SystemTimeError> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis())
}

fn valid_composition_text() -> &'static str {
    r#"{"schema":"nextframe.composition.v2","schema_version":"capy.composition.v1","id":"poster-snapshot","title":"Poster Snapshot","name":"Poster Snapshot","duration_ms":1000,"duration":"1000ms","viewport":{"w":1920,"h":1080,"ratio":"16:9"},"theme":"default","tracks":[{"id":"track-poster","kind":"component","component":"html.capy-poster","z":10,"time":{"start":"0ms","end":"1000ms"},"duration_ms":1000,"params":{"poster":{"type":"poster"}}}],"assets":[]}"#
}
