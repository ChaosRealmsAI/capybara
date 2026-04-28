use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use interprocess::local_socket::tokio::prelude::*;
use interprocess::local_socket::{GenericFilePath, ListenerOptions, ToFsName};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tao::event_loop::EventLoopProxy;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::oneshot;

use crate::app::{ShellEvent, ShellState};

const DEFAULT_EVENT_ACK_TIMEOUT: Duration = Duration::from_secs(60);
const EVENT_ACK_TIMEOUT_ENV: &str = "CAPY_EVENT_ACK_TIMEOUT_SECS";
const SOCKET_ENV: &str = "CAPYBARA_SOCKET";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcRequest {
    pub req_id: String,
    pub op: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcResponse {
    pub req_id: String,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<Value>,
}

pub fn socket_path() -> PathBuf {
    if let Some(path) = std::env::var_os(SOCKET_ENV).filter(|value| !value.is_empty()) {
        return PathBuf::from(path);
    }
    let uid = get_uid();
    PathBuf::from(format!("/tmp/capybara-{uid}.sock"))
}

pub fn spawn_server_thread(
    proxy: EventLoopProxy<ShellEvent>,
    state: Arc<ShellState>,
) -> Result<std::thread::JoinHandle<()>, String> {
    let path = socket_path();
    cleanup_stale_socket(&path)?;
    install_ctrlc_cleanup(path.clone())?;

    let handle = std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(err) => {
                eprintln!("capy-shell IPC runtime failed: {err}");
                return;
            }
        };
        if let Err(err) = runtime.block_on(serve(path, proxy, state)) {
            eprintln!("capy-shell IPC server failed: {err}");
        }
    });

    Ok(handle)
}

async fn serve(
    path: PathBuf,
    proxy: EventLoopProxy<ShellEvent>,
    state: Arc<ShellState>,
) -> Result<(), String> {
    let name = path
        .to_str()
        .ok_or_else(|| format!("socket path is not UTF-8: {path:?}"))?
        .to_fs_name::<GenericFilePath>()
        .map_err(|err| err.to_string())?;
    let listener = ListenerOptions::new()
        .name(name)
        .create_tokio()
        .map_err(|err| err.to_string())?;

    loop {
        let conn = listener.accept().await.map_err(|err| err.to_string())?;
        let proxy = proxy.clone();
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            let _result = handle_connection(conn, proxy, state).await;
        });
    }
}

async fn handle_connection(
    conn: interprocess::local_socket::tokio::Stream,
    proxy: EventLoopProxy<ShellEvent>,
    state: Arc<ShellState>,
) -> Result<(), String> {
    let (read_half, mut write_half) = conn.split();
    let mut reader = BufReader::new(read_half);
    let mut line = String::new();

    loop {
        line.clear();
        let bytes = reader
            .read_line(&mut line)
            .await
            .map_err(|err| err.to_string())?;
        if bytes == 0 {
            return Ok(());
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<IpcRequest>(trimmed) {
            Ok(req) => dispatch(req, &proxy, &state).await,
            Err(err) => IpcResponse {
                req_id: "parse-error".to_string(),
                ok: false,
                data: None,
                error: Some(json!({
                    "error": "validation failed",
                    "detail": format!("invalid NDJSON request: {err}"),
                    "hint": "send one JSON request per line",
                    "exit_code": 2
                })),
            },
        };

        let mut payload = serde_json::to_string(&response).map_err(|err| err.to_string())?;
        payload.push('\n');
        write_half
            .write_all(payload.as_bytes())
            .await
            .map_err(|err| err.to_string())?;
    }
}

async fn dispatch(
    req: IpcRequest,
    proxy: &EventLoopProxy<ShellEvent>,
    state: &ShellState,
) -> IpcResponse {
    match req.op.as_str() {
        "state-query" if state.can_answer_directly(&req) => state.state_query(req),
        "state-query" => {
            send_event(req, proxy, |request, ack| ShellEvent::StateQuery {
                request,
                ack,
            })
            .await
        }
        "open-window" => {
            send_event(req, proxy, |request, ack| ShellEvent::OpenWindow {
                request,
                ack,
            })
            .await
        }
        "devtools-query" => {
            send_event(req, proxy, |request, ack| ShellEvent::DevtoolsQuery {
                request,
                ack,
            })
            .await
        }
        "devtools-eval" => {
            send_event(req, proxy, |request, ack| ShellEvent::DevtoolsEval {
                request,
                ack,
            })
            .await
        }
        "screenshot" => {
            send_event(req, proxy, |request, ack| ShellEvent::Screenshot {
                request,
                ack,
            })
            .await
        }
        "capture" => {
            send_event(req, proxy, |request, ack| ShellEvent::CaptureWindow {
                request,
                ack,
            })
            .await
        }
        "conversation-list"
        | "conversation-create"
        | "conversation-open"
        | "conversation-events"
        | "conversation-send"
        | "conversation-stop"
        | "conversation-update-config"
        | "agent-doctor" => {
            send_event(req, proxy, |request, ack| ShellEvent::ConversationRequest {
                request,
                ack,
            })
            .await
        }
        "nextframe-attach" => {
            send_event(req, proxy, |request, ack| ShellEvent::NextFrameAttach {
                request,
                ack,
            })
            .await
        }
        "quit" => send_event(req, proxy, |request, ack| ShellEvent::Quit { request, ack }).await,
        _ => error_response(&req.req_id, format!("unknown op: {}", req.op)),
    }
}

async fn send_event(
    req: IpcRequest,
    proxy: &EventLoopProxy<ShellEvent>,
    build: impl FnOnce(IpcRequest, oneshot::Sender<IpcResponse>) -> ShellEvent,
) -> IpcResponse {
    let req_id = req.req_id.clone();
    let (tx, rx) = oneshot::channel();
    if let Err(err) = proxy.send_event(build(req, tx)) {
        return IpcResponse {
            req_id,
            ok: false,
            data: None,
            error: Some(json!({
                "error": "socket failed",
                "detail": format!("event loop proxy failed: {err}"),
                "hint": "restart capy shell",
                "exit_code": 1
            })),
        };
    }

    let timeout = event_ack_timeout();
    match tokio::time::timeout(timeout, rx).await {
        Ok(Ok(resp)) => resp,
        Ok(Err(err)) => IpcResponse {
            req_id,
            ok: false,
            data: None,
            error: Some(json!({
                "error": "socket failed",
                "detail": format!("event ack dropped: {err}"),
                "hint": "restart capy shell",
                "exit_code": 1
            })),
        },
        Err(_) => IpcResponse {
            req_id,
            ok: false,
            data: None,
            error: Some(json!({
                "error": "socket failed",
                "detail": format!("event ack timed out after {}s", timeout.as_secs()),
                "hint": "restart capy shell",
                "exit_code": 1
            })),
        },
    }
}

fn event_ack_timeout() -> Duration {
    env::var(EVENT_ACK_TIMEOUT_ENV)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|seconds| *seconds > 0)
        .map(Duration::from_secs)
        .unwrap_or(DEFAULT_EVENT_ACK_TIMEOUT)
}

pub fn ok_response(req: &IpcRequest, data: Value) -> IpcResponse {
    IpcResponse {
        req_id: req.req_id.clone(),
        ok: true,
        data: Some(data),
        error: None,
    }
}

pub fn error_response(req_id: &str, detail: impl Into<String>) -> IpcResponse {
    IpcResponse {
        req_id: req_id.to_string(),
        ok: false,
        data: None,
        error: Some(json!({
            "error": "validation failed",
            "detail": detail.into(),
            "hint": "run `capy <cmd> --help` for expected format",
            "exit_code": 2
        })),
    }
}

pub fn write_ready_event() {
    let mut stdout = std::io::stdout().lock();
    let _write_result = writeln!(
        stdout,
        "{}",
        json!({
            "event": "ready",
            "bin": "capy-shell",
            "version": env!("CARGO_PKG_VERSION"),
            "sock": socket_path().display().to_string()
        })
    );
    let _flush_result = stdout.flush();
}

pub fn cleanup_stale_socket(path: &Path) -> Result<bool, String> {
    if !path.exists() {
        return Ok(false);
    }
    fs::remove_file(path).map_err(|err| err.to_string())?;
    Ok(true)
}

fn install_ctrlc_cleanup(path: PathBuf) -> Result<(), String> {
    ctrlc::set_handler(move || {
        if path.exists() {
            let _remove_result = fs::remove_file(&path);
        }
        std::process::exit(0);
    })
    .map_err(|err| err.to_string())
}

fn get_uid() -> u32 {
    unsafe extern "C" {
        fn getuid() -> u32;
    }
    unsafe { getuid() }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::cleanup_stale_socket;

    #[test]
    fn cleanup_stale_socket_removes_file() -> Result<(), Box<dyn std::error::Error>> {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let path = std::env::temp_dir().join(format!(
            "capybara-socket-cleanup-{}-{nanos}.sock",
            std::process::id()
        ));
        std::fs::write(&path, b"stale")?;

        let removed = cleanup_stale_socket(&path)?;

        assert!(removed);
        assert!(!path.exists());
        Ok(())
    }
}
