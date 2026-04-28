use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn media_scroll_pack_emits_composition_from_poster_json() -> Result<(), Box<dyn std::error::Error>>
{
    let dir = unique_dir("scroll-composition")?;
    let input = workspace_root()?.join("fixtures/poster/sample-poster.json");
    let output = capy_command()?
        .args([
            "media",
            "scroll-pack",
            "--input",
            &input.display().to_string(),
            "--emit-composition",
            "--out",
            &dir.display().to_string(),
            "--overwrite",
        ])
        .output()?;

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(value["ok"], true);
    assert_eq!(value["components"][0], "html.capy-scroll-chapter");
    let composition_path = value["composition_path"]
        .as_str()
        .ok_or("composition_path should be a string")?;
    assert!(Path::new(composition_path).is_file());
    assert!(dir.join("components/html.capy-scroll-chapter.js").is_file());

    let composition: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(composition_path)?)?;
    assert_eq!(composition["tracks"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        composition["tracks"][0]["component"],
        "html.capy-scroll-chapter"
    );
    assert_eq!(composition["tracks"][0]["params"]["chapter_index"], 0);
    fs::remove_dir_all(dir)?;
    Ok(())
}

#[test]
fn media_scroll_pack_rejects_mixed_emit_flags() -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_dir("mixed-flags")?;
    let input = workspace_root()?.join("fixtures/poster/sample-poster.json");
    let output = capy_command()?
        .args([
            "media",
            "scroll-pack",
            "--input",
            &input.display().to_string(),
            "--emit-html",
            "--emit-composition",
            "--out",
            &dir.display().to_string(),
        ])
        .output()?;

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr)?;
    assert!(stderr.contains("--emit-html and --emit-composition are mutually exclusive"));
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
        "capy-media-cli-{label}-{}-{}",
        std::process::id(),
        monotonic_millis()?
    ));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn monotonic_millis() -> Result<u128, std::time::SystemTimeError> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis())
}
