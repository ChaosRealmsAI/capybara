use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[path = "support/nextframe_snapshot_cases.rs"]
mod nextframe_snapshot_cases;
#[path = "support/nextframe_state_cases.rs"]
mod nextframe_state_cases;

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
    assert!(dir.join("assets").is_dir());
    let assets = composition["assets"]
        .as_array()
        .ok_or("composition assets should be an array")?;
    assert_eq!(
        assets.first().and_then(|asset| asset["kind"].as_str()),
        Some("copied")
    );
    let materialized_exists = assets
        .first()
        .and_then(|asset| asset["materialized_path"].as_str())
        .map(|path| dir.join(path).is_file())
        == Some(true);
    assert!(materialized_exists);
    let has_sha = assets
        .first()
        .and_then(|asset| asset["sha256"].as_str())
        .map(|sha| sha.starts_with("sha256-"))
        == Some(true);
    assert!(has_sha);
    fs::remove_dir_all(dir)?;
    Ok(())
}

#[test]
fn nextframe_compose_poster_writes_brand_tokens() -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_dir("compose-brand")?;
    let input = workspace_root()?.join("fixtures/poster/sample-poster.json");
    let tokens = dir.join("source-tokens.css");
    fs::write(&tokens, ":root { --c-brand-1: #f9a8d4; --r-card: 20px; }\n")?;

    let output = capy_command()?
        .args([
            "nextframe",
            "compose-poster",
            "--input",
            &input.display().to_string(),
            "--brand-tokens",
            &tokens.display().to_string(),
            "--out",
            &dir.display().to_string(),
        ])
        .output()?;

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(value["ok"], true);
    let theme_hash = value["theme_hash"]
        .as_str()
        .ok_or("theme_hash should be a string")?;
    assert!(theme_hash.starts_with("brand-token-"));
    let composition_path = value["composition_path"]
        .as_str()
        .ok_or("composition_path should be a string")?;
    let composition: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(composition_path)?)?;
    assert_eq!(composition["theme"]["tokens_ref"], "tokens/tokens.json");
    assert!(dir.join("tokens/tokens.css").is_file());
    assert!(dir.join("tokens/tokens.json").is_file());
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

#[test]
fn nextframe_compile_writes_render_source_embedded() -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_dir("compile-happy")?;
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
        .env_remove("CAPY_NF")
        .args(["nextframe", "compile", "--composition", composition_path])
        .output()?;

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(value["ok"], true);
    assert_eq!(value["stage"], "compile");
    assert_eq!(value["compile_mode"], "embedded");
    assert_eq!(value["render_source_schema"], "nf.render_source.v1");
    let render_source_path = value["render_source_path"]
        .as_str()
        .ok_or("render_source_path should be a string")?;
    assert!(Path::new(render_source_path).is_file());
    fs::remove_dir_all(dir)?;
    Ok(())
}

#[test]
fn nextframe_rebuild_skips_then_recompiles_after_token_change()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_dir("rebuild-brand")?;
    let input = workspace_root()?.join("fixtures/poster/sample-poster.json");
    let tokens = dir.join("source-tokens.css");
    fs::write(&tokens, ":root { --c-brand-1: #f9a8d4; }\n")?;
    let compose = capy_command()?
        .args([
            "nextframe",
            "compose-poster",
            "--input",
            &input.display().to_string(),
            "--brand-tokens",
            &tokens.display().to_string(),
            "--out",
            &dir.display().to_string(),
        ])
        .output()?;
    assert!(compose.status.success());
    let composed: serde_json::Value = serde_json::from_slice(&compose.stdout)?;
    let composition_path = composed["composition_path"]
        .as_str()
        .ok_or("composition_path should be a string")?;

    let noop = capy_command()?
        .env("PATH", "/definitely/not/on/path")
        .env_remove("CAPY_NF")
        .args(["nextframe", "rebuild", "--composition", composition_path])
        .output()?;

    assert!(noop.status.success());
    let noop_value: serde_json::Value = serde_json::from_slice(&noop.stdout)?;
    assert_eq!(noop_value["ok"], true);
    assert_eq!(noop_value["skipped"], true);
    assert_eq!(noop_value["theme_hash"], noop_value["previous_theme_hash"]);

    fs::write(&tokens, ":root { --c-brand-1: #84cc16; }\n")?;
    let rebuild = capy_command()?
        .env("PATH", "/definitely/not/on/path")
        .env_remove("CAPY_NF")
        .args(["nextframe", "rebuild", "--composition", composition_path])
        .output()?;

    assert!(rebuild.status.success());
    let rebuild_value: serde_json::Value = serde_json::from_slice(&rebuild.stdout)?;
    assert_eq!(rebuild_value["ok"], true);
    assert!(rebuild_value.get("skipped").is_none());
    assert_ne!(
        rebuild_value["theme_hash"],
        rebuild_value["previous_theme_hash"]
    );
    assert!(dir.join("render_source.json").is_file());
    fs::remove_dir_all(dir)?;
    Ok(())
}

#[test]
fn nextframe_compile_reports_missing_composition() -> Result<(), Box<dyn std::error::Error>> {
    let output = capy_command()?
        .env("PATH", "/definitely/not/on/path")
        .env_remove("CAPY_NF")
        .args([
            "nextframe",
            "compile",
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
fn nextframe_compile_reports_invalid_composition() -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_dir("compile-invalid")?;
    let composition = dir.join("composition.json");
    fs::write(
        &composition,
        r#"{"schema":"nextframe.composition.v2","schema_version":"capy.composition.v1","id":"broken","title":"broken","name":"broken","duration_ms":1000,"duration":"1000ms","viewport":{"w":1920,"h":1080,"ratio":"16:9"},"theme":"default","tracks":[],"assets":[]}"#,
    )?;

    let output = capy_command()?
        .env("PATH", "/definitely/not/on/path")
        .env_remove("CAPY_NF")
        .args([
            "nextframe",
            "compile",
            "--composition",
            &composition.display().to_string(),
        ])
        .output()?;

    assert!(!output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(value["ok"], false);
    assert_eq!(value["errors"][0]["code"], "INVALID_COMPOSITION");
    fs::remove_dir_all(dir)?;
    Ok(())
}

#[test]
fn nextframe_compile_strict_binary_requires_nf() -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_dir("compile-strict")?;
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
        .env_remove("CAPY_NF")
        .args([
            "nextframe",
            "compile",
            "--composition",
            composition_path,
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

#[test]
#[ignore = "requires ffmpeg on PATH"]
fn nextframe_export_writes_mp4_embedded() -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_dir("export-happy")?;
    let input = workspace_root()?.join("fixtures/poster/sample-poster.json");
    let compose = capy_command()?
        .args([
            "nextframe",
            "compose-poster",
            "--input",
            &input.display().to_string(),
            "--out",
            &dir.display().to_string(),
            "--duration-ms",
            "200",
        ])
        .output()?;
    assert!(compose.status.success());
    let composed: serde_json::Value = serde_json::from_slice(&compose.stdout)?;
    let composition_path = composed["composition_path"]
        .as_str()
        .ok_or("composition_path should be a string")?;
    let compile = capy_command()?
        .args(["nextframe", "compile", "--composition", composition_path])
        .output()?;
    assert!(compile.status.success());
    let out = dir.join("export.mp4");

    let output = capy_command()?
        .env("CAPY_NF_RECORDER", "/definitely/not/nf-recorder")
        .args([
            "nextframe",
            "export",
            "--composition",
            composition_path,
            "--kind",
            "mp4",
            "--fps",
            "10",
            "--out",
            &out.display().to_string(),
        ])
        .output()?;

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(value["ok"], true);
    assert_eq!(value["stage"], "export");
    assert_eq!(value["status"], "done");
    assert_eq!(value["kind"], "mp4");
    assert_eq!(value["fps"], 10);
    assert_eq!(value["frame_count"], 2);
    assert_eq!(value["export_mode"], "embedded");
    assert!(out.is_file());
    assert!(value["byte_size"].as_u64().unwrap_or(0) > 0);
    fs::remove_dir_all(dir)?;
    Ok(())
}

#[test]
fn nextframe_export_reports_missing_render_source() -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_dir("export-missing-source")?;
    let composition = dir.join("composition.json");
    fs::write(&composition, "{}")?;

    let output = capy_command()?
        .args([
            "nextframe",
            "export",
            "--composition",
            &composition.display().to_string(),
        ])
        .output()?;

    assert!(!output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(value["ok"], false);
    assert_eq!(value["status"], "failed");
    assert_eq!(value["errors"][0]["code"], "RENDER_SOURCE_MISSING");
    fs::remove_dir_all(dir)?;
    Ok(())
}

#[test]
fn nextframe_verify_export_writes_evidence_index() -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_dir("verify-export-happy")?;
    let input = workspace_root()?.join("fixtures/poster/sample-poster.json");
    let compose = capy_command()?
        .args([
            "nextframe",
            "compose-poster",
            "--input",
            &input.display().to_string(),
            "--out",
            &dir.display().to_string(),
            "--duration-ms",
            "200",
        ])
        .output()?;
    assert!(compose.status.success());
    let composed: serde_json::Value = serde_json::from_slice(&compose.stdout)?;
    let composition_path = composed["composition_path"]
        .as_str()
        .ok_or("composition_path should be a string")?;

    let output = capy_command()?
        .env("CAPY_NF_RECORDER", "/definitely/not/nf-recorder")
        .args([
            "nextframe",
            "verify-export",
            "--composition",
            composition_path,
        ])
        .output()?;

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(value["ok"], true);
    assert_eq!(value["stage"], "verify-export");
    assert_eq!(value["verdict"], "passed");
    assert_eq!(value["stages"]["validate"]["ok"], true);
    assert_eq!(value["stages"]["compile"]["compile_mode"], "embedded");
    assert_eq!(value["stages"]["snapshot"]["ok"], true);
    assert_eq!(value["stages"]["export"]["ok"], true);
    let index_path = value["evidence_index_html"]
        .as_str()
        .ok_or("evidence_index_html should be a string")?;
    let html = fs::read_to_string(index_path)?;
    assert_eq!(html.matches(r#"<article class="stage-card">"#).count(), 4);
    assert!(html.contains("<img"));
    assert!(html.contains("<video"));
    fs::remove_dir_all(dir)?;
    Ok(())
}

#[test]
fn nextframe_verify_export_reports_missing_composition() -> Result<(), Box<dyn std::error::Error>> {
    let output = capy_command()?
        .args([
            "nextframe",
            "verify-export",
            "--composition",
            "/definitely/not/composition.json",
        ])
        .output()?;

    assert!(!output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(value["ok"], false);
    assert_eq!(value["stage"], "verify-export");
    assert_eq!(value["verdict"], "failed");
    assert_eq!(
        value["stages"]["validate"]["errors"][0]["code"],
        "COMPOSITION_NOT_FOUND"
    );
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
