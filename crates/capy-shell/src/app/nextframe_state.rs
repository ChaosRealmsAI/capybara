use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NextFrameNodeState {
    Draft,
    Valid,
    Compiled,
    PreviewReady,
    Exported,
    Error {
        code: String,
        message: String,
        hint: Option<String>,
    },
}

impl NextFrameNodeState {
    pub fn transition(
        &self,
        action: NextFrameNodeAction,
    ) -> Result<NextFrameNodeState, IllegalTransitionError> {
        let to = match (self, action) {
            (Self::Draft, NextFrameNodeAction::ValidateOk) => Self::Valid,
            (Self::Valid, NextFrameNodeAction::CompileOk) => Self::Compiled,
            (Self::Compiled, NextFrameNodeAction::PreviewReady) => Self::PreviewReady,
            (Self::Compiled, NextFrameNodeAction::ExportOk) => Self::Exported,
            (Self::PreviewReady, NextFrameNodeAction::ExportOk) => Self::Exported,
            (
                _,
                NextFrameNodeAction::Error {
                    code,
                    message,
                    hint,
                },
            ) => Self::Error {
                code,
                message,
                hint,
            },
            (from, action) => {
                return Err(IllegalTransitionError {
                    from: from.label().to_string(),
                    action: action.label().to_string(),
                });
            }
        };
        Ok(to)
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Valid => "valid",
            Self::Compiled => "compiled",
            Self::PreviewReady => "preview-ready",
            Self::Exported => "exported",
            Self::Error { .. } => "error",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NextFrameNodeAction {
    ValidateOk,
    CompileOk,
    PreviewReady,
    ExportOk,
    Error {
        code: String,
        message: String,
        hint: Option<String>,
    },
}

impl NextFrameNodeAction {
    fn label(&self) -> &'static str {
        match self {
            Self::ValidateOk => "validate-ok",
            Self::CompileOk => "compile-ok",
            Self::PreviewReady => "preview-ready",
            Self::ExportOk => "export-ok",
            Self::Error { .. } => "error",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IllegalTransitionError {
    pub from: String,
    pub action: String,
}

impl std::fmt::Display for IllegalTransitionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "illegal NextFrame transition from {} via {}",
            self.from, self.action
        )
    }
}

impl std::error::Error for IllegalTransitionError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NextFrameTransition {
    pub from: NextFrameNodeState,
    pub to: NextFrameNodeState,
    pub at: String,
    pub reason: String,
}

pub fn iso_now() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    iso_from_unix(seconds)
}

pub fn iso_from_unix(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let seconds_of_day = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn civil_from_days(days_since_unix_epoch: i64) -> (i64, u32, u32) {
    let z = days_since_unix_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if m <= 2 { 1 } else { 0 };
    (year, m as u32, d as u32)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportJob {
    pub job_id: String,
    pub status: ExportJobStatus,
    pub progress: u8,
    pub output_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byte_size: Option<u64>,
    pub started_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportJobStatus {
    Queued,
    Running,
    Done,
    Failed,
    Cancelled,
}

#[cfg(test)]
mod tests {
    use super::{NextFrameNodeAction, NextFrameNodeState, iso_from_unix};

    #[test]
    fn transition_happy_path_reaches_preview_ready() -> Result<(), Box<dyn std::error::Error>> {
        let state = NextFrameNodeState::Draft;
        let state = state.transition(NextFrameNodeAction::ValidateOk)?;
        let state = state.transition(NextFrameNodeAction::CompileOk)?;
        let state = state.transition(NextFrameNodeAction::PreviewReady)?;

        assert_eq!(state, NextFrameNodeState::PreviewReady);
        Ok(())
    }

    #[test]
    fn transition_allows_export_from_compiled_or_preview_ready()
    -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            NextFrameNodeState::Compiled.transition(NextFrameNodeAction::ExportOk)?,
            NextFrameNodeState::Exported
        );
        assert_eq!(
            NextFrameNodeState::PreviewReady.transition(NextFrameNodeAction::ExportOk)?,
            NextFrameNodeState::Exported
        );
        Ok(())
    }

    #[test]
    fn transition_rejects_compile_before_validate() -> Result<(), Box<dyn std::error::Error>> {
        let error = NextFrameNodeState::Draft
            .transition(NextFrameNodeAction::CompileOk)
            .err()
            .ok_or("draft cannot compile directly")?;

        assert_eq!(error.from, "draft");
        assert_eq!(error.action, "compile-ok");
        Ok(())
    }

    #[test]
    fn transition_allows_error_from_any_state() -> Result<(), Box<dyn std::error::Error>> {
        let state = NextFrameNodeState::Compiled.transition(NextFrameNodeAction::Error {
            code: "COMPILE_FAILED".to_string(),
            message: "compile failed".to_string(),
            hint: Some("next step · rerun capy nextframe compile".to_string()),
        })?;

        assert_eq!(state.label(), "error");
        Ok(())
    }

    #[test]
    fn iso_from_unix_formats_utc_timestamp() {
        assert_eq!(iso_from_unix(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_from_unix(1_775_000_000), "2026-03-31T23:33:20Z");
    }
}
