use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use wef::Settings;

pub struct CefRuntime {
    #[cfg(target_os = "macos")]
    _loader: wef::FrameworkLoader,
    cache_dir: PathBuf,
}

impl Drop for CefRuntime {
    fn drop(&mut self) {
        wef::shutdown();
        let _remove_result = std::fs::remove_dir_all(&self.cache_dir);
    }
}

pub fn maybe_run_cef_subprocess() -> Result<bool, String> {
    if !std::env::args().any(|arg| arg.starts_with("--type=") || arg == "--type") {
        return Ok(false);
    }
    #[cfg(target_os = "macos")]
    let _sandbox = wef::SandboxContext::new().map_err(|err| err.to_string())?;
    #[cfg(target_os = "macos")]
    let _loader = wef::FrameworkLoader::load_in_helper().map_err(|err| err.to_string())?;
    wef::exec_process().map_err(|err| err.to_string())
}

pub fn init_cef_runtime() -> Result<CefRuntime, String> {
    let cache_dir = create_temp_dir("capy-shell-cef")?;
    #[cfg(target_os = "macos")]
    let loader = wef::FrameworkLoader::load_in_main().map_err(|err| err.to_string())?;

    let mut settings = Settings::new()
        .disable_gpu(false)
        .root_cache_path(path_to_string(&cache_dir)?)
        .cache_path(path_to_string(&cache_dir.join("profile"))?);
    if let Some(helper) = browser_subprocess_path()? {
        settings = settings.browser_subprocess_path(helper);
    }
    wef::init(settings).map_err(|err| err.to_string())?;

    Ok(CefRuntime {
        #[cfg(target_os = "macos")]
        _loader: loader,
        cache_dir,
    })
}

fn create_temp_dir(prefix: &str) -> Result<PathBuf, String> {
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("{prefix}-{pid}-{nanos}"));
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    std::fs::canonicalize(&dir).map_err(|err| err.to_string())
}

fn path_to_string(path: &Path) -> Result<String, String> {
    path.to_str()
        .map(str::to_string)
        .ok_or_else(|| format!("path is not valid UTF-8: {}", path.display()))
}

fn browser_subprocess_path() -> Result<Option<String>, String> {
    if let Ok(helper) = std::env::var("CAPY_CEF_HELPER") {
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
