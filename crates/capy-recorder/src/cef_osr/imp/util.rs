use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use capy_shell_mac::webview::pump_main_run_loop;
use serde_json::Value;

use super::RUN_LOOP_TICK;
use crate::cef_osr::CefOsrError;

pub(super) fn wait_until<F>(timeout: Duration, mut predicate: F) -> Result<(), CefOsrError>
where
    F: FnMut() -> Result<bool, CefOsrError>,
{
    let started = Instant::now();
    while started.elapsed() < timeout {
        if predicate()? {
            return Ok(());
        }
        pump_main_run_loop(RUN_LOOP_TICK);
    }
    Err(CefOsrError::PaintTimeout(format!(
        "timed out after {}ms",
        timeout.as_millis()
    )))
}

pub(super) fn js_number_as_u64(v: Option<&Value>) -> Option<u64> {
    let v = v?;
    if let Some(u) = v.as_u64() {
        return Some(u);
    }
    if let Some(i) = v.as_i64() {
        if i >= 0 {
            return Some(i as u64);
        }
    }
    if let Some(f) = v.as_f64() {
        if f.is_finite() && f >= 0.0 && f.fract() == 0.0 && f <= u64::MAX as f64 {
            return Some(f as u64);
        }
    }
    None
}

pub(super) fn path_to_string(path: &Path) -> Result<String, CefOsrError> {
    path.to_str()
        .map(str::to_string)
        .ok_or_else(|| CefOsrError::Io(format!("path is not valid UTF-8: {}", path.display())))
}

pub(super) fn browser_subprocess_path() -> Result<Option<String>, CefOsrError> {
    if let Ok(helper) = std::env::var("NF_CEF_HELPER") {
        return Ok(Some(helper));
    }
    let Some(path) = default_macos_helper_path() else {
        return Ok(None);
    };
    Ok(Some(path_to_string(&path)?))
}

fn default_macos_helper_path() -> Option<PathBuf> {
    #[cfg(not(target_os = "macos"))]
    {
        return None;
    }

    #[cfg(target_os = "macos")]
    {
        let exe = std::env::current_exe().ok()?;
        let exe_name = exe.file_name()?.to_str()?;
        let contents_dir = exe.parent()?.parent()?;
        if contents_dir.file_name()?.to_str()? != "Contents" {
            return None;
        }
        let helper_name = format!("{exe_name} Helper");
        let helper = contents_dir
            .join("Frameworks")
            .join(format!("{helper_name}.app"))
            .join("Contents")
            .join("MacOS")
            .join(helper_name);
        helper.exists().then_some(helper)
    }
}

pub(super) fn create_temp_dir(prefix: &str) -> Result<PathBuf, String> {
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("{prefix}-{pid}-{nanos}"));
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    std::fs::canonicalize(&dir).map_err(|err| err.to_string())
}

pub(super) fn file_url(path: &Path) -> Result<String, CefOsrError> {
    let abs = std::fs::canonicalize(path).map_err(|err| CefOsrError::Io(err.to_string()))?;
    let raw = abs.to_string_lossy();
    let mut out = String::from("file://");
    for b in raw.as_bytes() {
        match *b {
            b'/' | b'-' | b'_' | b'.' | b'~' => out.push(*b as char),
            b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' => out.push(*b as char),
            other => {
                out.push('%');
                out.push_str(&format!("{other:02X}"));
            }
        }
    }
    Ok(out)
}
