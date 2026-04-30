use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use super::model;

pub(super) fn verify_manifest(manifest_path: &Path) -> Result<Value, String> {
    let root = package_root(manifest_path)?;
    let manifest = read_json(manifest_path)?;
    let report = read_json(&root.join("qa/report.json"))?;
    let mut missing = Vec::new();
    for rel in required_files() {
        if !root.join(rel).is_file() {
            missing.push(rel.to_string());
        }
    }
    let frame_count = source_frame_count(&manifest);
    let rgba_count = count_pngs(&root.join("frames/rgba"));
    let mask_count = count_pngs(&root.join("masks"));
    let passed = missing.is_empty()
        && frame_count > 0
        && rgba_count == frame_count
        && mask_count == frame_count
        && manifest.get("schema").and_then(Value::as_str) == Some(model::MANIFEST_SCHEMA)
        && report.get("schema").and_then(Value::as_str) == Some(model::QA_SCHEMA);
    Ok(json!({
        "schema": "capy.motion.verify.v1",
        "verdict": if passed { "passed" } else { "failed" },
        "manifest": manifest_path,
        "missing": missing,
        "frame_count": frame_count,
        "rgba_frames": rgba_count,
        "masks": mask_count,
        "qa_verdict": report.get("verdict").cloned().unwrap_or(Value::Null),
        "preview_html": root.join("qa/preview.html")
    }))
}

pub(super) fn inspect_manifest(manifest_path: &Path) -> Result<Value, String> {
    let root = package_root(manifest_path)?;
    let manifest = read_json(manifest_path)?;
    let report = read_json(&root.join("qa/report.json"))?;
    let prompt_files = prompt_file_status(root, &manifest);
    let source_frame_count = source_frame_count(&manifest);
    let rgba_frames = count_pngs(&root.join("frames/rgba"));
    let masks = count_pngs(&root.join("masks"));
    let missing_outputs = required_outputs()
        .iter()
        .filter(|path| !root.join(path).is_file())
        .copied()
        .collect::<Vec<_>>();
    Ok(json!({
        "ok": missing_outputs.is_empty()
            && prompt_files.iter().all(|item| item.get("exists").and_then(Value::as_bool) == Some(true))
            && source_frame_count > 0
            && rgba_frames == source_frame_count
            && masks == source_frame_count,
        "schema": "capy.motion.inspect.v1",
        "package_root": root,
        "manifest": manifest_path,
        "source": manifest.get("source").cloned().unwrap_or(Value::Null),
        "strategy": manifest.get("strategy").cloned().unwrap_or(Value::Null),
        "outputs": manifest.get("outputs").cloned().unwrap_or(Value::Null),
        "prompts": prompt_files,
        "quality": manifest.get("quality").cloned().unwrap_or(Value::Null),
        "qa_verdict": report.get("verdict").cloned().unwrap_or(Value::Null),
        "counts": {
            "source_frame_count": source_frame_count,
            "rgba_frames": rgba_frames,
            "masks": masks
        },
        "missing_outputs": missing_outputs,
        "integration": {
            "preferred_browser_alpha": "video/preview.webm",
            "preferred_game_runtime": ["frames/rgba", "atlas/walk.png", "atlas/walk.json"],
            "fallback_dual_stream": ["video/rgb.mp4", "video/alpha.mp4"],
            "ordinary_h264_transparency": false
        },
        "preview_url_path": "/qa/preview.html"
    }))
}

fn package_root(manifest_path: &Path) -> Result<&Path, String> {
    manifest_path
        .parent()
        .ok_or_else(|| format!("{} has no parent", manifest_path.display()))
}

fn prompt_file_status(root: &Path, manifest: &Value) -> Vec<Value> {
    manifest
        .get("prompts")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("path").and_then(Value::as_str))
                .map(|path| {
                    json!({
                        "path": path,
                        "exists": root.join(path).is_file()
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn source_frame_count(manifest: &Value) -> u64 {
    manifest
        .pointer("/source/frame_count")
        .and_then(Value::as_u64)
        .unwrap_or(0)
}

fn required_files() -> &'static [&'static str] {
    &[
        "manifest.json",
        "qa/report.json",
        "qa/preview.html",
        "qa/contact-deep.png",
        "atlas/walk.png",
        "atlas/walk.json",
        "video/preview.webm",
        "video/rgb.mp4",
        "video/alpha.mp4",
        "prompts/README.md",
        "prompts/process.md",
        "prompts/qa-review.md",
        "prompts/app-integration.md",
    ]
}

fn required_outputs() -> &'static [&'static str] {
    &[
        "manifest.json",
        "qa/report.json",
        "qa/preview.html",
        "atlas/walk.png",
        "atlas/walk.json",
        "video/preview.webm",
        "video/rgb.mp4",
        "video/alpha.mp4",
    ]
}

fn count_pngs(dir: &Path) -> u64 {
    fs::read_dir(dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().and_then(|value| value.to_str()) == Some("png"))
        .count() as u64
}

fn read_json(path: &Path) -> Result<Value, String> {
    let text =
        fs::read_to_string(path).map_err(|err| format!("read {} failed: {err}", path.display()))?;
    serde_json::from_str(&text).map_err(|err| format!("parse {} failed: {err}", path.display()))
}
