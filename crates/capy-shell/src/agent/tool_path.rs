use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

const GUI_TOOL_PATH_DIRS: &[&str] = &[
    "/opt/homebrew/bin",
    "/usr/local/bin",
    "/opt/local/bin",
    "/usr/bin",
    "/bin",
    "/usr/sbin",
    "/sbin",
];

#[derive(Debug, Clone)]
pub(super) struct ToolLaunch {
    program: PathBuf,
    path_env: String,
}

impl ToolLaunch {
    pub(super) fn program(&self) -> &Path {
        &self.program
    }

    pub(super) fn path_env(&self) -> &str {
        &self.path_env
    }

    pub(super) fn display(&self) -> String {
        self.program.display().to_string()
    }
}

pub(super) fn tool_version(bin: &str, args: &[&str]) -> Value {
    let launch = tool_launch(bin);
    match Command::new(launch.program())
        .env("PATH", launch.path_env())
        .args(args)
        .output()
    {
        Ok(output) => json!({
            "available": output.status.success(),
            "version": String::from_utf8_lossy(&output.stdout).trim(),
            "error": String::from_utf8_lossy(&output.stderr).trim()
        }),
        Err(err) => json!({ "available": false, "error": err.to_string() }),
    }
}

pub(super) fn tool_launch(bin: &str) -> ToolLaunch {
    let path_env = desktop_tool_path_env();
    let program = resolve_tool_path(bin, &path_env).unwrap_or_else(|| PathBuf::from(bin));
    ToolLaunch { program, path_env }
}

pub(super) fn resolve_tool_path(bin: &str, path_env: &str) -> Option<PathBuf> {
    let path = Path::new(bin);
    if path.is_absolute() || bin.contains('/') {
        return path.is_file().then(|| path.to_path_buf());
    }
    env::split_paths(path_env)
        .map(|dir| dir.join(bin))
        .find(|candidate| candidate.is_file())
}

pub(super) fn desktop_tool_path_env() -> String {
    let mut dirs: Vec<PathBuf> = env::var_os("PATH")
        .map(|value| env::split_paths(&value).collect())
        .unwrap_or_default();
    for fallback in GUI_TOOL_PATH_DIRS {
        push_unique_path(&mut dirs, PathBuf::from(fallback));
    }
    if let Some(home) = env::var_os("HOME") {
        let home = PathBuf::from(home);
        push_unique_path(&mut dirs, home.join(".local/bin"));
        push_unique_path(&mut dirs, home.join(".cargo/bin"));
    } else if let Some(user) = env::var_os("USER") {
        let home = PathBuf::from("/Users").join(user);
        push_unique_path(&mut dirs, home.join(".local/bin"));
        push_unique_path(&mut dirs, home.join(".cargo/bin"));
    }
    env::join_paths(dirs)
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

fn push_unique_path(dirs: &mut Vec<PathBuf>, dir: PathBuf) {
    if !dirs.iter().any(|existing| existing == &dir) {
        dirs.push(dir);
    }
}
