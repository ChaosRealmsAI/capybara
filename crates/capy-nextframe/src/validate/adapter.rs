use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::adapter::crate_adapter::CrateAdapter;
use crate::config::NextFrameConfig;
use crate::ports::{CompositionArtifact, NextFrameProjectPort};
use crate::validate::report::{BinaryPassthroughResult, ValidationError, ValidationReport};

pub fn append_crate_passthrough(report: &mut ValidationReport) {
    if !report.errors.is_empty() {
        return;
    }

    let result = validate_with_port(
        &CrateAdapter::new(NextFrameConfig::default()),
        &report.composition_path,
    );
    if !result.ok {
        if let Some(error) = result.error.clone() {
            report.push_error(error);
        }
    }
    report.binary_passthrough = Some(result);
    report.refresh_ok();
}

pub fn validate_with_port(
    port: &dyn NextFrameProjectPort,
    composition_path: &Path,
) -> BinaryPassthroughResult {
    let artifact = artifact_for_path(composition_path);
    match port.validate(&artifact) {
        Ok(report) => BinaryPassthroughResult {
            ok: report.ok,
            command: report.command,
            stdout_json: serde_json::from_str::<Value>(&report.stdout).ok(),
            stdout: report.stdout,
            stderr: report.stderr,
            error: (!report.ok).then(|| {
                ValidationError::new(
                    "COMPOSITION_INVALID",
                    "$.binary_passthrough",
                    "NextFrame project adapter rejected the composition",
                    "next step · inspect adapter stdout and rerun capy nextframe validate",
                )
            }),
        },
        Err(err) => BinaryPassthroughResult {
            ok: false,
            command: Vec::new(),
            stdout: String::new(),
            stderr: String::new(),
            stdout_json: None,
            error: Some(error_from_body(
                "$.binary_passthrough",
                err.body.code,
                err.body.message,
                err.body.hint,
            )),
        },
    }
}

fn artifact_for_path(composition_path: &Path) -> CompositionArtifact {
    let composition_id = composition_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.trim().is_empty())
        .unwrap_or("composition")
        .to_string();
    let project_root = composition_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let project_slug = project_root
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("capy-nextframe")
        .to_string();

    CompositionArtifact {
        project_slug,
        composition_id,
        project_root,
        composition_path: composition_path.to_path_buf(),
        component_paths: Vec::new(),
    }
}

fn error_from_body(
    path: impl Into<String>,
    code: impl Into<String>,
    message: impl Into<String>,
    hint: impl Into<String>,
) -> ValidationError {
    ValidationError::new(code, path, message, format!("next step · {}", hint.into()))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::error::{NextFrameError, NextFrameErrorCode};
    use crate::ports::{
        CompileReport, CompositionArtifact, NextFrameProjectPort, ValidationReport,
    };

    use super::validate_with_port;

    #[test]
    fn crate_passthrough_reports_ok() {
        let result = validate_with_port(
            &MockProjectPort::Ok,
            Path::new("target/sample/composition.json"),
        );

        assert!(result.ok);
        assert_eq!(result.command, vec!["nf", "composition", "validate"]);
        assert_eq!(
            result
                .stdout_json
                .as_ref()
                .and_then(|value| value.get("ok"))
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );
    }

    #[test]
    fn crate_passthrough_reports_failure() {
        let result = validate_with_port(
            &MockProjectPort::Fail,
            Path::new("target/sample/composition.json"),
        );

        assert!(!result.ok);
        assert_eq!(
            result.error.as_ref().map(|error| error.code.as_str()),
            Some("COMPOSITION_INVALID")
        );
    }

    #[test]
    fn crate_passthrough_reports_not_found() {
        let result = validate_with_port(
            &MockProjectPort::NotFound,
            Path::new("target/sample/composition.json"),
        );

        assert!(!result.ok);
        assert_eq!(
            result.error.as_ref().map(|error| error.code.as_str()),
            Some("NEXTFRAME_NOT_FOUND")
        );
    }

    enum MockProjectPort {
        Ok,
        Fail,
        NotFound,
    }

    impl NextFrameProjectPort for MockProjectPort {
        fn validate(
            &self,
            _artifact: &CompositionArtifact,
        ) -> Result<ValidationReport, NextFrameError> {
            match self {
                Self::Ok => Ok(ValidationReport {
                    ok: true,
                    command: vec![
                        "nf".to_string(),
                        "composition".to_string(),
                        "validate".to_string(),
                    ],
                    stdout: "{\"ok\":true}".to_string(),
                    stderr: String::new(),
                }),
                Self::Fail => Err(NextFrameError::new(
                    NextFrameErrorCode::CompositionInvalid,
                    "composition failed",
                    "inspect stderr",
                )),
                Self::NotFound => Err(NextFrameError::not_found("nf missing")),
            }
        }

        fn compile(
            &self,
            _artifact: &CompositionArtifact,
            _out: &Path,
        ) -> Result<CompileReport, NextFrameError> {
            Err(NextFrameError::new(
                NextFrameErrorCode::CompileFailed,
                "not implemented",
                "not used",
            ))
        }
    }
}
