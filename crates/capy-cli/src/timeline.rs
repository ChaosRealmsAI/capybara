use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::{Args, Subcommand};
use serde::Serialize;
use serde_json::{Value, json};

use capy_contracts::timeline::{
    OP_TIMELINE_ATTACH, OP_TIMELINE_EXPORT_CANCEL, OP_TIMELINE_EXPORT_STATUS, OP_TIMELINE_OPEN,
    OP_TIMELINE_STATE,
};

use crate::ipc_client;

#[derive(Debug, Args)]
pub struct TimelineArgs {
    #[command(subcommand)]
    command: TimelineCommand,
}

#[derive(Debug, Subcommand)]
enum TimelineCommand {
    #[command(about = "Check Timeline binary adapter availability")]
    Doctor(TimelineDoctorArgs),
    #[command(about = "Compose Poster JSON into a Timeline composition project")]
    ComposePoster(TimelineComposePosterArgs),
    #[command(about = "Validate a Timeline composition JSON document")]
    Validate(TimelineValidateArgs),
    #[command(about = "Compile a Timeline composition JSON document")]
    Compile(TimelineCompileArgs),
    #[command(about = "Rebuild a branded Timeline composition when tokens changed")]
    Rebuild(TimelineRebuildArgs),
    #[command(about = "Render a single PNG snapshot from a compiled Timeline composition")]
    Snapshot(TimelineSnapshotArgs),
    #[command(about = "Export MP4 from a compiled Timeline composition")]
    Export(TimelineExportArgs),
    #[command(about = "Run validate, compile, snapshot, export, and write evidence HTML")]
    VerifyExport(TimelineVerifyExportArgs),
    #[command(about = "Attach a Timeline composition to a live canvas node")]
    Attach(TimelineAttachArgs),
    #[command(about = "Read live Timeline attachment state from capy-shell")]
    State(TimelineStateArgs),
    #[command(about = "Read a live Timeline export job from capy-shell")]
    Status(TimelineStatusArgs),
    #[command(about = "Cancel a live Timeline export job tracked by capy-shell")]
    Cancel(TimelineCancelArgs),
    #[command(about = "Open a live Timeline composition preview in the desktop host")]
    Open(TimelineOpenArgs),
}

#[derive(Debug, Args)]
struct TimelineDoctorArgs {
    #[arg(long)]
    recorder: Option<PathBuf>,
    #[arg(long)]
    home: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct TimelineComposePosterArgs {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    brand_tokens: Option<PathBuf>,
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
struct TimelineValidateArgs {
    #[arg(long)]
    composition: PathBuf,
}

#[derive(Debug, Args)]
struct TimelineCompileArgs {
    #[arg(long)]
    composition: PathBuf,
}

#[derive(Debug, Args)]
struct TimelineRebuildArgs {
    #[arg(long)]
    composition: PathBuf,
}

#[derive(Debug, Args)]
struct TimelineSnapshotArgs {
    #[arg(long)]
    composition: PathBuf,
    #[arg(long, default_value_t = 0)]
    frame: u64,
    #[arg(long)]
    out: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct TimelineExportArgs {
    #[arg(long)]
    composition: PathBuf,
    #[arg(long, default_value = "mp4")]
    kind: String,
    #[arg(long)]
    out: Option<PathBuf>,
    #[arg(long, default_value_t = 30)]
    fps: u32,
}

#[derive(Debug, Args)]
struct TimelineVerifyExportArgs {
    #[arg(long)]
    composition: PathBuf,
    #[arg(long)]
    out_html: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct TimelineAttachArgs {
    #[arg(long)]
    canvas_node: u64,
    #[arg(long)]
    composition: PathBuf,
    #[arg(long)]
    socket: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct TimelineStateArgs {
    #[arg(long)]
    canvas_node: Option<u64>,
}

#[derive(Debug, Args)]
struct TimelineStatusArgs {
    #[arg(long)]
    job: String,
}

#[derive(Debug, Args)]
struct TimelineCancelArgs {
    #[arg(long)]
    job: String,
}

#[derive(Debug, Args)]
struct TimelineOpenArgs {
    #[arg(long)]
    canvas_node: u64,
    #[arg(long)]
    socket: Option<PathBuf>,
}

pub fn handle(args: TimelineArgs) -> Result<(), String> {
    match args.command {
        TimelineCommand::Doctor(args) => doctor(args),
        TimelineCommand::ComposePoster(args) => compose_poster(args),
        TimelineCommand::Validate(args) => validate(args),
        TimelineCommand::Compile(args) => compile(args),
        TimelineCommand::Rebuild(args) => rebuild(args),
        TimelineCommand::Snapshot(args) => snapshot(args),
        TimelineCommand::Export(args) => export(args),
        TimelineCommand::VerifyExport(args) => verify_export(args),
        TimelineCommand::Attach(args) => attach(args),
        TimelineCommand::State(args) => state(args),
        TimelineCommand::Status(args) => status(args),
        TimelineCommand::Cancel(args) => cancel(args),
        TimelineCommand::Open(args) => open(args),
    }
}

fn doctor(args: TimelineDoctorArgs) -> Result<(), String> {
    let report = capy_timeline::doctor(capy_timeline::TimelineConfig {
        recorder_bin: args.recorder,
        home: args.home,
    });
    print_json(&report)
}

fn compose_poster(args: TimelineComposePosterArgs) -> Result<(), String> {
    let request = capy_timeline::ComposePosterRequest {
        poster_path: args.input,
        brand_tokens_path: args.brand_tokens,
        project_slug: args.project,
        composition_id: args.composition,
        output_dir: args.out,
        duration_ms: args.duration_ms,
    };
    match capy_timeline::compose_poster(request) {
        Ok(report) => print_json(&report),
        Err(err) => {
            print_json(&capy_timeline::compose::failure(err))?;
            std::process::exit(1);
        }
    }
}

fn rebuild(args: TimelineRebuildArgs) -> Result<(), String> {
    let report = capy_timeline::rebuild(capy_timeline::RebuildRequest {
        composition_path: args.composition,
    });
    print_json(&report)?;
    if report.ok {
        Ok(())
    } else {
        std::process::exit(1);
    }
}

fn validate(args: TimelineValidateArgs) -> Result<(), String> {
    let report = capy_timeline::validate_composition(capy_timeline::ValidateCompositionRequest {
        composition_path: args.composition,
    });
    print_json(&report)?;
    if report.ok {
        Ok(())
    } else {
        std::process::exit(1);
    }
}

fn compile(args: TimelineCompileArgs) -> Result<(), String> {
    let report = capy_timeline::compile_composition(capy_timeline::CompileCompositionRequest {
        composition_path: args.composition,
    });
    print_json(&report)?;
    if report.ok {
        Ok(())
    } else {
        std::process::exit(1);
    }
}

fn snapshot(args: TimelineSnapshotArgs) -> Result<(), String> {
    let report = capy_timeline::snapshot::snapshot(capy_timeline::snapshot::SnapshotRequest {
        composition_path: args.composition,
        frame_ms: args.frame,
        out: args.out,
    });
    print_json(&report)?;
    if report.ok {
        Ok(())
    } else {
        std::process::exit(1);
    }
}

fn export(args: TimelineExportArgs) -> Result<(), String> {
    let kind = match args.kind.as_str() {
        "mp4" => capy_timeline::ExportKind::Mp4,
        _ => {
            let report = export_failure(
                "UNSUPPORTED_EXPORT_KIND",
                format!("unsupported export kind: {}", args.kind),
                "next step · pass --kind mp4",
            );
            print_json(&report)?;
            std::process::exit(1);
        }
    };
    let report = capy_timeline::export_composition(capy_timeline::ExportCompositionRequest {
        composition_path: args.composition,
        kind,
        out: args.out,
        fps: args.fps,
    });
    print_json(&report)?;
    if report.ok {
        Ok(())
    } else {
        std::process::exit(1);
    }
}

fn verify_export(args: TimelineVerifyExportArgs) -> Result<(), String> {
    let report = capy_timeline::verify_export(capy_timeline::VerifyExportRequest {
        composition_path: args.composition,
        out_html: args.out_html,
    });
    print_json(&report)?;
    if report.ok {
        Ok(())
    } else {
        std::process::exit(1);
    }
}

fn attach(args: TimelineAttachArgs) -> Result<(), String> {
    let composition_path = absolute_path(args.composition)?;
    let socket = args.socket.unwrap_or_else(ipc_client::socket_path);
    let request = ipc_client::request(
        OP_TIMELINE_ATTACH,
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
                    .unwrap_or("capy-shell timeline attach failed"),
                response
                    .error
                    .as_ref()
                    .and_then(|error| error.get("hint"))
                    .and_then(Value::as_str)
                    .unwrap_or("next step · run capy timeline attach --help"),
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

fn state(args: TimelineStateArgs) -> Result<(), String> {
    let socket = ipc_client::socket_path();
    let request = ipc_client::request(
        OP_TIMELINE_STATE,
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
                    .unwrap_or("capy-shell timeline state failed"),
                response
                    .error
                    .as_ref()
                    .and_then(|error| error.get("hint"))
                    .and_then(Value::as_str)
                    .unwrap_or("next step · run capy timeline state --help"),
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

fn status(args: TimelineStatusArgs) -> Result<(), String> {
    timeline_job_ipc(
        OP_TIMELINE_EXPORT_STATUS,
        &args.job,
        "status",
        "next step · run capy shell",
    )
}

fn cancel(args: TimelineCancelArgs) -> Result<(), String> {
    timeline_job_ipc(
        OP_TIMELINE_EXPORT_CANCEL,
        &args.job,
        "cancel",
        "next step · run capy shell",
    )
}

fn timeline_job_ipc(op: &str, job_id: &str, stage: &str, shell_hint: &str) -> Result<(), String> {
    let socket = ipc_client::socket_path();
    let request = ipc_client::request(op, json!({ "job_id": job_id }));
    match ipc_client::send_to(request, socket.clone()) {
        Ok(response) if response.ok => print_json(&response.data.unwrap_or(Value::Null)),
        Ok(response) => {
            let report = job_failure(
                stage,
                job_id,
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
                    .unwrap_or("capy-shell timeline export job op failed"),
                response
                    .error
                    .as_ref()
                    .and_then(|error| error.get("hint"))
                    .and_then(Value::as_str)
                    .unwrap_or("next step · run capy timeline status --help"),
            );
            print_json(&report)?;
            std::process::exit(1);
        }
        Err(error) => {
            let report = job_failure(
                stage,
                job_id,
                &socket,
                "SHELL_UNAVAILABLE",
                error,
                shell_hint,
            );
            print_json(&report)?;
            std::process::exit(1);
        }
    }
}

fn open(args: TimelineOpenArgs) -> Result<(), String> {
    let socket = args.socket.unwrap_or_else(ipc_client::socket_path);
    let request = ipc_client::request(
        OP_TIMELINE_OPEN,
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
                    .unwrap_or("capy-shell timeline open failed"),
                response
                    .error
                    .as_ref()
                    .and_then(|error| error.get("hint"))
                    .and_then(Value::as_str)
                    .unwrap_or("next step · run capy timeline open --help"),
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

fn export_failure(code: &str, message: impl Into<String>, hint: &str) -> Value {
    let error = json!({
        "code": code,
        "message": message.into(),
        "hint": hint
    });
    json!({
        "ok": false,
        "trace_id": export_trace_id(),
        "stage": "export",
        "status": "failed",
        "code": code,
        "errors": [error]
    })
}

fn job_failure(
    stage: &str,
    job_id: &str,
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
        "trace_id": job_trace_id(stage),
        "stage": stage,
        "job_id": job_id,
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

fn export_trace_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!("export-{millis}-{}", std::process::id())
}

fn job_trace_id(stage: &str) -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!("{stage}-{millis}-{}", std::process::id())
}
