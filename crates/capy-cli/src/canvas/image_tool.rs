use std::fs;
use std::path::{Path, PathBuf};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use clap::{Args, ValueEnum};
use image::ImageEncoder;
use serde_json::{Value, json};

use super::{absolute_path, canvas_eval, js_string, js_value, placement, snapshot};

#[derive(Debug, Args)]
pub(super) struct CanvasInsertImageArgs {
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
pub(super) struct CanvasGenerateImageArgs {
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
    #[arg(
        long,
        help = "Require prompt terms for images that will be passed to capy cutout"
    )]
    cutout_ready: bool,
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

struct InsertImageRequest {
    path: PathBuf,
    x: Option<f64>,
    y: Option<f64>,
    title: Option<String>,
    provider: Option<String>,
    prompt_summary: Option<String>,
    window: Option<String>,
}

pub(super) fn generate_image(args: CanvasGenerateImageArgs) -> Result<Value, String> {
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
        cutout_ready: args.cutout_ready,
    };
    let generation = capy_image_gen::generate_image(request).map_err(|err| err.to_string())?;
    let image_path = if live {
        capy_image_gen::find_downloaded_image_path(&generation).ok_or_else(|| {
            "live generation did not report an existing downloaded image path".to_string()
        })?
    } else {
        write_fixture_png(&out_dir, &name)?
    };
    let inserted = insert_image(CanvasInsertImageArgs {
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

pub(super) fn insert_image(args: CanvasInsertImageArgs) -> Result<Value, String> {
    insert_image_request(InsertImageRequest {
        path: args.path,
        x: args.x,
        y: args.y,
        title: args.title,
        provider: args.provider,
        prompt_summary: args.prompt_summary,
        window: args.window,
    })
}

fn insert_image_request(request: InsertImageRequest) -> Result<Value, String> {
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
