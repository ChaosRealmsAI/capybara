//! Local whisperX runtime and alignment model cache helpers.

use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Value, json};

mod models;

use models::{ALIGN_MODELS, DEFAULT_LANGUAGES};

pub(crate) const PYTHON_PACKAGES: &[&str] = &[
    "whisperx==3.8.5",
    "huggingface_hub==0.36.2",
    "torch==2.8.0",
    "torchaudio==2.8.0",
];

#[derive(Debug, Clone)]
pub(crate) struct PythonRuntime {
    pub(crate) python: PathBuf,
    pub(crate) cache_dir: Option<PathBuf>,
    pub(crate) venv_dir: Option<PathBuf>,
    pub(crate) hf_home: Option<PathBuf>,
    pub(crate) source: RuntimeSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeSource {
    ExplicitPython,
    EnvPython,
    ManagedVenv,
    SystemPython,
}

impl RuntimeSource {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ExplicitPython => "explicit-python",
            Self::EnvPython => "env-python",
            Self::ManagedVenv => "managed-venv",
            Self::SystemPython => "system-python",
        }
    }
}

pub(crate) fn resolve_languages(requested: &[String], all: bool) -> Result<Vec<String>> {
    let mut languages: Vec<String> = if all {
        ALIGN_MODELS
            .iter()
            .map(|model| model.language.to_string())
            .collect()
    } else if requested.is_empty() {
        DEFAULT_LANGUAGES
            .iter()
            .map(|value| (*value).to_string())
            .collect()
    } else {
        requested
            .iter()
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .collect()
    };

    languages.sort();
    languages.dedup();

    if languages.is_empty() {
        bail!("no languages selected");
    }

    for language in &languages {
        if ALIGN_MODELS
            .iter()
            .all(|model| model.language != language.as_str())
        {
            bail!(
                "unsupported TTS alignment language '{language}'. Supported: {}",
                ALIGN_MODELS
                    .iter()
                    .map(|model| format!("{}={}", model.language, model.repo))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }

    Ok(languages)
}

pub(crate) fn existing_runtime(cache_dir: Option<&Path>, python: Option<&Path>) -> PythonRuntime {
    let cache_dir = cache_dir.map(Path::to_path_buf);
    let env_hf_home = env::var_os("CAPY_TTS_HF_HOME").map(PathBuf::from);

    if let Some(python) = python {
        let hf_home = cache_dir
            .as_ref()
            .map(|root| root.join("hf"))
            .or(env_hf_home);
        return PythonRuntime {
            python: python.to_path_buf(),
            cache_dir,
            venv_dir: None,
            hf_home,
            source: RuntimeSource::ExplicitPython,
        };
    }

    if let Some(python) = env::var_os("CAPY_TTS_PYTHON").map(PathBuf::from) {
        let hf_home = cache_dir
            .as_ref()
            .map(|root| root.join("hf"))
            .or(env_hf_home);
        return PythonRuntime {
            python,
            cache_dir,
            venv_dir: None,
            hf_home,
            source: RuntimeSource::EnvPython,
        };
    }

    let managed = managed_runtime(cache_dir.as_deref());
    if managed.python.is_file() || cache_dir.is_some() {
        return managed;
    }

    PythonRuntime {
        python: PathBuf::from("python3"),
        cache_dir: None,
        venv_dir: None,
        hf_home: env_hf_home,
        source: RuntimeSource::SystemPython,
    }
}

pub(crate) fn managed_runtime(cache_dir: Option<&Path>) -> PythonRuntime {
    let cache_dir = cache_dir
        .map(Path::to_path_buf)
        .unwrap_or_else(default_cache_dir);
    let venv_dir = cache_dir.join("venv");
    PythonRuntime {
        python: venv_python(&venv_dir),
        cache_dir: Some(cache_dir.clone()),
        venv_dir: Some(venv_dir),
        hf_home: Some(cache_dir.join("hf")),
        source: RuntimeSource::ManagedVenv,
    }
}

pub(crate) fn fixed_python_requested(python: Option<&Path>) -> bool {
    python.is_some() || env::var_os("CAPY_TTS_PYTHON").is_some()
}

pub(crate) fn align_script_path() -> Option<PathBuf> {
    if let Ok(path) = env::var("CAPY_TTS_ALIGN_SCRIPT") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Some(path);
        }
    }

    if let Some(manifest_dir) = option_env!("CARGO_MANIFEST_DIR") {
        let candidate = PathBuf::from(manifest_dir).join("scripts/align_ffa.py");
        if candidate.exists() {
            return Some(candidate);
        }
    }

    if let Ok(exe) = env::current_exe() {
        for parent in exe.ancestors() {
            let candidate = parent.join("scripts/align_ffa.py");
            if candidate.exists() {
                return Some(candidate);
            }

            let candidate = parent.join("capy-tts/scripts/align_ffa.py");
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    None
}

pub(crate) fn status(runtime: &PythonRuntime, languages: &[String]) -> Result<Value> {
    run_helper(runtime, "--status", languages)
}

pub(crate) fn download(runtime: &PythonRuntime, languages: &[String]) -> Result<Value> {
    run_helper(runtime, "--download", languages)
}

pub(crate) fn status_or_error(runtime: &PythonRuntime, languages: &[String]) -> Value {
    match status(runtime, languages) {
        Ok(value) => value,
        Err(error) => json!({
            "ok": false,
            "kind": "tts-align-status",
            "error": error.to_string(),
            "runtime": runtime_json(runtime),
        }),
    }
}

pub(crate) fn packages_ready(status: &Value) -> bool {
    let Some(packages) = status.get("packages").and_then(Value::as_array) else {
        return false;
    };
    packages.iter().all(|package| {
        package
            .get("available")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    })
}

pub(crate) fn missing_languages(status: &Value) -> Vec<String> {
    let Some(models) = status.get("models").and_then(Value::as_array) else {
        return Vec::new();
    };
    models
        .iter()
        .filter(|model| {
            !model
                .get("cached")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .filter_map(|model| {
            model
                .get("language")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
        .collect()
}

pub(crate) fn create_venv(runtime: &PythonRuntime) -> Result<bool> {
    let Some(venv_dir) = &runtime.venv_dir else {
        return Ok(false);
    };
    if runtime.python.is_file() {
        return Ok(false);
    }
    if let Some(cache_dir) = &runtime.cache_dir {
        std::fs::create_dir_all(cache_dir).with_context(|| {
            format!("failed to create TTS runtime cache {}", cache_dir.display())
        })?;
    }
    let bootstrap = env::var_os("CAPY_TTS_BOOTSTRAP_PYTHON")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("python3"));
    run_status(
        Command::new(&bootstrap).arg("-m").arg("venv").arg(venv_dir),
        "create TTS whisperX Python venv",
    )?;
    Ok(true)
}

pub(crate) fn install_packages(runtime: &PythonRuntime) -> Result<()> {
    let mut pip = Command::new(&runtime.python);
    pip.arg("-m")
        .arg("pip")
        .arg("install")
        .arg("--upgrade")
        .arg("pip")
        .args(PYTHON_PACKAGES);
    apply_runtime_env(&mut pip, runtime);
    run_status(&mut pip, "install TTS whisperX Python packages")
}

pub(crate) fn runtime_json(runtime: &PythonRuntime) -> Value {
    json!({
        "python": runtime.python,
        "source": runtime.source.as_str(),
        "cache_dir": runtime.cache_dir,
        "venv_dir": runtime.venv_dir,
        "hf_home": runtime.hf_home,
    })
}

pub(crate) fn apply_runtime_env(command: &mut Command, runtime: &PythonRuntime) {
    if let Some(hf_home) = &runtime.hf_home {
        command.env("HF_HOME", hf_home);
    }
    command.env("TOKENIZERS_PARALLELISM", "false");
    command.env("TRANSFORMERS_VERBOSITY", "error");
    command.env("HF_HUB_DISABLE_PROGRESS_BARS", "1");
}

fn run_helper(runtime: &PythonRuntime, operation: &str, languages: &[String]) -> Result<Value> {
    let script = align_script_path()
        .ok_or_else(|| anyhow!("scripts/align_ffa.py not found (set CAPY_TTS_ALIGN_SCRIPT)"))?;
    let mut command = Command::new(&runtime.python);
    command
        .arg(&script)
        .arg(operation)
        .args(languages)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    apply_runtime_env(&mut command, runtime);

    let output = command.output().with_context(|| {
        format!(
            "failed to run TTS align helper: python={} script={}",
            runtime.python.display(),
            script.display()
        )
    })?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if !output.status.success() {
        bail!(
            "TTS align helper {operation} failed: {}",
            if stderr.is_empty() { stdout } else { stderr }
        );
    }

    serde_json::from_str(&stdout)
        .with_context(|| format!("failed to parse TTS align helper JSON: {stdout}"))
}

fn run_status(command: &mut Command, label: &str) -> Result<()> {
    let output = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| format!("{label} failed to start"))?;
    if output.status.success() {
        return Ok(());
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    bail!(
        "{label} failed: {}",
        if stderr.is_empty() { stdout } else { stderr }
    )
}

#[cfg(windows)]
fn venv_python(venv_dir: &Path) -> PathBuf {
    venv_dir.join("Scripts").join("python.exe")
}

#[cfg(not(windows))]
fn venv_python(venv_dir: &Path) -> PathBuf {
    venv_dir.join("bin").join("python")
}

fn default_cache_dir() -> PathBuf {
    if let Some(value) = env::var_os("CAPY_TTS_RUNTIME_CACHE") {
        return PathBuf::from(value);
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = env::var_os("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Caches")
                .join("capybara")
                .join("tts")
                .join("whisperx");
        }
    }
    #[cfg(windows)]
    {
        if let Some(local) = env::var_os("LOCALAPPDATA") {
            return PathBuf::from(local)
                .join("Capybara")
                .join("tts")
                .join("whisperx");
        }
    }
    if let Some(home) = env::var_os("HOME") {
        return PathBuf::from(home)
            .join(".cache")
            .join("capybara")
            .join("tts")
            .join("whisperx");
    }
    PathBuf::from(".capybara-cache")
        .join("tts")
        .join("whisperx")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn resolve_languages_defaults_to_zh() {
        assert_eq!(resolve_languages(&[], false).unwrap(), vec!["zh"]);
    }

    #[test]
    fn resolve_languages_dedupes_and_sorts_requested() {
        let requested = vec!["en".to_string(), "zh".to_string(), "en".to_string()];
        assert_eq!(
            resolve_languages(&requested, false).unwrap(),
            vec!["en", "zh"]
        );
    }

    #[test]
    fn resolve_languages_rejects_unknown_values() {
        let requested = vec!["xx".to_string()];
        assert!(resolve_languages(&requested, false).is_err());
    }

    #[test]
    fn missing_languages_reads_helper_status_shape() {
        let status = json!({
            "models": [
                {"language": "zh", "cached": true},
                {"language": "en", "cached": false}
            ]
        });
        assert_eq!(missing_languages(&status), vec!["en"]);
    }

    #[test]
    fn packages_ready_requires_every_package() {
        assert!(packages_ready(&json!({
            "packages": [
                {"name": "whisperx", "available": true},
                {"name": "huggingface_hub", "available": true}
            ]
        })));
        assert!(!packages_ready(&json!({
            "packages": [
                {"name": "whisperx", "available": true},
                {"name": "huggingface_hub", "available": false}
            ]
        })));
    }

    #[test]
    fn existing_runtime_uses_cache_venv_when_cache_is_explicit() {
        let root = temp_dir("runtime-cache");
        let runtime = existing_runtime(Some(&root), None);
        let hf_home = root.join("hf");
        assert_eq!(runtime.source, RuntimeSource::ManagedVenv);
        assert_eq!(runtime.cache_dir.as_deref(), Some(root.as_path()));
        assert_eq!(runtime.hf_home.as_deref(), Some(hf_home.as_path()));
        let _ = fs::remove_dir_all(root);
    }

    fn temp_dir(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        let path = env::temp_dir().join(format!("capy-tts-{label}-{unique}"));
        fs::create_dir_all(&path).expect("create temp dir");
        path
    }
}
