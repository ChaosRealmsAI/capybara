use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentRuntimeError {
    #[error("{0}")]
    Invalid(String),
    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{context}: {source}")]
    Json {
        context: String,
        #[source]
        source: serde_json::Error,
    },
}

pub type AgentRuntimeResult<T> = Result<T, AgentRuntimeError>;

#[derive(Debug, Clone)]
pub struct AgentSdkRunRequest {
    pub provider: String,
    pub cwd: PathBuf,
    pub prompt: String,
    pub output_schema: Value,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub fake_response: Option<PathBuf>,
}

pub fn run_sdk_json(request: AgentSdkRunRequest) -> AgentRuntimeResult<Value> {
    if let Some(path) = request.fake_response.as_ref() {
        return read_json_fixture(path);
    }
    validate_provider(&request.provider)?;
    let output = Command::new(node_bin_path())
        .arg(sdk_script_path())
        .arg("run")
        .arg("--provider")
        .arg(&request.provider)
        .arg("--cwd")
        .arg(&request.cwd)
        .arg("--prompt")
        .arg(&request.prompt)
        .arg("--json")
        .arg("--output-schema")
        .arg(
            serde_json::to_string(&request.output_schema).map_err(|source| {
                AgentRuntimeError::Json {
                    context: "serialize SDK output schema".to_string(),
                    source,
                }
            })?,
        )
        .args(optional_pair("--model", request.model))
        .args(optional_pair(
            "--effort",
            request.effort.or_else(|| Some("low".to_string())),
        ))
        .current_dir(repo_root())
        .output()
        .map_err(|source| AgentRuntimeError::Io {
            context: "start Capybara Agent SDK bridge".to_string(),
            source,
        })?;
    if !output.status.success() {
        return Err(AgentRuntimeError::Invalid(format!(
            "Capybara Agent SDK bridge exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    serde_json::from_slice(&output.stdout).map_err(|source| AgentRuntimeError::Json {
        context: "parse Capybara Agent SDK JSON output".to_string(),
        source,
    })
}

fn read_json_fixture(path: &Path) -> AgentRuntimeResult<Value> {
    let raw = fs::read_to_string(path).map_err(|source| AgentRuntimeError::Io {
        context: format!("read SDK response fixture {}", path.display()),
        source,
    })?;
    serde_json::from_str(&raw).map_err(|source| AgentRuntimeError::Json {
        context: format!("parse SDK response fixture {}", path.display()),
        source,
    })
}

fn validate_provider(provider: &str) -> AgentRuntimeResult<()> {
    match provider {
        "codex" | "claude" => Ok(()),
        other => Err(AgentRuntimeError::Invalid(format!(
            "project AI provider must be codex or claude, got {other}"
        ))),
    }
}

fn optional_pair(key: &'static str, value: Option<String>) -> Vec<String> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(|value| vec![key.to_string(), value])
        .unwrap_or_default()
}

fn sdk_script_path() -> PathBuf {
    std::env::var_os("CAPY_AGENT_SDK_CLI")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root().join("tools/capy-agent-sdk/src/cli.mjs"))
}

fn node_bin_path() -> PathBuf {
    if let Some(path) = std::env::var_os("CAPY_AGENT_NODE_BIN") {
        return PathBuf::from(path);
    }
    for candidate in [
        "/opt/homebrew/bin/node",
        "/usr/local/bin/node",
        "/usr/bin/node",
    ] {
        let path = PathBuf::from(candidate);
        if path.exists() {
            return path;
        }
    }
    PathBuf::from("node")
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}
