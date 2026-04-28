use std::path::Path;
use std::process::Command;

use crate::config::{NextFrameConfig, resolve_binary};

use super::report::ExportError;

pub enum BinaryExport {
    Exported,
    Missing,
    Failed(ExportError),
}

pub fn export_with_binary(render_source_path: &Path, out: &Path, fps: u32) -> BinaryExport {
    let recorder = match resolve_binary(
        NextFrameConfig::default().recorder_bin,
        "CAPY_NF_RECORDER",
        "nf-recorder",
    ) {
        Ok(recorder) if recorder.found => recorder,
        Ok(_) => return BinaryExport::Missing,
        Err(err) => {
            return BinaryExport::Failed(ExportError::new(
                err.body.code,
                "$.binary",
                err.body.message,
                with_next_step(err.body.hint),
            ));
        }
    };
    let Some(program) = recorder.path else {
        return BinaryExport::Missing;
    };
    let args = [
        "export",
        "--source",
        &render_source_path.display().to_string(),
        "--profile",
        "draft",
        "--output",
        &out.display().to_string(),
        "--fps",
        &fps.to_string(),
    ];
    let output = match Command::new(&program).args(args).output() {
        Ok(output) => output,
        Err(err) => {
            return BinaryExport::Failed(ExportError::new(
                "EXPORT_FAILED",
                "$.binary",
                format!("spawn {} failed: {err}", program.display()),
                "next step · rerun capy nextframe doctor",
            ));
        }
    };
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return BinaryExport::Failed(ExportError::new(
            "EXPORT_FAILED",
            "$.binary",
            if stderr.is_empty() {
                format!("nf-recorder export failed: {}", output.status)
            } else {
                stderr
            },
            "next step · inspect nf-recorder export stderr",
        ));
    }
    BinaryExport::Exported
}

fn with_next_step(hint: String) -> String {
    if hint.contains("next step ·") {
        hint
    } else {
        format!("next step · {hint}")
    }
}
