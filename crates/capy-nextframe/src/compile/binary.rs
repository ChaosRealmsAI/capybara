use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::Value;

use crate::compile::report::CompileError;
use crate::config::resolve_binary;

pub fn compile_with_binary(composition_path: &Path, render_source_path: &Path) -> BinaryCompile {
    let resolved = match resolve_binary(None, "CAPY_NF", "nf") {
        Ok(resolved) => resolved,
        Err(err) => {
            return BinaryCompile::Failed(CompileError::new(
                "NEXTFRAME_NOT_FOUND",
                "$.binary",
                err.body.message,
                with_next_step(err.body.hint),
            ));
        }
    };
    if !resolved.found {
        return BinaryCompile::Missing;
    }
    let Some(nf) = resolved.path else {
        return BinaryCompile::Missing;
    };
    run_nf_compile(&nf, composition_path, render_source_path)
}

fn run_nf_compile(nf: &Path, composition_path: &Path, render_source_path: &Path) -> BinaryCompile {
    let args = vec![
        "composition".to_string(),
        "compile".to_string(),
        composition_path.display().to_string(),
        "--out".to_string(),
        render_source_path.display().to_string(),
    ];
    let output = match Command::new(nf).args(&args).output() {
        Ok(output) => output,
        Err(err) => {
            return BinaryCompile::Failed(CompileError::new(
                "NEXTFRAME_NOT_FOUND",
                "$.binary",
                format!("spawn {} failed: {err}", nf.display()),
                "next step · rerun capy nextframe doctor and verify CAPY_NF",
            ));
        }
    };
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !output.status.success() {
        return BinaryCompile::Failed(CompileError::new(
            "COMPILE_FAILED",
            "$.binary",
            format!("nf composition compile failed: {stderr}"),
            "next step · inspect nf stderr and rerun capy nextframe validate",
        ));
    }
    copy_stdout_out_if_needed(&stdout, render_source_path)
}

fn copy_stdout_out_if_needed(stdout: &str, render_source_path: &Path) -> BinaryCompile {
    if let Some(path) = out_path_from_stdout(stdout) {
        if !render_source_path.is_file() && path != render_source_path && path.is_file() {
            if let Err(err) = copy_render_source(&path, render_source_path) {
                return BinaryCompile::Failed(err);
            }
        }
    }
    if !render_source_path.is_file() {
        return BinaryCompile::Failed(CompileError::new(
            "COMPILE_FAILED",
            "$.render_source_path",
            format!(
                "nf compile completed but render source was not written: {}",
                render_source_path.display()
            ),
            "next step · rerun compile without --strict-binary or check nf compile output",
        ));
    }
    BinaryCompile::Compiled
}

pub enum BinaryCompile {
    Compiled,
    Missing,
    Failed(CompileError),
}

fn copy_render_source(from: &Path, to: &Path) -> Result<(), CompileError> {
    if let Some(parent) = to.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|err| {
            CompileError::new(
                "COMPILE_FAILED",
                "$.render_source_path",
                format!("create render_source parent failed: {err}"),
                "next step · check output directory permissions",
            )
        })?;
    }
    fs::copy(from, to).map(|_| ()).map_err(|err| {
        CompileError::new(
            "COMPILE_FAILED",
            "$.render_source_path",
            format!("copy binary render_source failed: {err}"),
            "next step · check output directory permissions",
        )
    })
}

fn out_path_from_stdout(stdout: &str) -> Option<std::path::PathBuf> {
    let value: Value = serde_json::from_str(stdout).ok()?;
    value
        .get("out")
        .and_then(Value::as_str)
        .map(std::path::PathBuf::from)
}

fn with_next_step(hint: String) -> String {
    if hint.contains("next step ·") {
        hint
    } else {
        format!("next step · {hint}")
    }
}
