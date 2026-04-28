use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[test]
fn nextframe_doctor_reports_happy_path_json() -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_dir("happy")?;
    let nf = write_fake_binary(&dir, "nf", "nf 1.2.3")?;
    let recorder = write_fake_binary(&dir, "nf-recorder", "nf-recorder 4.5.6")?;
    let output = capy_command()?
        .args([
            "nextframe",
            "doctor",
            "--nf",
            &nf.display().to_string(),
            "--recorder",
            &recorder.display().to_string(),
        ])
        .output()?;

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(value["ok"], true);
    assert_eq!(value["stage"], "doctor");
    assert_eq!(value["mode"], "binary");
    assert_eq!(value["nf"]["found"], true);
    assert_eq!(value["nf_recorder"]["found"], true);
    assert_eq!(value["config"]["discovery"], "FLAG");
    fs::remove_dir_all(dir)?;
    Ok(())
}

#[test]
fn nextframe_doctor_reports_missing_json() -> Result<(), Box<dyn std::error::Error>> {
    let output = capy_command()?
        .args([
            "nextframe",
            "doctor",
            "--nf",
            "/definitely/not/nf",
            "--recorder",
            "/definitely/not/nf-recorder",
        ])
        .output()?;

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(value["ok"], false);
    assert_eq!(value["error"]["code"], "NEXTFRAME_NOT_FOUND");
    assert_eq!(
        value["error"]["hint"],
        [
            "install nf via cargo install --path ",
            "/Users/Zhuanz/workspace/",
            "NextFrame/crates/nf-cli or set CAPY_NF env"
        ]
        .concat()
    );
    Ok(())
}

#[test]
fn nextframe_compose_poster_writes_composition_json() -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_dir("compose-poster")?;
    let input = workspace_root()?.join("fixtures/poster/sample-poster.json");
    let output = capy_command()?
        .args([
            "nextframe",
            "compose-poster",
            "--input",
            &input.display().to_string(),
            "--out",
            &dir.display().to_string(),
        ])
        .output()?;

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(value["ok"], true);
    assert_eq!(value["components"][0], "html.capy-poster");
    assert_eq!(value["layers"], 6);
    let composition_path = value["composition_path"]
        .as_str()
        .ok_or("composition_path should be a string")?;
    assert!(Path::new(composition_path).exists());

    let composition_text = fs::read_to_string(composition_path)?;
    assert!(composition_text.contains("\"html.capy-poster\""));
    let composition: serde_json::Value = serde_json::from_str(&composition_text)?;
    assert_eq!(composition["tracks"].as_array().map(Vec::len), Some(1));
    fs::remove_dir_all(dir)?;
    Ok(())
}

#[test]
fn nextframe_compose_poster_reports_invalid_json() -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_dir("compose-invalid")?;
    let input = dir.join("invalid.json");
    fs::write(&input, "{")?;
    let out = dir.join("out");
    let output = capy_command()?
        .args([
            "nextframe",
            "compose-poster",
            "--input",
            &input.display().to_string(),
            "--out",
            &out.display().to_string(),
        ])
        .output()?;

    assert!(!output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(value["ok"], false);
    assert_eq!(value["error"]["code"], "POSTER_INVALID");
    assert!(output.stderr.is_empty());
    fs::remove_dir_all(dir)?;
    Ok(())
}

#[test]
fn nextframe_validate_accepts_composed_composition() -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_dir("validate-happy")?;
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
        .args(["nextframe", "validate", "--composition", composition_path])
        .output()?;

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(value["ok"], true);
    assert_eq!(value["track_count"], 1);
    assert_eq!(value["components"][0], "html.capy-poster");
    assert!(
        value["errors"]
            .as_array()
            .ok_or("errors should be array")?
            .is_empty()
    );
    fs::remove_dir_all(dir)?;
    Ok(())
}

#[test]
fn nextframe_validate_reports_empty_tracks() -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_dir("validate-empty")?;
    let composition = dir.join("composition.json");
    fs::write(
        &composition,
        r#"{"schema":"nextframe.composition.v2","schema_version":"capy.composition.v1","id":"broken","title":"broken","name":"broken","duration_ms":1000,"duration":"1000ms","viewport":{"w":1920,"h":1080,"ratio":"16:9"},"theme":"default","tracks":[],"assets":[]}"#,
    )?;

    let output = capy_command()?
        .args([
            "nextframe",
            "validate",
            "--composition",
            &composition.display().to_string(),
        ])
        .output()?;

    assert!(!output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(value["ok"], false);
    assert_eq!(value["errors"][0]["code"], "EMPTY_TRACKS");
    fs::remove_dir_all(dir)?;
    Ok(())
}

#[test]
fn nextframe_validate_reports_missing_composition() -> Result<(), Box<dyn std::error::Error>> {
    let output = capy_command()?
        .args([
            "nextframe",
            "validate",
            "--composition",
            "/definitely/not/composition.json",
        ])
        .output()?;

    assert!(!output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(value["ok"], false);
    assert_eq!(value["errors"][0]["code"], "COMPOSITION_NOT_FOUND");
    Ok(())
}

#[test]
fn nextframe_validate_strict_binary_requires_nf() -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_dir("validate-strict")?;
    let composition = dir.join("composition.json");
    fs::write(
        &composition,
        r#"{"schema":"nextframe.composition.v2","schema_version":"capy.composition.v1","id":"poster-snapshot","title":"Poster Snapshot","name":"Poster Snapshot","duration_ms":1000,"duration":"1000ms","viewport":{"w":1920,"h":1080,"ratio":"16:9"},"theme":"default","tracks":[{"id":"track-poster","kind":"component","component":"html.capy-poster","z":10,"time":{"start":"0ms","end":"1000ms"},"duration_ms":1000,"params":{"poster":{"type":"poster"}}}],"assets":[]}"#,
    )?;

    let output = capy_command()?
        .env("PATH", "/definitely/not/on/path")
        .env_remove("CAPY_NF")
        .env_remove("CAPY_NF_RECORDER")
        .args([
            "nextframe",
            "validate",
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
        "capy-nextframe-cli-{label}-{}-{}",
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

fn write_fake_binary(
    dir: &Path,
    name: &str,
    version: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let path = dir.join(name);
    fs::write(
        &path,
        format!(
            "#!/usr/bin/env bash\nif [[ \"$1\" == \"--version\" ]]; then echo \"{version}\"; exit 0; fi\nexit 0\n"
        ),
    )?;
    #[cfg(unix)]
    {
        let mut permissions = fs::metadata(&path)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions)?;
    }
    Ok(path)
}
