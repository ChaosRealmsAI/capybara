use std::fs;
use std::path::PathBuf;

use clap::Args;
use serde_json::{Value, json};

use super::{absolute_path, canvas_eval, js_value, placement, snapshot};

#[derive(Debug, Args)]
pub(super) struct CanvasLoadPosterArgs {
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

pub(super) fn load_poster(args: CanvasLoadPosterArgs) -> Result<Value, String> {
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
