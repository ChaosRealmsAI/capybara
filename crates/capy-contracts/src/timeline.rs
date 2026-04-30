use serde::{Deserialize, Serialize};

pub const OP_TIMELINE_ATTACH: &str = "timeline-attach";
pub const OP_TIMELINE_STATE: &str = "timeline-state";
pub const OP_TIMELINE_STATE_DETAIL: &str = "timeline-state-detail";
pub const OP_TIMELINE_OPEN: &str = "timeline-open";
pub const OP_TIMELINE_COMPOSITION_OPEN: &str = "timeline-composition-open";
pub const OP_TIMELINE_COMPOSITION_STATE: &str = "timeline-composition-state";
pub const OP_TIMELINE_COMPOSITION_PATCH: &str = "timeline-composition-patch";
pub const OP_TIMELINE_EXPORT_START: &str = "timeline-export-start";
pub const OP_TIMELINE_EXPORT_STATUS: &str = "timeline-export-status";
pub const OP_TIMELINE_EXPORT_CANCEL: &str = "timeline-export-cancel";

pub const KIND_TIMELINE_COMPOSITION: &str = "timeline-composition";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TimelineNodeState {
    Draft,
    Valid,
    Compiled,
    PreviewReady,
    Exported,
    Error,
}

impl TimelineNodeState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Valid => "valid",
            Self::Compiled => "compiled",
            Self::PreviewReady => "preview-ready",
            Self::Exported => "exported",
            Self::Error => "error",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        KIND_TIMELINE_COMPOSITION, OP_TIMELINE_ATTACH, OP_TIMELINE_COMPOSITION_OPEN,
        OP_TIMELINE_EXPORT_START, TimelineNodeState,
    };

    #[test]
    fn keeps_timeline_names_explicit() {
        assert_eq!(OP_TIMELINE_ATTACH, "timeline-attach");
        assert_eq!(OP_TIMELINE_COMPOSITION_OPEN, "timeline-composition-open");
        assert_eq!(OP_TIMELINE_EXPORT_START, "timeline-export-start");
        assert_eq!(KIND_TIMELINE_COMPOSITION, "timeline-composition");
        assert_eq!(TimelineNodeState::PreviewReady.as_str(), "preview-ready");
    }
}
