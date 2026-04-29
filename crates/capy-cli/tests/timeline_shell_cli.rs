use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;

use interprocess::local_socket::tokio::prelude::*;
use interprocess::local_socket::{GenericFilePath, ListenerOptions, ToFsName};
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[test]
fn timeline_attach_reports_shell_unavailable_json() -> TestResult<()> {
    let dir = unique_dir("attach-no-shell")?;
    let composition = dir.join("composition.json");
    fs::write(&composition, valid_composition_text())?;
    let socket = dir.join("missing.sock");

    let output = capy_command()?
        .args([
            "timeline",
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
fn timeline_open_reports_shell_unavailable_json() -> TestResult<()> {
    let dir = unique_dir("open-no-shell")?;
    let socket = dir.join("missing.sock");

    let output = capy_command()?
        .args([
            "timeline",
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
fn timeline_open_reports_not_preview_ready_json() -> TestResult<()> {
    let socket = short_socket_path("open-not-ready")?;
    let server = fake_open_shell(&socket, false)?;

    let output = capy_command()?
        .args([
            "timeline",
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
    join_fake_shell(server)?;
    let _cleanup = fs::remove_file(socket);
    Ok(())
}

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

fn capy_command() -> TestResult<Command> {
    let path = std::env::var("CARGO_BIN_EXE_capy")?;
    Ok(Command::new(path))
}

fn unique_dir(label: &str) -> TestResult<PathBuf> {
    let dir = std::env::temp_dir().join(format!(
        "capy-timeline-shell-cli-{label}-{}-{}",
        std::process::id(),
        monotonic_millis()?
    ));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn short_socket_path(label: &str) -> TestResult<PathBuf> {
    Ok(PathBuf::from(format!(
        "/tmp/capy-open-{label}-{}-{}.sock",
        std::process::id(),
        monotonic_millis()?
    )))
}

fn fake_open_shell(socket: &Path, ok: bool) -> TestResult<std::thread::JoinHandle<TestResult<()>>> {
    let socket = socket.to_path_buf();
    let (ready_tx, ready_rx) = mpsc::channel();
    let handle = std::thread::spawn(move || -> TestResult<()> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        runtime.block_on(async move {
            let socket_str = socket.to_str().ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "test socket path should be UTF-8",
                )
            })?;
            let name = socket_str.to_fs_name::<GenericFilePath>()?;
            let listener = ListenerOptions::new().name(name).create_tokio()?;
            ready_tx.send(())?;
            let conn = listener.accept().await?;
            let (read_half, mut write_half) = conn.split();
            let mut reader = BufReader::new(read_half);
            let mut line = String::new();
            let bytes = reader.read_line(&mut line).await?;
            if bytes == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "empty IPC request",
                )
                .into());
            }
            let request: serde_json::Value = serde_json::from_str(line.trim_end())?;
            if request["op"] != "timeline-open" || request["params"]["canvas_node_id"] != 7 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("unexpected IPC request: {request}"),
                )
                .into());
            }
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
                        "message": "canvas node 7 Timeline state is compiled",
                        "hint": "next step · run capy timeline attach"
                    }
                })
            };
            let mut payload = serde_json::to_string(&response)?;
            payload.push('\n');
            write_half.write_all(payload.as_bytes()).await?;
            write_half.flush().await?;
            Ok(())
        })
    });
    ready_rx.recv()?;
    Ok(handle)
}

fn join_fake_shell(handle: std::thread::JoinHandle<TestResult<()>>) -> TestResult<()> {
    match handle.join() {
        Ok(result) => result,
        Err(_) => Err(std::io::Error::other("fake shell thread panicked").into()),
    }
}

fn monotonic_millis() -> Result<u128, std::time::SystemTimeError> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis())
}

fn valid_composition_text() -> &'static str {
    r#"{"schema":"capy.timeline.composition.v1","schema_version":"capy.composition.v1","id":"poster-snapshot","title":"Poster Snapshot","name":"Poster Snapshot","duration_ms":1000,"duration":"1000ms","viewport":{"w":1920,"h":1080,"ratio":"16:9"},"theme":"default","tracks":[{"id":"track-poster","kind":"component","component":"html.capy-poster","z":10,"time":{"start":"0ms","end":"1000ms"},"duration_ms":1000,"params":{"poster":{"type":"poster"}}}],"assets":[]}"#
}
