use serde::{Deserialize, Serialize};

use crate::video_clip_queue::{ProjectVideoClipQueueItemV1, ProjectVideoClipQueueManifestV1};

pub const VIDEO_CLIP_PROPOSAL_SCHEMA_VERSION: &str = "capy.project-video-clip-proposal.v1";
pub const VIDEO_CLIP_PROPOSAL_DECISION_SCHEMA_VERSION: &str =
    "capy.project-video-clip-proposal-decision-result.v1";
pub const VIDEO_CLIP_PROPOSAL_HISTORY_SCHEMA_VERSION: &str =
    "capy.project-video-clip-proposal-history.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectVideoClipProposalV1 {
    pub schema_version: String,
    pub project_id: String,
    pub project_name: String,
    pub proposal_id: String,
    #[serde(default)]
    pub revision: u64,
    pub source_suggestion_id: String,
    pub planner: String,
    pub status: String,
    pub generated_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decided_at: Option<u64>,
    #[serde(default)]
    pub base_queue_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_queue_hash: Option<String>,
    pub rationale: String,
    pub safety_note: String,
    #[serde(default)]
    pub before_queue: Vec<ProjectVideoClipQueueItemV1>,
    #[serde(default)]
    pub after_queue: Vec<ProjectVideoClipQueueItemV1>,
    #[serde(default)]
    pub changes: Vec<ProjectVideoClipProposalChangeV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision: Option<ProjectVideoClipProposalDecisionV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conflict: Option<ProjectVideoClipProposalConflictV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectVideoClipProposalChangeV1 {
    pub id: String,
    pub action: String,
    pub action_label_zh: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before_sequence: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_sequence: Option<u64>,
    pub queue_item_id: String,
    pub clip_key: String,
    pub scene: String,
    pub reason_summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feedback_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feedback_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feedback_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_reason: Option<String>,
    pub applicable: bool,
    pub apply_status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before_item: Option<ProjectVideoClipQueueItemV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_item: Option<ProjectVideoClipQueueItemV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectVideoClipProposalDecisionV1 {
    pub decision: String,
    pub reason: String,
    pub decided_at: u64,
    pub queue_updated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectVideoClipProposalConflictV1 {
    pub conflict_type: String,
    pub message_zh: String,
    pub base_queue_hash: String,
    pub current_queue_hash: String,
    pub detected_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectVideoClipProposalDecisionResultV1 {
    pub schema_version: String,
    pub proposal: ProjectVideoClipProposalV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queue_manifest: Option<ProjectVideoClipQueueManifestV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectVideoClipProposalHistoryV1 {
    pub schema_version: String,
    pub project_id: String,
    pub project_name: String,
    pub updated_at: u64,
    #[serde(default)]
    pub entries: Vec<ProjectVideoClipProposalHistoryEntryV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectVideoClipProposalHistoryEntryV1 {
    pub proposal_id: String,
    #[serde(default)]
    pub revision: u64,
    pub source_suggestion_id: String,
    pub planner: String,
    pub status: String,
    pub generated_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decided_at: Option<u64>,
    #[serde(default)]
    pub base_queue_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_queue_hash: Option<String>,
    pub rationale: String,
    pub safety_note: String,
    #[serde(default)]
    pub before_queue_count: usize,
    #[serde(default)]
    pub after_queue_count: usize,
    #[serde(default)]
    pub changes: Vec<ProjectVideoClipProposalChangeV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision: Option<ProjectVideoClipProposalDecisionV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conflict: Option<ProjectVideoClipProposalConflictV1>,
}
