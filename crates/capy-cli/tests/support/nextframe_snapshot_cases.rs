use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn nextframe_snapshot_writes_png_embedded() -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_dir("snapshot-happy")?;
    let input = workspace_root()?.join("fixtures/poster/sample-poster.json");
    let compose = capy_command()?
        .args([
            "nextframe",
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
        .env_remove("CAPY_NF")
        .args(["nextframe", "compile", "--composition", composition_path])
        .output()?;
    assert!(compile.status.success());

    let output = capy_command()?
        .env("PATH", "/definitely/not/on/path")
        .env_remove("CAPY_NF")
        .env_remove("CAPY_NF_RECORDER")
        .args([
            "nextframe",
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
fn nextframe_snapshot_reports_render_source_missing() -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_dir("snapshot-missing-render-source")?;
    let input = workspace_root()?.join("fixtures/poster/sample-poster.json");
    let compose = capy_command()?
        .args([
            "nextframe",
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
        .env_remove("CAPY_NF_RECORDER")
        .args(["nextframe", "snapshot", "--composition", composition_path])
        .output()?;

    assert!(!output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(value["ok"], false);
    assert_eq!(value["errors"][0]["code"], "RENDER_SOURCE_MISSING");
    fs::remove_dir_all(dir)?;
    Ok(())
}

#[test]
fn nextframe_snapshot_strict_binary_requires_recorder() -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_dir("snapshot-strict")?;
    let composition = dir.join("composition.json");
    fs::write(&composition, valid_composition_text())?;
    fs::write(dir.join("render_source.json"), valid_render_source_text())?;

    let output = capy_command()?
        .env("PATH", "/definitely/not/on/path")
        .env_remove("CAPY_NF")
        .env_remove("CAPY_NF_RECORDER")
        .args([
            "nextframe",
            "snapshot",
            "--composition",
            &composition.display().to_string(),
            "--strict-binary",
        ])
        .output()?;

    assert!(!output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(value["ok"], false);
    assert_eq!(value["errors"][0]["code"], "NEXTFRAME_NOT_FOUND");
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
        "capy-nextframe-snapshot-cli-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis()
    ));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn valid_composition_text() -> &'static str {
    r#"{"schema":"nextframe.composition.v2","schema_version":"capy.composition.v1","id":"poster-snapshot","title":"Poster Snapshot","name":"Poster Snapshot","duration_ms":1000,"duration":"1000ms","viewport":{"w":1920,"h":1080,"ratio":"16:9"},"theme":"default","tracks":[{"id":"track-poster","kind":"component","component":"html.capy-poster","z":10,"time":{"start":"0ms","end":"1000ms"},"duration_ms":1000,"params":{"poster":{"type":"poster"}}}],"assets":[]}"#
}

fn valid_render_source_text() -> &'static str {
    r##"{"schema_version":"nf.render_source.v1","viewport":{"w":64,"h":64},"tracks":[{"id":"poster.document","clips":[{"params":{"params":{"poster":{"canvas":{"background":"#ffffff"},"assets":{},"layers":[{"id":"box","type":"shape","shape":"rect","x":0,"y":0,"width":64,"height":64,"style":{"fill":"#eeeeee"}}]}}}}]}]}"##
}
