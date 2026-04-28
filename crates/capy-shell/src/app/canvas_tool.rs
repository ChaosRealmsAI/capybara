use std::fs;
use std::path::{Path, PathBuf};
use std::thread;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use image::ImageEncoder;
use serde_json::{Value, json};
use tao::event_loop::EventLoopProxy;
use uuid::Uuid;

use super::ShellEvent;

pub fn start_image_generation(
    proxy: EventLoopProxy<ShellEvent>,
    window_id: String,
    params: Value,
) -> Result<Value, String> {
    let run_id = format!("canvas-image-{}", Uuid::new_v4());
    let request = ToolRequest::from_params(run_id.clone(), params)?;
    let ack = json!({
        "ok": true,
        "kind": "canvas-image-tool-started",
        "run_id": run_id,
        "mode": if request.live { "live" } else { "dry-run" },
        "provider": request.provider.as_str()
    });
    thread::spawn(move || {
        let event = run_image_generation(request);
        let _send_result = proxy.send_event(ShellEvent::CanvasToolEvent { window_id, event });
    });
    Ok(ack)
}

#[derive(Debug)]
struct ToolRequest {
    run_id: String,
    provider: capy_image_gen::ImageProviderId,
    prompt: String,
    size: String,
    resolution: String,
    refs: Vec<String>,
    out_dir: PathBuf,
    name: String,
    live: bool,
    x: f64,
    y: f64,
    title: String,
}

impl ToolRequest {
    fn from_params(run_id: String, params: Value) -> Result<Self, String> {
        let prompt = required_string(&params, "prompt")?;
        let provider = match params
            .get("provider")
            .and_then(Value::as_str)
            .unwrap_or("apimart-gpt-image-2")
        {
            "apimart-gpt-image-2" => capy_image_gen::ImageProviderId::ApimartGptImage2,
            other => return Err(format!("unsupported image provider: {other}")),
        };
        let live = params.get("live").and_then(Value::as_bool).unwrap_or(false);
        let out_dir = params
            .get("out")
            .and_then(Value::as_str)
            .map(PathBuf::from)
            .unwrap_or_else(default_output_dir);
        Ok(Self {
            run_id,
            provider,
            prompt,
            size: params
                .get("size")
                .and_then(Value::as_str)
                .unwrap_or("1:1")
                .to_string(),
            resolution: params
                .get("resolution")
                .and_then(Value::as_str)
                .unwrap_or("1k")
                .to_string(),
            refs: params
                .get("refs")
                .and_then(Value::as_array)
                .map(|refs| {
                    refs.iter()
                        .filter_map(Value::as_str)
                        .map(ToString::to_string)
                        .collect()
                })
                .unwrap_or_default(),
            out_dir,
            name: params
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("canvas-image")
                .to_string(),
            live,
            x: params.get("x").and_then(Value::as_f64).unwrap_or(360.0),
            y: params.get("y").and_then(Value::as_f64).unwrap_or(140.0),
            title: params
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("Generated image")
                .to_string(),
        })
    }
}

fn run_image_generation(request: ToolRequest) -> Value {
    match run_image_generation_inner(&request) {
        Ok(mut event) => {
            event["run_id"] = json!(request.run_id);
            event
        }
        Err(error) => json!({
            "kind": "canvas-image-tool-result",
            "ok": false,
            "run_id": request.run_id,
            "mode": if request.live { "live" } else { "dry-run" },
            "error": { "message": error }
        }),
    }
}

fn run_image_generation_inner(request: &ToolRequest) -> Result<Value, String> {
    let mode = if request.live {
        capy_image_gen::ImageGenerateMode::Generate
    } else {
        capy_image_gen::ImageGenerateMode::DryRun
    };
    let image_request = capy_image_gen::GenerateImageRequest {
        provider: request.provider,
        mode,
        prompt: Some(request.prompt.clone()),
        size: request.size.clone(),
        resolution: request.resolution.clone(),
        refs: request.refs.clone(),
        output_dir: Some(request.out_dir.clone()),
        name: Some(request.name.clone()),
        download: true,
        task_id: None,
    };
    let generation =
        capy_image_gen::generate_image(image_request).map_err(|err| err.to_string())?;
    let image_path = if request.live {
        capy_image_gen::find_downloaded_image_path(&generation).ok_or_else(|| {
            "live generation did not report an existing downloaded image path".to_string()
        })?
    } else {
        write_fixture_png(&request.out_dir, &request.name)?
    };
    let bytes =
        fs::read(&image_path).map_err(|err| format!("read generated image failed: {err}"))?;
    Ok(json!({
        "kind": "canvas-image-tool-result",
        "ok": true,
        "mode": if request.live { "live" } else { "dry-run" },
        "provider": request.provider.as_str(),
        "title": request.title,
        "x": request.x,
        "y": request.y,
        "source_path": image_path.display().to_string(),
        "prompt_summary": prompt_summary(&request.prompt),
        "image_base64": STANDARD.encode(bytes),
        "image_bytes": fs::metadata(&image_path).map(|meta| meta.len()).unwrap_or(0),
        "generation": generation
    }))
}

fn write_fixture_png(out_dir: &Path, name: &str) -> Result<PathBuf, String> {
    fs::create_dir_all(out_dir).map_err(|err| format!("create output dir failed: {err}"))?;
    let path = out_dir.join(format!("{}.png", sanitize_name(name)));
    let width = 512;
    let height = 512;
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let r = 170u8.saturating_add((x % 60) as u8);
            let g = 128u8.saturating_add((y % 70) as u8);
            let b = 210u8.saturating_sub(((x + y) % 70) as u8);
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

fn default_output_dir() -> PathBuf {
    std::env::var_os("CAPY_DEFAULT_CWD")
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tmp/capy-canvas-image-tool")
}

fn required_string(params: &Value, key: &str) -> Result<String, String> {
    params
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| format!("missing required parameter: {key}"))
}

fn prompt_summary(prompt: &str) -> String {
    let summary = prompt.split_whitespace().collect::<Vec<_>>().join(" ");
    summary.chars().take(220).collect()
}

fn sanitize_name(name: &str) -> String {
    let mut out = String::new();
    for ch in name.trim().chars() {
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
