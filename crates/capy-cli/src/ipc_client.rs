use std::env;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use interprocess::local_socket::tokio::prelude::*;
use interprocess::local_socket::{GenericFilePath, ToFsName};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

pub use capy_contracts::ipc::{IpcRequest, IpcResponse};

const DEFAULT_IPC_TIMEOUT: Duration = Duration::from_secs(60);
const IPC_TIMEOUT_ENV: &str = "CAPY_IPC_TIMEOUT_SECS";
const SOCKET_ENV: &str = "CAPYBARA_SOCKET";
static REQ_COUNTER: AtomicU64 = AtomicU64::new(1);

pub fn request(op: impl Into<String>, params: Value) -> IpcRequest {
    let seq = REQ_COUNTER.fetch_add(1, Ordering::Relaxed);
    IpcRequest::new(format!("{}-{seq}", std::process::id()), op, params)
}

pub fn send(req: IpcRequest) -> Result<IpcResponse, String> {
    send_with_path(req, socket_path())
}

pub fn send_to(req: IpcRequest, path: PathBuf) -> Result<IpcResponse, String> {
    send_with_path(req, path)
}

fn send_with_path(req: IpcRequest, path: PathBuf) -> Result<IpcResponse, String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("socket runtime failed: {err}"))?;

    runtime
        .block_on(async { tokio::time::timeout(ipc_timeout(), send_async(req, path)).await })
        .map_err(|_| format!("IPC request timed out after {}s", ipc_timeout().as_secs()))?
}

fn ipc_timeout() -> Duration {
    env::var(IPC_TIMEOUT_ENV)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|seconds| *seconds > 0)
        .map(Duration::from_secs)
        .unwrap_or(DEFAULT_IPC_TIMEOUT)
}

async fn send_async(req: IpcRequest, path: PathBuf) -> Result<IpcResponse, String> {
    let name = path
        .to_str()
        .ok_or_else(|| format!("socket path is not UTF-8: {path:?}"))?
        .to_fs_name::<GenericFilePath>()
        .map_err(|err| err.to_string())?;

    let conn = interprocess::local_socket::tokio::Stream::connect(name)
        .await
        .map_err(|err| format!("socket failed: {err} · next step · run `capy shell`"))?;
    let (read_half, mut write_half) = conn.split();
    let mut payload = serde_json::to_string(&req).map_err(|err| err.to_string())?;
    payload.push('\n');

    write_half
        .write_all(payload.as_bytes())
        .await
        .map_err(|err| err.to_string())?;
    write_half.flush().await.map_err(|err| err.to_string())?;

    let mut reader = BufReader::new(read_half);
    let mut line = String::new();
    let bytes = reader
        .read_line(&mut line)
        .await
        .map_err(|err| err.to_string())?;
    if bytes == 0 {
        return Err("IPC server closed before sending a response".to_string());
    }

    let resp: IpcResponse = serde_json::from_str(line.trim_end()).map_err(|err| err.to_string())?;
    if resp.req_id != req.req_id {
        return Err(format!(
            "IPC req_id mismatch: sent {}, received {}",
            req.req_id, resp.req_id
        ));
    }
    Ok(resp)
}

pub fn socket_path() -> PathBuf {
    if let Some(path) = std::env::var_os(SOCKET_ENV).filter(|value| !value.is_empty()) {
        return PathBuf::from(path);
    }
    let uid = get_uid();
    PathBuf::from(format!("/tmp/capybara-{uid}.sock"))
}

fn get_uid() -> u32 {
    unsafe extern "C" {
        fn getuid() -> u32;
    }
    unsafe { getuid() }
}
