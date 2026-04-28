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

fn capy_command() -> Result<Command, Box<dyn std::error::Error>> {
    let path = std::env::var("CARGO_BIN_EXE_capy")?;
    Ok(Command::new(path))
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
