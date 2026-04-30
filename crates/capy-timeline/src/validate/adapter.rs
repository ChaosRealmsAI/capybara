use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::adapters::timeline_engine::TimelineAdapter;
use crate::config::TimelineConfig;
use crate::ports::{CompositionArtifact, TimelineProjectPort};
use crate::validate::report::{BinaryPassthroughResult, ValidationError, ValidationReport};

pub fn append_crate_passthrough(report: &mut ValidationReport) {
    if !report.errors.is_empty() {
        return;
    }

    let result = validate_with_port(
        &TimelineAdapter::new(TimelineConfig::default()),
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
    port: &dyn TimelineProjectPort,
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
                    "Timeline project adapter rejected the composition",
                    "next step · inspect adapter stdout and rerun capy timeline validate",
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
    let composition_dir = composition_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let project_root =
        if composition_dir.file_name().and_then(|name| name.to_str()) == Some("compositions") {
            composition_dir
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or(composition_dir)
        } else {
            composition_dir
        };
    let project_slug = project_root
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("capy-timeline")
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

    use crate::error::{TimelineError, TimelineErrorCode};
    use crate::ports::{CompileReport, CompositionArtifact, TimelineProjectPort, ValidationReport};

    use super::validate_with_port;

    #[test]
    fn crate_passthrough_reports_ok() {
        let result = validate_with_port(
            &MockProjectPort::Ok,
            Path::new("target/sample/composition.json"),
        );

        assert!(result.ok);
        assert_eq!(
            result.command,
            vec!["capy-timeline-project", "composition", "validate"]
        );
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
            Some("TIMELINE_NOT_FOUND")
        );
    }

    enum MockProjectPort {
        Ok,
        Fail,
        NotFound,
    }

    impl TimelineProjectPort for MockProjectPort {
        fn validate(
            &self,
            _artifact: &CompositionArtifact,
        ) -> Result<ValidationReport, TimelineError> {
            match self {
                Self::Ok => Ok(ValidationReport {
                    ok: true,
                    command: vec![
                        "capy-timeline-project".to_string(),
                        "composition".to_string(),
                        "validate".to_string(),
                    ],
                    stdout: "{\"ok\":true}".to_string(),
                    stderr: String::new(),
                }),
                Self::Fail => Err(TimelineError::new(
                    TimelineErrorCode::CompositionInvalid,
                    "composition failed",
                    "inspect stderr",
                )),
                Self::NotFound => Err(TimelineError::not_found("timeline engine missing")),
            }
        }

        fn compile(
            &self,
            _artifact: &CompositionArtifact,
            _out: &Path,
        ) -> Result<CompileReport, TimelineError> {
            Err(TimelineError::new(
                TimelineErrorCode::CompileFailed,
                "not implemented",
                "not used",
            ))
        }
    }
}
