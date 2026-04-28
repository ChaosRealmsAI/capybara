use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use clap::{Args, Subcommand, ValueEnum};
use image::{GenericImageView, ImageEncoder};
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

#[derive(Debug, Args)]
struct CanvasContextArgs {
    #[command(subcommand)]
    command: CanvasContextCommand,
}

#[derive(Debug, Subcommand)]
enum CanvasContextCommand {
    #[command(about = "Write context.json plus real desktop visual attachments")]
    Export(CanvasContextExportArgs),
}

#[derive(Debug, Args)]
struct CanvasContextExportArgs {
    #[arg(
        long,
        help = "Export the currently selected image as whole-image context"
    )]
    selected: bool,
    #[arg(
        long,
        value_name = "x,y,w,h",
        help = "Canvas-world rectangle for region context; omitted uses the live UI region"
    )]
    region: Option<String>,
    #[arg(long, default_value = "tmp/capy-canvas-context")]
    out: PathBuf,
    #[arg(long)]
    window: Option<String>,
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
        CanvasCommand::Context(args) => match args.command {
            CanvasContextCommand::Export(args) => export_context(args),
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

#[derive(Debug, Clone, Copy)]
struct RectF {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

#[derive(Debug, Clone, Copy)]
struct RectU32 {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

impl RectF {
    fn normalized(self) -> Self {
        Self {
            x: if self.w < 0.0 {
                self.x + self.w
            } else {
                self.x
            },
            y: if self.h < 0.0 {
                self.y + self.h
            } else {
                self.y
            },
            w: self.w.abs(),
            h: self.h.abs(),
        }
    }

    fn json(self) -> Value {
        json!({
            "x": round2(self.x),
            "y": round2(self.y),
            "w": round2(self.w),
            "h": round2(self.h)
        })
    }
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

fn export_context(args: CanvasContextExportArgs) -> Result<Value, String> {
    let out_dir = absolute_path(args.out)?;
    fs::create_dir_all(&out_dir).map_err(|err| format!("create context dir failed: {err}"))?;

    let initial_snapshot = snapshot(args.window.clone())?;
    let initial_selected = selected_node(&initial_snapshot)?;
    let selected_id = initial_selected
        .get("id")
        .and_then(Value::as_u64)
        .ok_or_else(|| "selected node is missing id".to_string())?;
    focus_selected_node(selected_id, args.window.clone())?;

    let snapshot = snapshot(args.window.clone())?;
    let selected = selected_node(&snapshot)?;
    let selected_id = selected
        .get("id")
        .and_then(Value::as_u64)
        .ok_or_else(|| "selected node is missing id".to_string())?;
    let content_kind = selected
        .get("content_kind")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    if content_kind != "image" {
        return Err(format!(
            "canvas context export requires an image selection; selected node {selected_id} is {content_kind}"
        ));
    }
    let node_bounds = rect_from_value(
        selected
            .get("bounds")
            .or_else(|| selected.get("geometry"))
            .ok_or_else(|| "selected node is missing bounds".to_string())?,
    )?;
    let viewport = snapshot
        .get("canvas")
        .and_then(|canvas| canvas.get("viewport"))
        .ok_or_else(|| "canvas viewport is missing".to_string())?;
    let layout = canvas_eval(&context_layout_script(), args.window.clone())?;
    let canvas_rect = rect_from_value(
        layout
            .get("canvasRect")
            .ok_or_else(|| "canvas layout rect is missing".to_string())?,
    )?;
    let inner_w = layout
        .get("innerWidth")
        .and_then(Value::as_f64)
        .unwrap_or((canvas_rect.x + canvas_rect.w).max(1.0));
    let inner_h = layout
        .get("innerHeight")
        .and_then(Value::as_f64)
        .unwrap_or((canvas_rect.y + canvas_rect.h).max(1.0));

    let window_capture = out_dir.join("window.png");
    let capture_result = request_data(
        "capture",
        json!({
            "out": window_capture.display().to_string(),
            "window": args.window
        }),
    )?;
    let image = image::open(&window_capture)
        .map_err(|err| format!("read native window capture failed: {err}"))?;
    let (image_w, image_h) = image.dimensions();
    let scale_x = image_w as f64 / inner_w.max(1.0);
    let scale_y = image_h as f64 / inner_h.max(1.0);

    let viewport_path = out_dir.join("viewport.png");
    crop_to_file(
        &image,
        css_rect_to_pixel(canvas_rect, scale_x, scale_y, image_w, image_h)?,
        &viewport_path,
    )?;

    let selected_screen_rect = world_rect_to_window_rect(node_bounds, viewport, canvas_rect)?;
    let selected_node_path = out_dir.join("selected-node.png");
    crop_to_file(
        &image,
        css_rect_to_pixel(selected_screen_rect, scale_x, scale_y, image_w, image_h)?,
        &selected_node_path,
    )?;

    let active_context = layout.get("activeContext").cloned().unwrap_or(Value::Null);
    let region_world =
        if args.selected && args.region.is_none() {
            None
        } else {
            match args.region.as_deref() {
                Some(region) => Some(clamp_rect(parse_rect_arg(region)?, node_bounds).ok_or_else(
                    || "requested region is outside selected image bounds".to_string(),
                )?),
                None => active_context
                    .get("region_bounds_world")
                    .and_then(|value| if value.is_null() { None } else { Some(value) })
                    .map(rect_from_value)
                    .transpose()?
                    .and_then(|region| clamp_rect(region, node_bounds)),
            }
        };
    let export_region = region_world.filter(|region| region.w > 0.0 && region.h > 0.0);
    if args.region.is_none() && !args.selected && export_region.is_none() {
        return Err(
            "missing context target: use --selected or --region x,y,w,h, or draw a live UI region"
                .to_string(),
        );
    }

    let region_path = if let Some(region) = export_region {
        let region_screen_rect = world_rect_to_window_rect(region, viewport, canvas_rect)?;
        let path = out_dir.join("region.png");
        crop_to_file(
            &image,
            css_rect_to_pixel(region_screen_rect, scale_x, scale_y, image_w, image_h)?,
            &path,
        )?;
        Some(path)
    } else {
        None
    };

    let source_path = selected
        .get("source_path")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    let source_image_size = source_path
        .as_deref()
        .and_then(|path| image::image_dimensions(path).ok())
        .map(|(w, h)| json!({ "w": w, "h": h }));
    let region_source_pixels = export_region.and_then(|region| {
        let source = source_image_size.as_ref()?;
        let source_w = source.get("w")?.as_f64()?;
        let source_h = source.get("h")?.as_f64()?;
        Some(
            RectF {
                x: ((region.x - node_bounds.x) / node_bounds.w) * source_w,
                y: ((region.y - node_bounds.y) / node_bounds.h) * source_h,
                w: (region.w / node_bounds.w) * source_w,
                h: (region.h / node_bounds.h) * source_h,
            }
            .normalized()
            .json(),
        )
    });

    let kind = if export_region.is_some() {
        "image_region"
    } else {
        "selected_image"
    };
    let region = export_region.map(|region| {
        json!({
            "world": region.json(),
            "source_pixels": region_source_pixels.clone(),
            "node_percent": RectF {
                x: (region.x - node_bounds.x) / node_bounds.w,
                y: (region.y - node_bounds.y) / node_bounds.h,
                w: region.w / node_bounds.w,
                h: region.h / node_bounds.h,
            }
            .normalized()
            .json()
        })
    });
    let context_id = format!("ctx-{}-{selected_id}-{}", kind.replace('_', "-"), now_ms());
    let attachments = json!({
        "window_capture": window_capture.display().to_string(),
        "viewport_png": viewport_path.display().to_string(),
        "selected_node_png": selected_node_path.display().to_string(),
        "region_png": region_path.as_ref().map(|path| path.display().to_string())
    });
    let attachment_paths = [
        Some(window_capture.clone()),
        Some(viewport_path.clone()),
        Some(selected_node_path.clone()),
        region_path.clone(),
    ]
    .into_iter()
    .flatten()
    .map(|path| path.display().to_string())
    .collect::<Vec<_>>();
    let context = json!({
        "schema_version": 1,
        "context_id": context_id,
        "kind": kind,
        "created_at_ms": now_ms(),
        "source": {
            "node": selected,
            "source_path": source_path,
            "source_image_size": source_image_size,
            "canvas_snapshot": {
                "selectedId": snapshot.get("selectedId").cloned().unwrap_or(Value::Null),
                "nodeCount": snapshot.get("canvas").and_then(|canvas| canvas.get("nodeCount")).cloned().unwrap_or(Value::Null),
                "viewport": viewport
            }
        },
        "geometry": {
            "node_world": node_bounds.json(),
            "node_window_css": selected_screen_rect.json(),
            "canvas_window_css": canvas_rect.json(),
            "region_world": export_region.map(RectF::json),
            "region_source_pixels": region_source_pixels
        },
        "region": region,
        "attachments": attachments,
        "attachment_paths": attachment_paths.clone(),
        "capture": capture_result,
        "runtime": {
            "page_errors": layout.get("pageErrors").cloned().unwrap_or_else(|| json!([])),
            "console_errors": layout.get("consoleErrors").cloned().unwrap_or_else(|| json!([])),
            "active_context_preview": active_context
        }
    });
    let context_path = out_dir.join("context.json");
    fs::write(
        &context_path,
        serde_json::to_string_pretty(&context).map_err(|err| err.to_string())?,
    )
    .map_err(|err| format!("write context.json failed: {err}"))?;
    Ok(json!({
        "ok": true,
        "kind": "canvas-context-packet",
        "context_id": context.get("context_id").cloned().unwrap_or(Value::Null),
        "context_kind": kind,
        "out": out_dir.display().to_string(),
        "context_json": context_path.display().to_string(),
        "attachment_paths": attachment_paths,
        "context": context
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

fn focus_selected_node(id: u64, window: Option<String>) -> Result<(), String> {
    let result = canvas_eval(
        &format!(
            r#"(() => {{
  const focusNode = window.capyWorkbench && window.capyWorkbench.focusNode;
  if (!focusNode) return {{ ok: false, error: "missing capyWorkbench.focusNode" }};
  return {{ ok: Boolean(focusNode({id})) }};
}})()"#
        ),
        window,
    )?;
    if result.get("ok").and_then(Value::as_bool).unwrap_or(false) {
        Ok(())
    } else {
        Err(result
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("selected node could not be focused for context export")
            .to_string())
    }
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

fn selected_node(snapshot: &Value) -> Result<&Value, String> {
    snapshot
        .get("canvas")
        .and_then(|canvas| canvas.get("selectedNode"))
        .filter(|value| !value.is_null())
        .ok_or_else(|| "no selected canvas node".to_string())
}

fn rect_from_value(value: &Value) -> Result<RectF, String> {
    let read = |key: &str| {
        value
            .get(key)
            .and_then(Value::as_f64)
            .ok_or_else(|| format!("rect is missing numeric {key}: {value}"))
    };
    Ok(RectF {
        x: read("x")?,
        y: read("y")?,
        w: read("w").or_else(|_| read("width"))?,
        h: read("h").or_else(|_| read("height"))?,
    }
    .normalized())
}

fn parse_rect_arg(value: &str) -> Result<RectF, String> {
    let parts = value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.parse::<f64>()
                .map_err(|err| format!("invalid --region number {part:?}: {err}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if parts.len() != 4 {
        return Err("--region must be x,y,w,h".to_string());
    }
    Ok(RectF {
        x: parts[0],
        y: parts[1],
        w: parts[2],
        h: parts[3],
    }
    .normalized())
}

fn clamp_rect(rect: RectF, bounds: RectF) -> Option<RectF> {
    let x1 = rect.x.max(bounds.x);
    let y1 = rect.y.max(bounds.y);
    let x2 = (rect.x + rect.w).min(bounds.x + bounds.w);
    let y2 = (rect.y + rect.h).min(bounds.y + bounds.h);
    (x2 > x1 && y2 > y1).then_some(RectF {
        x: x1,
        y: y1,
        w: x2 - x1,
        h: y2 - y1,
    })
}

fn world_rect_to_window_rect(
    world: RectF,
    viewport: &Value,
    canvas_rect: RectF,
) -> Result<RectF, String> {
    let zoom = viewport.get("zoom").and_then(Value::as_f64).unwrap_or(1.0);
    let offset = viewport
        .get("camera_offset")
        .ok_or_else(|| "viewport camera_offset is missing".to_string())?;
    let offset_x = offset.get("x").and_then(Value::as_f64).unwrap_or(0.0);
    let offset_y = offset.get("y").and_then(Value::as_f64).unwrap_or(0.0);
    Ok(RectF {
        x: canvas_rect.x + world.x * zoom + offset_x,
        y: canvas_rect.y + world.y * zoom + offset_y,
        w: world.w * zoom,
        h: world.h * zoom,
    }
    .normalized())
}

fn css_rect_to_pixel(
    rect: RectF,
    scale_x: f64,
    scale_y: f64,
    image_w: u32,
    image_h: u32,
) -> Result<RectU32, String> {
    let x1 = (rect.x * scale_x).floor().max(0.0).min(image_w as f64) as u32;
    let y1 = (rect.y * scale_y).floor().max(0.0).min(image_h as f64) as u32;
    let x2 = ((rect.x + rect.w) * scale_x)
        .ceil()
        .max(0.0)
        .min(image_w as f64) as u32;
    let y2 = ((rect.y + rect.h) * scale_y)
        .ceil()
        .max(0.0)
        .min(image_h as f64) as u32;
    let w = x2.saturating_sub(x1);
    let h = y2.saturating_sub(y1);
    if w == 0 || h == 0 {
        return Err(format!(
            "crop rect is empty after clamping: {:?}",
            rect.json()
        ));
    }
    Ok(RectU32 { x: x1, y: y1, w, h })
}

fn crop_to_file(image: &image::DynamicImage, rect: RectU32, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("create crop dir failed: {err}"))?;
    }
    let crop = image::imageops::crop_imm(image, rect.x, rect.y, rect.w, rect.h).to_image();
    crop.save(path)
        .map_err(|err| format!("write crop {} failed: {err}", path.display()))
}

fn context_layout_script() -> String {
    r#"(() => {
  const rect = (selector) => {
    const el = document.querySelector(selector);
    if (!el) return { found: false, x: 0, y: 0, w: 0, h: 0, width: 0, height: 0 };
    const box = el.getBoundingClientRect();
    return { found: true, x: box.x, y: box.y, w: box.width, h: box.height, width: box.width, height: box.height };
  };
  const activeContext = window.capyWorkbench?.activeCanvasContext?.() || null;
  return {
    innerWidth: window.innerWidth,
    innerHeight: window.innerHeight,
    devicePixelRatio: window.devicePixelRatio || 1,
    canvasRect: rect('[data-section="canvas-host"]'),
    plannerRect: rect('[data-section="planner-chat"]'),
    contextTitle: document.querySelector('#context-title')?.textContent || '',
    contextMeta: document.querySelector('#context-meta')?.textContent || '',
    activeContext,
    pageErrors: window.__capyPageErrors || [],
    consoleErrors: (window.__capyConsoleEvents || []).filter((event) => event.level === 'error')
  };
})()"#
        .to_string()
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

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
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
