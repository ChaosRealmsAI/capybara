use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn timeline_snapshot_writes_png_embedded() -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_dir("snapshot-happy")?;
    let input = workspace_root()?.join("fixtures/poster/sample-poster.json");
    let compose = capy_command()?
        .args([
            "timeline",
            "compose-poster",
            "--input",
            &input.display().to_string(),
            "--out",
            &dir.display().to_string(),
        ])
        .output()?;
    assert!(compose.status.success());
    let composed: serde_json::Value = serde_json::from_slice(&compose.stdout)?;
    let composition_path = composed["composition_path"]
        .as_str()
        .ok_or("composition_path should be a string")?;
    let compile = capy_command()?
        .env("PATH", "/definitely/not/on/path")
        .args(["timeline", "compile", "--composition", composition_path])
        .output()?;
    assert!(compile.status.success());

    let output = capy_command()?
        .env("PATH", "/definitely/not/on/path")
        .env_remove("CAPY_RECORDER")
        .args([
            "timeline",
            "snapshot",
            "--composition",
            composition_path,
            "--frame",
            "0",
        ])
        .output()?;

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(value["ok"], true);
    assert_eq!(value["stage"], "snapshot");
    assert_eq!(value["snapshot_mode"], "embedded");
    assert_eq!(value["frame_ms"], 0);
    assert_eq!(value["width"], 1920);
    assert_eq!(value["height"], 1080);
    assert!(value["byte_size"].as_u64().unwrap_or(0) > 0);
    let snapshot_path = value["snapshot_path"]
        .as_str()
        .ok_or("snapshot_path should be a string")?;
    assert!(Path::new(snapshot_path).is_file());
    fs::remove_dir_all(dir)?;
    Ok(())
}

#[test]
fn timeline_snapshot_reports_render_source_missing() -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_dir("snapshot-missing-render-source")?;
    let input = workspace_root()?.join("fixtures/poster/sample-poster.json");
    let compose = capy_command()?
        .args([
            "timeline",
            "compose-poster",
            "--input",
            &input.display().to_string(),
            "--out",
            &dir.display().to_string(),
        ])
        .output()?;
    assert!(compose.status.success());
    let composed: serde_json::Value = serde_json::from_slice(&compose.stdout)?;
    let composition_path = composed["composition_path"]
        .as_str()
        .ok_or("composition_path should be a string")?;

    let output = capy_command()?
        .env("PATH", "/definitely/not/on/path")
        .env_remove("CAPY_RECORDER")
        .args(["timeline", "snapshot", "--composition", composition_path])
        .output()?;

    assert!(!output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(value["ok"], false);
    assert_eq!(value["errors"][0]["code"], "RENDER_SOURCE_MISSING");
    fs::remove_dir_all(dir)?;
    Ok(())
}

fn capy_command() -> Result<Command, Box<dyn std::error::Error>> {
    let path = std::env::var("CARGO_BIN_EXE_capy")?;
    Ok(Command::new(path))
}

fn workspace_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or("workspace root should exist")?
        .to_path_buf())
}

fn unique_dir(label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = std::env::temp_dir().join(format!(
        "capy-timeline-snapshot-cli-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis()
    ));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}
