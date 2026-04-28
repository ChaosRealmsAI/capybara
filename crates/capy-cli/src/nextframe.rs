use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::{Args, Subcommand};
use serde::Serialize;
use serde_json::{Value, json};

use crate::ipc_client;

#[derive(Debug, Args)]
pub struct NextFrameArgs {
    #[command(subcommand)]
    command: NextFrameCommand,
}

#[derive(Debug, Subcommand)]
enum NextFrameCommand {
    #[command(about = "Check NextFrame binary adapter availability")]
    Doctor(NextFrameDoctorArgs),
    #[command(about = "Compose Poster JSON into a NextFrame composition project")]
    ComposePoster(NextFrameComposePosterArgs),
    #[command(about = "Validate a NextFrame composition JSON document")]
    Validate(NextFrameValidateArgs),
    #[command(about = "Compile a NextFrame composition JSON document")]
    Compile(NextFrameCompileArgs),
    #[command(about = "Render a single PNG snapshot from a compiled NextFrame composition")]
    Snapshot(NextFrameSnapshotArgs),
    #[command(about = "Attach a NextFrame composition to a live canvas node")]
    Attach(NextFrameAttachArgs),
    #[command(about = "Read live NextFrame attachment state from capy-shell")]
    State(NextFrameStateArgs),
    #[command(about = "Open a live NextFrame composition preview in the desktop host")]
    Open(NextFrameOpenArgs),
}

#[derive(Debug, Args)]
struct NextFrameDoctorArgs {
    #[arg(long)]
    nf: Option<PathBuf>,
    #[arg(long, alias = "nf-recorder")]
    recorder: Option<PathBuf>,
    #[arg(long)]
    home: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct NextFrameComposePosterArgs {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    out: Option<PathBuf>,
    #[arg(long)]
    project: Option<String>,
    #[arg(long)]
    composition: Option<String>,
    #[arg(long, default_value_t = 1000)]
    duration_ms: u64,
}

#[derive(Debug, Args)]
struct NextFrameValidateArgs {
    #[arg(long)]
    composition: PathBuf,
    #[arg(long)]
    strict_binary: bool,
}

#[derive(Debug, Args)]
struct NextFrameCompileArgs {
    #[arg(long)]
    composition: PathBuf,
    #[arg(long)]
    strict_binary: bool,
}

#[derive(Debug, Args)]
struct NextFrameSnapshotArgs {
    #[arg(long)]
    composition: PathBuf,
    #[arg(long, default_value_t = 0)]
    frame: u64,
    #[arg(long)]
    out: Option<PathBuf>,
    #[arg(long)]
    strict_binary: bool,
}

#[derive(Debug, Args)]
struct NextFrameAttachArgs {
    #[arg(long)]
    canvas_node: u64,
    #[arg(long)]
    composition: PathBuf,
    #[arg(long)]
    socket: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct NextFrameStateArgs {
    #[arg(long)]
    canvas_node: Option<u64>,
}

#[derive(Debug, Args)]
struct NextFrameOpenArgs {
    #[arg(long)]
    canvas_node: u64,
    #[arg(long)]
    socket: Option<PathBuf>,
}

pub fn handle(args: NextFrameArgs) -> Result<(), String> {
    match args.command {
        NextFrameCommand::Doctor(args) => doctor(args),
        NextFrameCommand::ComposePoster(args) => compose_poster(args),
        NextFrameCommand::Validate(args) => validate(args),
        NextFrameCommand::Compile(args) => compile(args),
        NextFrameCommand::Snapshot(args) => snapshot(args),
        NextFrameCommand::Attach(args) => attach(args),
        NextFrameCommand::State(args) => state(args),
        NextFrameCommand::Open(args) => open(args),
    }
}

fn doctor(args: NextFrameDoctorArgs) -> Result<(), String> {
    let report = capy_nextframe::doctor(capy_nextframe::NextFrameConfig {
        nf_bin: args.nf,
        recorder_bin: args.recorder,
        home: args.home,
    });
    print_json(&report)
}

fn compose_poster(args: NextFrameComposePosterArgs) -> Result<(), String> {
    let request = capy_nextframe::ComposePosterRequest {
        poster_path: args.input,
        project_slug: args.project,
        composition_id: args.composition,
        output_dir: args.out,
        duration_ms: args.duration_ms,
    };
    match capy_nextframe::compose_poster(request) {
        Ok(report) => print_json(&report),
        Err(err) => {
            print_json(&capy_nextframe::compose::failure(err))?;
            std::process::exit(1);
        }
    }
}

fn validate(args: NextFrameValidateArgs) -> Result<(), String> {
    let report = capy_nextframe::validate_composition(capy_nextframe::ValidateCompositionRequest {
        composition_path: args.composition,
        strict_binary: args.strict_binary,
    });
    print_json(&report)?;
    if report.ok {
        Ok(())
    } else {
        std::process::exit(1);
    }
}

fn compile(args: NextFrameCompileArgs) -> Result<(), String> {
    let report = capy_nextframe::compile_composition(capy_nextframe::CompileCompositionRequest {
        composition_path: args.composition,
        strict_binary: args.strict_binary,
    });
    print_json(&report)?;
    if report.ok {
        Ok(())
    } else {
        std::process::exit(1);
    }
}

fn snapshot(args: NextFrameSnapshotArgs) -> Result<(), String> {
    let report = capy_nextframe::snapshot::snapshot(capy_nextframe::snapshot::SnapshotRequest {
        composition_path: args.composition,
        frame_ms: args.frame,
        out: args.out,
        strict_binary: args.strict_binary,
    });
    print_json(&report)?;
    if report.ok {
        Ok(())
    } else {
        std::process::exit(1);
    }
}

fn attach(args: NextFrameAttachArgs) -> Result<(), String> {
    let composition_path = absolute_path(args.composition)?;
    let socket = args.socket.unwrap_or_else(ipc_client::socket_path);
    let request = ipc_client::request(
        "nextframe-attach",
        json!({
            "canvas_node_id": args.canvas_node,
            "composition_path": composition_path.display().to_string()
        }),
    );
    match ipc_client::send_to(request, socket.clone()) {
        Ok(response) if response.ok => {
            let mut report = response.data.unwrap_or(Value::Null);
            report["ipc_socket"] = json!(socket.display().to_string());
            print_json(&report)
        }
        Ok(response) => {
            let report = attach_failure(
                args.canvas_node,
                &composition_path,
                &socket,
                response
                    .error
                    .as_ref()
                    .and_then(|error| error.get("code"))
                    .and_then(Value::as_str)
                    .unwrap_or("IPC_ERROR"),
                response
                    .error
                    .as_ref()
                    .and_then(|error| error.get("message"))
                    .and_then(Value::as_str)
                    .unwrap_or("capy-shell nextframe attach failed"),
                response
                    .error
                    .as_ref()
                    .and_then(|error| error.get("hint"))
                    .and_then(Value::as_str)
                    .unwrap_or("next step · run capy nextframe attach --help"),
            );
            print_json(&report)?;
            std::process::exit(1);
        }
        Err(error) => {
            let report = attach_failure(
                args.canvas_node,
                &composition_path,
                &socket,
                "SHELL_UNAVAILABLE",
                error,
                "next step · run capy shell",
            );
            print_json(&report)?;
            std::process::exit(1);
        }
    }
}

fn state(args: NextFrameStateArgs) -> Result<(), String> {
    let socket = ipc_client::socket_path();
    let request = ipc_client::request(
        "nextframe-state",
        json!({
            "canvas_node_id": args.canvas_node
        }),
    );
    match ipc_client::send_to(request, socket) {
        Ok(response) if response.ok => print_json(&response.data.unwrap_or(Value::Null)),
        Ok(response) => {
            let report = state_failure(
                response
                    .error
                    .as_ref()
                    .and_then(|error| error.get("code"))
                    .and_then(Value::as_str)
                    .unwrap_or("IPC_ERROR"),
                response
                    .error
                    .as_ref()
                    .and_then(|error| error.get("message"))
                    .and_then(Value::as_str)
                    .unwrap_or("capy-shell nextframe state failed"),
                response
                    .error
                    .as_ref()
                    .and_then(|error| error.get("hint"))
                    .and_then(Value::as_str)
                    .unwrap_or("next step · run capy nextframe state --help"),
            );
            print_json(&report)?;
            std::process::exit(1);
        }
        Err(error) => {
            let report = state_failure("SHELL_UNAVAILABLE", error, "next step · run capy shell");
            print_json(&report)?;
            std::process::exit(1);
        }
    }
}

fn open(args: NextFrameOpenArgs) -> Result<(), String> {
    let socket = args.socket.unwrap_or_else(ipc_client::socket_path);
    let request = ipc_client::request(
        "nextframe-open",
        json!({
            "canvas_node_id": args.canvas_node
        }),
    );
    match ipc_client::send_to(request, socket.clone()) {
        Ok(response) if response.ok => {
            let mut report = response.data.unwrap_or(Value::Null);
            report["ipc_socket"] = json!(socket.display().to_string());
            print_json(&report)
        }
        Ok(response) => {
            let report = open_failure(
                args.canvas_node,
                &socket,
                response
                    .error
                    .as_ref()
                    .and_then(|error| error.get("code"))
                    .and_then(Value::as_str)
                    .unwrap_or("IPC_ERROR"),
                response
                    .error
                    .as_ref()
                    .and_then(|error| error.get("message"))
                    .and_then(Value::as_str)
                    .unwrap_or("capy-shell nextframe open failed"),
                response
                    .error
                    .as_ref()
                    .and_then(|error| error.get("hint"))
                    .and_then(Value::as_str)
                    .unwrap_or("next step · run capy nextframe open --help"),
            );
            print_json(&report)?;
            std::process::exit(1);
        }
        Err(error) => {
            let report = open_failure(
                args.canvas_node,
                &socket,
                "SHELL_UNAVAILABLE",
                error,
                "next step · run capy shell",
            );
            print_json(&report)?;
            std::process::exit(1);
        }
    }
}

fn print_json<T: Serialize>(data: &T) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(data).map_err(|err| err.to_string())?
    );
    Ok(())
}

fn state_failure(code: &str, message: impl Into<String>, hint: &str) -> Value {
    let error = json!({
        "code": code,
        "message": message.into(),
        "hint": hint
    });
    json!({
        "ok": false,
        "trace_id": state_trace_id(),
        "stage": "state",
        "code": code,
        "errors": [error]
    })
}

fn attach_failure(
    canvas_node_id: u64,
    composition_path: &std::path::Path,
    socket: &std::path::Path,
    code: &str,
    message: impl Into<String>,
    hint: &str,
) -> Value {
    let error = json!({
        "code": code,
        "message": message.into(),
        "hint": hint
    });
    json!({
        "ok": false,
        "trace_id": trace_id(),
        "stage": "attach",
        "canvas_node_id": canvas_node_id,
        "composition_path": composition_path.display().to_string(),
        "node_state": "error",
        "ipc_socket": socket.display().to_string(),
        "code": code,
        "errors": [error]
    })
}

fn open_failure(
    canvas_node_id: u64,
    socket: &std::path::Path,
    code: &str,
    message: impl Into<String>,
    hint: &str,
) -> Value {
    let error = json!({
        "code": code,
        "message": message.into(),
        "hint": hint
    });
    json!({
        "ok": false,
        "trace_id": open_trace_id(),
        "stage": "open",
        "canvas_node_id": canvas_node_id,
        "ipc_socket": socket.display().to_string(),
        "code": code,
        "errors": [error]
    })
}

fn absolute_path(path: PathBuf) -> Result<PathBuf, String> {
    if path.is_absolute() {
        return Ok(path);
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(path))
        .map_err(|err| format!("read cwd failed: {err}"))
}

fn trace_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!("attach-{millis}-{}", std::process::id())
}

fn state_trace_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!("state-{millis}-{}", std::process::id())
}

fn open_trace_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!("open-{millis}-{}", std::process::id())
}
