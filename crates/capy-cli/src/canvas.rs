use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use clap::{Args, Subcommand, ValueEnum};
use image::ImageEncoder;
use serde_json::{Value, json};

use crate::ipc_client;

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
    LoadPoster(CanvasLoadPosterArgs),
    #[command(about = "Insert a local image file into the live canvas")]
    InsertImage(CanvasInsertImageArgs),
    #[command(about = "Generate an image and insert it into the live canvas")]
    GenerateImage(CanvasGenerateImageArgs),
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

#[derive(Debug, Args)]
struct CanvasLoadPosterArgs {
    #[arg(long)]
    path: PathBuf,
    #[arg(long)]
    x: Option<f64>,
    #[arg(long)]
    y: Option<f64>,
    #[arg(long)]
    title: Option<String>,
    #[arg(long)]
    window: Option<String>,
}

#[derive(Debug, Args)]
struct CanvasInsertImageArgs {
    #[arg(long)]
    path: PathBuf,
    #[arg(long)]
    x: Option<f64>,
    #[arg(long)]
    y: Option<f64>,
    #[arg(long)]
    title: Option<String>,
    #[arg(long)]
    provider: Option<String>,
    #[arg(long)]
    prompt_summary: Option<String>,
    #[arg(long)]
    window: Option<String>,
}

#[derive(Debug, Args)]
struct CanvasGenerateImageArgs {
    #[arg(long, value_enum, default_value = "apimart-gpt-image-2")]
    provider: CanvasImageProviderArg,
    #[arg(long, default_value = "1:1")]
    size: String,
    #[arg(long, alias = "aspect-ratio")]
    aspect_ratio: Option<String>,
    #[arg(long, default_value = "1k")]
    resolution: String,
    #[arg(long = "ref")]
    refs: Vec<String>,
    #[arg(long, default_value = "tmp/capy-canvas-image-tool")]
    out: PathBuf,
    #[arg(long)]
    name: Option<String>,
    #[arg(
        long,
        help = "Validate and insert a local fixture without provider spend"
    )]
    dry_run: bool,
    #[arg(long, help = "Make the live provider call")]
    live: bool,
    #[arg(long)]
    x: Option<f64>,
    #[arg(long)]
    y: Option<f64>,
    #[arg(long)]
    title: Option<String>,
    #[arg(long)]
    window: Option<String>,
    #[arg(required = true)]
    prompt: Vec<String>,
}

#[derive(Debug, Clone, ValueEnum)]
enum CanvasImageProviderArg {
    #[value(name = "apimart-gpt-image-2")]
    ApimartGptImage2,
}

impl CanvasImageProviderArg {
    fn id(&self) -> capy_image_gen::ImageProviderId {
        match self {
            Self::ApimartGptImage2 => capy_image_gen::ImageProviderId::ApimartGptImage2,
        }
    }
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
        CanvasCommand::LoadPoster(args) => load_poster(args),
        CanvasCommand::InsertImage(args) => insert_image(InsertImageRequest {
            path: args.path,
            x: args.x,
            y: args.y,
            title: args.title,
            provider: args.provider,
            prompt_summary: args.prompt_summary,
            window: args.window,
        }),
        CanvasCommand::GenerateImage(args) => generate_image(args),
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
        }
    }
}

fn load_poster(args: CanvasLoadPosterArgs) -> Result<Value, String> {
    let path = absolute_path(args.path)?;
    let source =
        fs::read_to_string(&path).map_err(|err| format!("read poster JSON failed: {err}"))?;
    let document: Value =
        serde_json::from_str(&source).map_err(|err| format!("parse poster JSON failed: {err}"))?;
    validate_poster_document(&document)?;
    let snapshot = snapshot(args.window.clone()).unwrap_or_else(|_| json!({}));
    let (x, y) = placement(args.x, args.y, &snapshot);
    let title = args
        .title
        .or_else(|| poster_document_title(&document))
        .unwrap_or_else(|| {
            path.file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or("Poster document")
                .to_string()
        });
    let script = format!(
        "window.capyWorkbench.loadPosterDocument({}, {})",
        js_value(&document),
        js_value(&json!({
            "title": title,
            "x": x,
            "y": y,
            "sourcePath": path.display().to_string()
        }))
    );
    canvas_eval(&script, args.window)
}

fn validate_poster_document(document: &Value) -> Result<(), String> {
    if document.get("type").and_then(Value::as_str) != Some("poster") {
        return Err("poster document type must be \"poster\"".to_string());
    }
    let canvas = document
        .get("canvas")
        .ok_or_else(|| "poster document requires canvas".to_string())?;
    if canvas.get("width").and_then(Value::as_f64).unwrap_or(0.0) <= 0.0
        || canvas.get("height").and_then(Value::as_f64).unwrap_or(0.0) <= 0.0
    {
        return Err("poster canvas width and height must be positive".to_string());
    }
    let layers = document
        .get("layers")
        .and_then(Value::as_array)
        .ok_or_else(|| "poster document requires layers[]".to_string())?;
    if layers.is_empty() {
        return Err("poster document requires at least one layer".to_string());
    }
    let assets = document
        .get("assets")
        .and_then(Value::as_object)
        .ok_or_else(|| "poster document requires assets object".to_string())?;
    for layer in layers {
        let id = layer
            .get("id")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "each poster layer requires id".to_string())?;
        let kind = layer
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("poster layer {id} requires type"))?;
        if kind == "image" {
            let asset_id = layer
                .get("assetId")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("poster image layer {id} requires assetId"))?;
            if !assets.contains_key(asset_id) {
                return Err(format!(
                    "poster image layer {id} references missing asset {asset_id}"
                ));
            }
        }
    }
    Ok(())
}

fn poster_document_title(document: &Value) -> Option<String> {
    document
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

struct InsertImageRequest {
    path: PathBuf,
    x: Option<f64>,
    y: Option<f64>,
    title: Option<String>,
    provider: Option<String>,
    prompt_summary: Option<String>,
    window: Option<String>,
}

fn generate_image(args: CanvasGenerateImageArgs) -> Result<Value, String> {
    if args.dry_run && args.live {
        return Err("--dry-run and --live cannot be used together".to_string());
    }
    let live = args.live;
    let prompt = args.prompt.join(" ");
    let provider = args.provider.id();
    let name = args.name.unwrap_or_else(|| "canvas-image".to_string());
    let out_dir = absolute_path(args.out)?;
    let mode = if live {
        capy_image_gen::ImageGenerateMode::Generate
    } else {
        capy_image_gen::ImageGenerateMode::DryRun
    };
    let request = capy_image_gen::GenerateImageRequest {
        provider,
        mode,
        prompt: Some(prompt.clone()),
        size: args.aspect_ratio.unwrap_or(args.size),
        resolution: args.resolution,
        refs: args.refs,
        output_dir: Some(out_dir.clone()),
        name: Some(name.clone()),
        download: true,
        task_id: None,
    };
    let generation = capy_image_gen::generate_image(request).map_err(|err| err.to_string())?;
    let image_path = if live {
        capy_image_gen::find_downloaded_image_path(&generation).ok_or_else(|| {
            "live generation did not report an existing downloaded image path".to_string()
        })?
    } else {
        write_fixture_png(&out_dir, &name)?
    };
    let inserted = insert_image(InsertImageRequest {
        path: image_path.clone(),
        x: args.x,
        y: args.y,
        title: args.title.or_else(|| Some("Generated image".to_string())),
        provider: Some(provider.as_str().to_string()),
        prompt_summary: Some(prompt_summary(&prompt)),
        window: args.window,
    })?;
    Ok(json!({
        "ok": true,
        "kind": "canvas-image-generation",
        "mode": if live { "live" } else { "dry-run" },
        "provider": provider.as_str(),
        "generation": generation,
        "image_path": image_path.display().to_string(),
        "inserted": inserted
    }))
}

fn insert_image(request: InsertImageRequest) -> Result<Value, String> {
    let path = absolute_path(request.path)?;
    let bytes = fs::read(&path).map_err(|err| format!("read image failed: {err}"))?;
    if bytes.is_empty() {
        return Err(format!("image file is empty: {}", path.display()));
    }
    let snapshot = snapshot(request.window.clone()).unwrap_or_else(|_| json!({}));
    let (x, y) = placement(request.x, request.y, &snapshot);
    let title = request.title.unwrap_or_else(|| {
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Image")
            .to_string()
    });
    let script = format!(
        "window.capyWorkbench.insertImageFromBase64({}, {}, {}, {}, {})",
        js_string(&STANDARD.encode(&bytes)),
        js_string(&title),
        x,
        y,
        js_value(&json!({
            "sourcePath": path.display().to_string(),
            "provider": request.provider,
            "promptSummary": request.prompt_summary
        }))
    );
    canvas_eval(&script, request.window)
}

fn snapshot(window: Option<String>) -> Result<Value, String> {
    canvas_eval(
        "window.capyWorkbench.refreshPlannerContext && window.capyWorkbench.refreshPlannerContext()",
        window,
    )
}

fn canvas_eval(script: &str, window: Option<String>) -> Result<Value, String> {
    request_data(
        "devtools-eval",
        json!({
            "eval": script,
            "window": window
        }),
    )
}

fn request_data(op: &str, params: Value) -> Result<Value, String> {
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

fn write_fixture_png(out_dir: &Path, name: &str) -> Result<PathBuf, String> {
    fs::create_dir_all(out_dir).map_err(|err| format!("create output dir failed: {err}"))?;
    let path = out_dir.join(format!("{}.png", sanitize_name(name)));
    let width = 512;
    let height = 512;
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let r = 180u8.saturating_add((x % 54) as u8);
            let g = 116u8.saturating_add((y % 82) as u8);
            let b = 200u8.saturating_sub(((x + y) % 60) as u8);
            pixels.extend_from_slice(&[r, g, b, 255]);
        }
    }
    let mut png = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut png);
    encoder
        .write_image(&pixels, width, height, image::ExtendedColorType::Rgba8)
        .map_err(|err| format!("encode dry-run fixture failed: {err}"))?;
    fs::write(&path, png).map_err(|err| format!("write dry-run fixture failed: {err}"))?;
    Ok(path)
}

fn prompt_summary(prompt: &str) -> String {
    let summary = prompt.split_whitespace().collect::<Vec<_>>().join(" ");
    summary.chars().take(220).collect()
}

fn sanitize_name(name: &str) -> String {
    let trimmed = name.trim();
    let mut out = String::new();
    for ch in trimmed.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else if ch.is_whitespace() || ch == '.' {
            out.push('-');
        }
    }
    if out.is_empty() {
        "canvas-image".to_string()
    } else {
        out
    }
}

fn absolute_path(path: PathBuf) -> Result<PathBuf, String> {
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
    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
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
