use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::config::{ResolvedBinary, TimelineConfig};
use crate::error::{ErrorBody, TimelineError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DoctorReport {
    pub ok: bool,
    pub trace_id: String,
    pub stage: &'static str,
    pub recorder: BinaryReport,
    pub mode: &'static str,
    pub config: DoctorConfigReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorBody>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BinaryReport {
    pub found: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DoctorConfigReport {
    pub discovery: String,
}

pub fn doctor(config: TimelineConfig) -> DoctorReport {
    let trace_id = trace_id();
    match config.resolve() {
        Ok(resolved) => {
            let recorder = resolved.recorder;
            DoctorReport {
                ok: true,
                trace_id,
                stage: "doctor",
                config: DoctorConfigReport {
                    discovery: if recorder.found {
                        recorder.discovery.as_str().to_string()
                    } else {
                        "MISSING".to_string()
                    },
                },
                recorder: BinaryReport::from(recorder),
                mode: "crate-only",
                error: None,
            }
        }
        Err(err) => failed_report(trace_id, err),
    }
}

fn failed_report(trace_id: String, err: TimelineError) -> DoctorReport {
    DoctorReport {
        ok: false,
        trace_id,
        stage: "doctor",
        recorder: BinaryReport {
            found: false,
            path: None,
            version: None,
        },
        mode: "crate-only",
        config: DoctorConfigReport {
            discovery: "MISSING".to_string(),
        },
        error: Some(err.body),
    }
}

fn trace_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!("doctor-{millis}-{}", std::process::id())
}

impl From<ResolvedBinary> for BinaryReport {
    fn from(value: ResolvedBinary) -> Self {
        Self {
            found: value.found,
            path: value.path,
            version: value.version,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::config::TimelineConfig;
    use crate::doctor::doctor;

    #[test]
    fn doctor_reports_crate_only_when_binaries_are_missing() -> Result<(), String> {
        let report = doctor(TimelineConfig {
            recorder_bin: Some("/definitely/not/capy-recorder".into()),
            home: None,
        });

        assert!(report.ok);
        assert_eq!(report.stage, "doctor");
        assert_eq!(report.config.discovery, "MISSING");
        assert_eq!(report.mode, "crate-only");
        assert!(report.error.is_none());
        Ok(())
    }
}
