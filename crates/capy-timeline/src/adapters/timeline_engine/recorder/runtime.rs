use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{TimelineError, TimelineErrorCode};
use crate::ports::ExportOptions;

pub(super) fn render_source_path(composition_path: &Path) -> PathBuf {
    composition_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("render_source.json")
}

pub(super) fn recorder_snapshot_command(source: &Path, out: &Path, t_ms: u64) -> Vec<String> {
    vec![
        "capy-recorder".to_string(),
        "snapshot-source".to_string(),
        "--source".to_string(),
        source.display().to_string(),
        "--t-ms".to_string(),
        t_ms.to_string(),
        "--output".to_string(),
        out.display().to_string(),
    ]
}

pub(super) fn recorder_export_command(
    source: &Path,
    out: &Path,
    options: &ExportOptions,
) -> Result<Vec<String>, TimelineError> {
    Ok(build_recorder_export_command(
        recorder_binary_for_export(source)?,
        source,
        out,
        options,
    ))
}

pub(super) fn build_recorder_export_command(
    binary: PathBuf,
    source: &Path,
    out: &Path,
    options: &ExportOptions,
) -> Vec<String> {
    let mut command = vec![
        binary.display().to_string(),
        "export".to_string(),
        "--source".to_string(),
        source.display().to_string(),
        "--profile".to_string(),
        options.profile.clone(),
        "--output".to_string(),
        out.display().to_string(),
        "--fps".to_string(),
        options.fps.max(1).to_string(),
    ];
    if let Some(resolution) = options
        .resolution
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        command.extend(["--resolution".to_string(), resolution.to_string()]);
    }
    if let Some(parallel) = options.parallel.filter(|value| *value > 0) {
        command.extend(["--parallel".to_string(), parallel.to_string()]);
    }
    command
}

#[cfg(target_os = "macos")]
fn recorder_binary_for_export(source: &Path) -> Result<PathBuf, TimelineError> {
    if let Some(raw) = std::env::var_os("CAPY_RECORDER").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(raw));
    }

    let app = capy_shell_app_for_source(source)?;
    let recorder = app.join("Contents").join("MacOS").join("capy-recorder");
    if recorder.is_file() {
        return Ok(recorder);
    }

    for root in candidate_roots(source) {
        for candidate in [
            root.join("crates/capy-recorder/target/debug/capy-recorder"),
            root.join("crates/capy-recorder/target/release/capy-recorder"),
        ] {
            if candidate.is_file() {
                fs::copy(&candidate, &recorder).map_err(|err| {
                    TimelineError::new(
                        TimelineErrorCode::ExportFailed,
                        format!(
                            "stage capy-recorder into app bundle failed: {} -> {}: {err}",
                            candidate.display(),
                            recorder.display()
                        ),
                        "next step · check target/debug/capy-shell.app permissions",
                    )
                })?;
                return Ok(recorder);
            }
        }
    }

    Err(TimelineError::new(
        TimelineErrorCode::ExportFailed,
        "capy-recorder app-bundle binary is missing",
        "next step · run `cargo build --manifest-path crates/capy-recorder/Cargo.toml --features cef-osr`, then rerun export",
    ))
}

#[cfg(not(target_os = "macos"))]
fn recorder_binary_for_export(_source: &Path) -> Result<PathBuf, TimelineError> {
    Ok(PathBuf::from("capy-recorder"))
}

#[cfg(target_os = "macos")]
pub(super) fn capy_shell_helper_path_for_recorder(
    recorder: &Path,
) -> Result<Option<String>, TimelineError> {
    if let Some(raw) = std::env::var_os("NF_CEF_HELPER").filter(|value| !value.is_empty()) {
        return Ok(Some(PathBuf::from(raw).display().to_string()));
    }
    let Some(mac_os_dir) = recorder.parent() else {
        return Ok(None);
    };
    let Some(contents_dir) = mac_os_dir.parent() else {
        return Ok(None);
    };
    let helper = contents_dir
        .join("Frameworks")
        .join("capy-shell Helper.app")
        .join("Contents")
        .join("MacOS")
        .join("capy-shell Helper");
    if helper.is_file() {
        Ok(Some(helper.display().to_string()))
    } else {
        Err(TimelineError::new(
            TimelineErrorCode::ExportFailed,
            format!("CEF helper is missing: {}", helper.display()),
            "next step · rebuild or restage target/debug/capy-shell.app",
        ))
    }
}

#[cfg(not(target_os = "macos"))]
pub(super) fn capy_shell_helper_path_for_recorder(
    _recorder: &Path,
) -> Result<Option<String>, TimelineError> {
    Ok(None)
}

#[cfg(target_os = "macos")]
fn capy_shell_app_for_source(source: &Path) -> Result<PathBuf, TimelineError> {
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            candidates.push(parent.join("capy-shell.app"));
        }
    }
    for root in candidate_roots(source) {
        candidates.push(root.join("target/debug/capy-shell.app"));
    }
    for app in candidates {
        if app
            .join("Contents/Frameworks/Chromium Embedded Framework.framework")
            .is_dir()
        {
            return Ok(app);
        }
    }
    Err(TimelineError::new(
        TimelineErrorCode::ExportFailed,
        "capy-shell.app with CEF framework was not found",
        "next step · run scripts/open-debug-shell.sh once to stage the CEF app bundle",
    ))
}

#[cfg(target_os = "macos")]
fn candidate_roots(source: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        push_root_candidates(&mut roots, &cwd);
    }
    if let Ok(abs) = fs::canonicalize(source) {
        if let Some(parent) = abs.parent() {
            push_root_candidates(&mut roots, parent);
        }
    }
    roots
}

#[cfg(target_os = "macos")]
fn push_root_candidates(roots: &mut Vec<PathBuf>, start: &Path) {
    for candidate in start.ancestors() {
        if candidate.join("Cargo.toml").is_file()
            && candidate.join("crates/capy-recorder").is_dir()
            && !roots.iter().any(|root| root == candidate)
        {
            roots.push(candidate.to_path_buf());
        }
    }
}

pub(super) fn stderr_tail(stderr: &str) -> String {
    let value = tail(stderr);
    if value.is_empty() {
        String::new()
    } else {
        format!("stderr: {value}")
    }
}

pub(super) fn stdout_tail(stdout: &str) -> String {
    let value = tail(stdout);
    if value.is_empty() {
        String::new()
    } else {
        format!(" stdout: {value}")
    }
}

fn tail(value: &str) -> String {
    let trimmed = value.trim();
    let chars: Vec<char> = trimmed.chars().collect();
    if chars.len() <= 600 {
        trimmed.to_string()
    } else {
        chars[chars.len().saturating_sub(600)..].iter().collect()
    }
}

#[cfg(target_os = "macos")]
pub(super) fn recorder_runtime() -> Result<tokio::runtime::Runtime, TimelineError> {
    if !current_exe_is_macos_app_bundle() {
        return Err(TimelineError::new(
            TimelineErrorCode::TimelineNotFound,
            "capy-recorder crate mode requires a macOS app bundle CEF runtime",
            "embedded mode required",
        ));
    }
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| {
            TimelineError::new(
                TimelineErrorCode::TimelineNotFound,
                format!("create capy-recorder runtime failed: {err}"),
                "embedded mode required",
            )
        })
}

#[cfg(target_os = "macos")]
pub(super) fn current_exe_is_macos_app_bundle() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|exe| {
            let contents_dir = exe.parent()?.parent()?;
            let app_dir = contents_dir.parent()?;
            let has_contents = contents_dir.file_name()?.to_str()? == "Contents";
            let has_app_extension = app_dir.extension()?.to_str()? == "app";
            Some(has_contents && has_app_extension)
        })
        .unwrap_or(false)
}

pub(super) fn ensure_parent(path: &Path, code: TimelineErrorCode) -> Result<(), TimelineError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|err| {
            TimelineError::new(
                code,
                format!("create output parent failed: {err}"),
                "next step · check output directory permissions",
            )
        })?;
    }
    Ok(())
}
