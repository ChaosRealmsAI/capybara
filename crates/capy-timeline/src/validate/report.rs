use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationReport {
    pub ok: bool,
    pub trace_id: String,
    pub stage: String,
    pub composition_path: PathBuf,
    pub schema_version: String,
    pub track_count: usize,
    pub asset_count: usize,
    pub components: Vec<String>,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
    pub binary_passthrough: Option<BinaryPassthroughResult>,
}

impl ValidationReport {
    pub fn new(composition_path: PathBuf, trace_id: String) -> Self {
        Self {
            ok: true,
            trace_id,
            stage: "validate".to_string(),
            composition_path,
            schema_version: String::new(),
            track_count: 0,
            asset_count: 0,
            components: Vec::new(),
            errors: Vec::new(),
            warnings: Vec::new(),
            binary_passthrough: None,
        }
    }

    pub fn push_error(&mut self, error: ValidationError) {
        self.ok = false;
        self.errors.push(error);
    }

    pub fn refresh_ok(&mut self) {
        self.ok = self.errors.is_empty()
            && self
                .binary_passthrough
                .as_ref()
                .map(|result| result.ok)
                .unwrap_or(true);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationError {
    pub code: String,
    pub path: String,
    pub message: String,
    pub hint: String,
}

impl ValidationError {
    pub fn new(
        code: impl Into<String>,
        path: impl Into<String>,
        message: impl Into<String>,
        hint: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            path: path.into(),
            message: message.into(),
            hint: hint.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationWarning {
    pub code: String,
    pub path: String,
    pub message: String,
}

impl ValidationWarning {
    pub fn new(
        code: impl Into<String>,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            path: path.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BinaryPassthroughResult {
    pub ok: bool,
    pub command: Vec<String>,
    pub stdout: String,
    pub stderr: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout_json: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ValidationError>,
}

#[cfg(test)]
mod tests {
    use super::{ValidationError, ValidationReport, ValidationWarning};

    #[test]
    fn validation_report_serializes_round_trip() -> Result<(), serde_json::Error> {
        let mut report =
            ValidationReport::new("target/composition.json".into(), "validate-123".to_string());
        report.schema_version = "capy.composition.v1".to_string();
        report.track_count = 1;
        report.asset_count = 2;
        report.components = vec!["html.capy-poster".to_string()];
        report.warnings.push(ValidationWarning::new(
            "TRACE_ID_GENERATED",
            "$.trace_id",
            "trace_id was generated for this validation run",
        ));
        report.push_error(ValidationError::new(
            "EMPTY_TRACKS",
            "$.tracks",
            "composition must include at least one track",
            "next step · rerun compose-poster",
        ));

        let text = serde_json::to_string(&report)?;
        let decoded: ValidationReport = serde_json::from_str(&text)?;

        assert_eq!(decoded, report);
        assert!(!decoded.ok);
        assert_eq!(decoded.errors[0].code, "EMPTY_TRACKS");
        Ok(())
    }
}
