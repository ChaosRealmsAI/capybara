use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::{Args, Subcommand};
use serde_json::{Value, json};

use crate::canvas_context::{self, CanvasContextArgs, CanvasContextCommand};
use crate::ipc_client;

mod image_tool;
mod poster;

#[derive(Debug, Args)]
pub struct CanvasArgs {
    #[command(subcommand)]
    command: CanvasCommand,
}

#[derive(Debug, Subcommand)]
enum CanvasCommand {
    #[command(about = "Read an AI-facing snapshot from the live canvas")]
    Snapshot(CanvasWindowArgs),
    #[command(about = "Select one live canvas node by stable id")]
    Select(CanvasSelectArgs),
    #[command(about = "Move one live canvas node by stable id")]
    Move(CanvasMoveArgs),
    #[command(about = "Create a semantic content card on the live canvas")]
    CreateCard(CanvasCreateCardArgs),
    #[command(about = "Load a poster JSON document onto the live canvas")]
    LoadPoster(poster::CanvasLoadPosterArgs),
    #[command(about = "Insert a local image file into the live canvas")]
    InsertImage(image_tool::CanvasInsertImageArgs),
    #[command(about = "Generate an image and insert it into the live canvas")]
    GenerateImage(image_tool::CanvasGenerateImageArgs),
    #[command(about = "Export AI-readable selected-image or region context packets")]
    Context(CanvasContextArgs),
}

#[derive(Debug, Args)]
struct CanvasWindowArgs {
    #[arg(long)]
    window: Option<String>,
}

#[derive(Debug, Args)]
struct CanvasSelectArgs {
    #[arg(long)]
    id: u64,
    #[arg(long)]
    window: Option<String>,
}

#[derive(Debug, Args)]
struct CanvasMoveArgs {
    #[arg(long)]
    id: u64,
    #[arg(long)]
    x: f64,
    #[arg(long)]
    y: f64,
    #[arg(long)]
    window: Option<String>,
}

#[derive(Debug, Args)]
struct CanvasCreateCardArgs {
    #[arg(long)]
    kind: String,
    #[arg(long)]
    title: String,
    #[arg(long)]
    x: f64,
    #[arg(long)]
    y: f64,
    #[arg(long)]
    window: Option<String>,
}

pub fn handle(args: CanvasArgs) -> Result<(), String> {
    let command_name = args.command.name();
    let result = match args.command {
        CanvasCommand::Snapshot(args) => snapshot(args.window),
        CanvasCommand::Select(args) => canvas_eval(
            &format!("window.capyWorkbench.selectNode({})", args.id),
            args.window,
        ),
        CanvasCommand::Move(args) => canvas_eval(
            &format!(
                "window.capyWorkbench.moveNodeById({}, {}, {})",
                args.id, args.x, args.y
            ),
            args.window,
        ),
        CanvasCommand::CreateCard(args) => canvas_eval(
            &format!(
                "window.capyWorkbench.createContentCard({}, {}, {}, {})",
                js_string(&args.kind),
                js_string(&args.title),
                args.x,
                args.y
            ),
            args.window,
        ),
        CanvasCommand::LoadPoster(args) => poster::load_poster(args),
        CanvasCommand::InsertImage(args) => image_tool::insert_image(args),
        CanvasCommand::GenerateImage(args) => image_tool::generate_image(args),
        CanvasCommand::Context(args) => match args.command {
            CanvasContextCommand::Export(args) => canvas_context::export_context(args),
        },
    };
    append_tool_call_log(command_name, &result);
    let data = result?;
    print_json(&data)
}

impl CanvasCommand {
    fn name(&self) -> &'static str {
        match self {
            Self::Snapshot(_) => "snapshot",
            Self::Select(_) => "select",
            Self::Move(_) => "move",
            Self::CreateCard(_) => "create-card",
            Self::LoadPoster(_) => "load-poster",
            Self::InsertImage(_) => "insert-image",
            Self::GenerateImage(_) => "generate-image",
            Self::Context(CanvasContextArgs {
                command: CanvasContextCommand::Export(_),
            }) => "context-export",
        }
    }
}

pub(crate) fn snapshot(window: Option<String>) -> Result<Value, String> {
    canvas_eval(
        "window.capyWorkbench.refreshPlannerContext && window.capyWorkbench.refreshPlannerContext()",
        window,
    )
}

pub(crate) fn canvas_eval(script: &str, window: Option<String>) -> Result<Value, String> {
    request_data(
        "devtools-eval",
        json!({
            "eval": script,
            "window": window
        }),
    )
}

pub(crate) fn request_data(op: &str, params: Value) -> Result<Value, String> {
    let request = ipc_client::request(op, params);
    let response = ipc_client::send(request)?;
    if response.ok {
        return Ok(response.data.unwrap_or(Value::Null));
    }
    Err(response
        .error
        .map(|value| value.to_string())
        .unwrap_or_else(|| "capy IPC request failed".to_string()))
}

fn placement(x: Option<f64>, y: Option<f64>, snapshot: &Value) -> (f64, f64) {
    if let (Some(x), Some(y)) = (x, y) {
        return (x, y);
    }
    let selected_id = snapshot.get("selectedId").and_then(Value::as_u64);
    let selected = snapshot
        .get("blocks")
        .and_then(Value::as_array)
        .and_then(|nodes| {
            selected_id.and_then(|id| {
                nodes
                    .iter()
                    .find(|node| node.get("id").and_then(Value::as_u64) == Some(id))
            })
        });
    let fallback_x = 360.0;
    let fallback_y = 140.0;
    let next_x = selected
        .and_then(|node| node.get("bounds"))
        .and_then(|bounds| Some(bounds.get("x")?.as_f64()? + bounds.get("w")?.as_f64()? + 48.0))
        .unwrap_or(fallback_x);
    let next_y = selected
        .and_then(|node| node.get("bounds"))
        .and_then(|bounds| bounds.get("y")?.as_f64())
        .unwrap_or(fallback_y);
    (x.unwrap_or(next_x), y.unwrap_or(next_y))
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

pub(crate) fn absolute_path(path: PathBuf) -> Result<PathBuf, String> {
    if path.is_absolute() {
        return Ok(path);
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(path))
        .map_err(|err| format!("read cwd failed: {err}"))
}

fn js_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

fn js_value(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "null".to_string())
}

fn print_json(data: &Value) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(data).map_err(|err| err.to_string())?
    );
    Ok(())
}

fn append_tool_call_log(command_name: &str, result: &Result<Value, String>) {
    let Some(path) = std::env::var_os("CAPY_TOOL_CALL_LOG").map(PathBuf::from) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _create_result = fs::create_dir_all(parent);
    }
    let timestamp_ms = now_ms();
    let entry = match result {
        Ok(data) => json!({
            "timestamp_ms": timestamp_ms,
            "tool": "capy canvas",
            "command": command_name,
            "argv": std::env::args().collect::<Vec<_>>(),
            "cwd": std::env::current_dir().ok().map(|path| path.display().to_string()),
            "socket": std::env::var("CAPYBARA_SOCKET").ok(),
            "ok": true,
            "result": data
        }),
        Err(error) => json!({
            "timestamp_ms": timestamp_ms,
            "tool": "capy canvas",
            "command": command_name,
            "argv": std::env::args().collect::<Vec<_>>(),
            "cwd": std::env::current_dir().ok().map(|path| path.display().to_string()),
            "socket": std::env::var("CAPYBARA_SOCKET").ok(),
            "ok": false,
            "error": error
        }),
    };
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) else {
        return;
    };
    if let Ok(line) = serde_json::to_string(&entry) {
        let _write_result = writeln!(file, "{line}");
    }
}
