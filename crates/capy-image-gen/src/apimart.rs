use std::env;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{Value, json};

use crate::types::{
    DoctorReport, GenerateImageRequest, ImageGenerateMode, ImageProviderId, ProviderInfo,
};
use crate::{ImageGenError, Result};

const DEFAULT_PROVIDER_ROOT: &str = "/Users/Zhuanz/workspace/apimart-image-gen";

pub fn provider_info() -> ProviderInfo {
    ProviderInfo {
        id: ImageProviderId::ApimartGptImage2,
        kind: "provider-adapter".to_string(),
        label: ImageProviderId::ApimartGptImage2.label().to_string(),
        model: ImageProviderId::ApimartGptImage2.model().to_string(),
        live_generation_requires_explicit_command: true,
        default_no_spend_gate: true,
    }
}

pub fn doctor() -> DoctorReport {
    let node = node_check();
    let bridge = path_check("bridge", &bridge_path());
    let root_path = provider_root();
    let provider_root = path_check("provider_root", &root_path);
    let provider_module = path_check("provider_module", &root_path.join("scripts/apimart.mjs"));
    let key_configured = env::var("APIMART_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .is_some()
        || root_path.join(".env").is_file();
    let ok = node.get("ok").and_then(Value::as_bool) == Some(true)
        && bridge.get("ok").and_then(Value::as_bool) == Some(true)
        && provider_root.get("ok").and_then(Value::as_bool) == Some(true)
        && provider_module.get("ok").and_then(Value::as_bool) == Some(true);
    DoctorReport {
        ok,
        provider: ImageProviderId::ApimartGptImage2,
        model: ImageProviderId::ApimartGptImage2.model().to_string(),
        checks: json!({
            "node": node,
            "bridge": bridge,
            "provider_root": provider_root,
            "provider_module": provider_module,
            "key_configured_hint": key_configured
        }),
    }
}

pub fn balance() -> Result<Value> {
    run_bridge("balance", json!({}))
}

pub fn generate(request: GenerateImageRequest) -> Result<Value> {
    let operation = match request.mode {
        ImageGenerateMode::SubmitOnly => "submit",
        ImageGenerateMode::Generate => "generate",
        ImageGenerateMode::Resume => "resume",
        ImageGenerateMode::DryRun => {
            return Err(ImageGenError::Message(
                "dry-run is handled by capy-image-gen without provider calls".to_string(),
            ));
        }
    };
    run_bridge(operation, bridge_request(&request))
}

fn bridge_request(request: &GenerateImageRequest) -> Value {
    json!({
        "provider": request.provider.as_str(),
        "prompt": request.prompt,
        "size": request.size,
        "resolution": request.resolution,
        "refs": request.refs,
        "out": request.output_dir.as_ref().map(|path| path.display().to_string()),
        "name": request.name,
        "download": request.download,
        "task_id": request.task_id
    })
}

fn run_bridge(operation: &str, input: Value) -> Result<Value> {
    let bridge = bridge_path();
    if !bridge.is_file() {
        return Err(ImageGenError::Message(format!(
            "image provider bridge missing: {}",
            bridge.display()
        )));
    }
    let mut child = Command::new(node_program())
        .arg(bridge)
        .arg(operation)
        .env("CAPY_IMAGE_GEN_APIMART_ROOT", provider_root())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| {
            ImageGenError::Message(format!("spawn image provider bridge failed: {err}"))
        })?;

    let input_text = serde_json::to_string(&input)
        .map_err(|err| ImageGenError::Message(format!("serialize bridge request failed: {err}")))?;
    let Some(stdin) = child.stdin.as_mut() else {
        return Err(ImageGenError::Message(
            "image provider bridge stdin unavailable".to_string(),
        ));
    };
    stdin
        .write_all(input_text.as_bytes())
        .map_err(|err| ImageGenError::Message(format!("write bridge request failed: {err}")))?;
    drop(child.stdin.take());

    let output = child
        .wait_with_output()
        .map_err(|err| ImageGenError::Message(format!("wait for bridge failed: {err}")))?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !output.status.success() {
        return Err(ImageGenError::Message(format!(
            "image provider bridge failed: {}",
            if stderr.is_empty() { stdout } else { stderr }
        )));
    }
    serde_json::from_str(&stdout).map_err(|err| {
        ImageGenError::Message(format!("parse bridge JSON failed: {err}; stdout={stdout}"))
    })
}

fn node_check() -> Value {
    match Command::new(node_program()).arg("--version").output() {
        Ok(output) => json!({
            "ok": output.status.success(),
            "program": node_program(),
            "version": String::from_utf8_lossy(&output.stdout).trim(),
            "error": String::from_utf8_lossy(&output.stderr).trim()
        }),
        Err(err) => json!({
            "ok": false,
            "program": node_program(),
            "error": err.to_string()
        }),
    }
}

fn path_check(kind: &str, path: &Path) -> Value {
    json!({
        "ok": path.exists(),
        "kind": kind,
        "path": path.display().to_string(),
        "is_file": path.is_file(),
        "is_dir": path.is_dir()
    })
}

fn node_program() -> String {
    env::var("CAPY_IMAGE_GEN_NODE")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "node".to_string())
}

fn provider_root() -> PathBuf {
    env::var_os("CAPY_IMAGE_GEN_APIMART_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_PROVIDER_ROOT))
}

fn bridge_path() -> PathBuf {
    env::var_os("CAPY_IMAGE_PROVIDER_APIMART_BRIDGE")
        .map(PathBuf::from)
        .unwrap_or_else(default_bridge_path)
}

fn default_bridge_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(|root| root.join("scripts/image-provider-apimart.mjs"))
        .unwrap_or_else(|| PathBuf::from("scripts/image-provider-apimart.mjs"))
}
