use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::config::{NextFrameConfig, ResolvedBinary};
use crate::error::{ErrorBody, NextFrameError, NextFrameErrorCode, nextframe_setup_hint};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DoctorReport {
    pub ok: bool,
    pub trace_id: String,
    pub stage: &'static str,
    pub nf: BinaryReport,
    pub nf_recorder: BinaryReport,
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

pub fn doctor(config: NextFrameConfig) -> DoctorReport {
    let trace_id = trace_id();
    match config.resolve() {
        Ok(resolved) => {
            let mode = resolved.mode;
            let nf = enrich_version(resolved.nf, &["--version"]);
            let recorder = enrich_version(resolved.recorder, &["--version"]);
            let ok = mode == crate::config::NextFrameMode::Crate || (nf.found && recorder.found);
            let error = if ok {
                None
            } else {
                Some(ErrorBody::new(
                    NextFrameErrorCode::NextframeNotFound,
                    "nf or nf-recorder was not found",
                    nextframe_setup_hint(),
                ))
            };
            DoctorReport {
                ok,
                trace_id,
                stage: "doctor",
                config: DoctorConfigReport {
                    discovery: combined_discovery(&nf, &recorder).to_string(),
                },
                nf: BinaryReport::from(nf),
                nf_recorder: BinaryReport::from(recorder),
                mode: mode.as_str(),
                error,
            }
        }
        Err(err) => failed_report(trace_id, err),
    }
}

fn failed_report(trace_id: String, err: NextFrameError) -> DoctorReport {
    DoctorReport {
        ok: false,
        trace_id,
        stage: "doctor",
        nf: BinaryReport {
            found: false,
            path: None,
            version: None,
        },
        nf_recorder: BinaryReport {
            found: false,
            path: None,
            version: None,
        },
        mode: "binary",
        config: DoctorConfigReport {
            discovery: "MISSING".to_string(),
        },
        error: Some(err.body),
    }
}

fn enrich_version(binary: ResolvedBinary, args: &[&str]) -> ResolvedBinary {
    if !binary.found {
        return binary;
    }
    let version = binary.path.as_ref().and_then(|path| {
        Command::new(path)
            .args(args)
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
            .filter(|value| !value.is_empty())
    });
    ResolvedBinary { version, ..binary }
}

fn combined_discovery(nf: &ResolvedBinary, recorder: &ResolvedBinary) -> &'static str {
    if !nf.found || !recorder.found {
        return "MISSING";
    }
    if nf.discovery == recorder.discovery {
        return nf.discovery.as_str();
    }
    "MIXED"
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
    use crate::config::{NextFrameConfig, NextFrameMode};
    use crate::doctor::doctor;

    #[test]
    fn doctor_reports_structured_error_when_binaries_are_missing() -> Result<(), String> {
        let report = doctor(NextFrameConfig {
            nf_bin: Some("/definitely/not/nf".into()),
            recorder_bin: Some("/definitely/not/nf-recorder".into()),
            home: None,
            mode: Some(NextFrameMode::Binary),
        });

        assert!(!report.ok);
        assert_eq!(report.stage, "doctor");
        assert_eq!(report.config.discovery, "MISSING");
        let error = report
            .error
            .ok_or_else(|| "missing binaries should include error".to_string())?;
        assert_eq!(error.code, "NEXTFRAME_NOT_FOUND");
        Ok(())
    }
}
