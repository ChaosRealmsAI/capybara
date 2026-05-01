//! Project package core for Capybara.
//!
//! This crate owns the file-backed `.capy/` project truth used by CLI, shell,
//! frontend surfaces, and future AI context builders.

mod ai;
mod campaign;
mod design_language;
mod generate;
mod model;
mod package;
mod patch;
mod review;
mod selection_context;
mod surface_nodes;
mod video_clip_feedback;
mod video_clip_proposal;
mod video_clip_proposal_history;
#[cfg(test)]
mod video_clip_proposal_tests;
mod video_clip_proposal_types;
mod video_clip_queue;
mod video_clip_semantics;
mod video_clip_suggestion;
#[cfg(test)]
mod video_clip_suggestion_tests;
mod video_import;
mod workbench;

pub use ai::{parse_project_ai_response, project_ai_output_schema};
pub use campaign::{
    ProjectCampaignGenerateResultV1, ProjectCampaignPlanV1, ProjectCampaignRequestV1,
    ProjectCampaignRunV1,
};
pub use model::{
    ArtifactKind, ArtifactRefV1, ArtifactRegistryV1, ContextBuildRequest, ContextPackageV1,
    DesignLanguageAssetStatusV1, DesignLanguageAssetV1, DesignLanguageInspectionV1,
    DesignLanguageManifestV1, DesignLanguageSummaryV1, DesignLanguageValidationV1,
    GENERATE_RUN_SCHEMA_VERSION, PatchApplyResultV1, PatchDocumentV1, PatchRunV1,
    ProjectAiArtifactV1, ProjectAiPromptV1, ProjectAiResponseV1, ProjectDiffSummaryV1,
    ProjectGenerateRequestV1, ProjectGenerateResultV1, ProjectGenerateRunV1, ProjectInspectionV1,
    ProjectManifestV1, ProjectRunDecisionResultV1, ProjectRunDecisionV1, ProjectRunReviewV1,
    ProjectSurfaceNodeV1, ProjectSurfaceNodesV1, ProjectWorkbenchCardV1, ProjectWorkbenchV1,
    ReplaceExactTextOperationV1, SURFACE_NODES_SCHEMA_VERSION, SurfaceGeometryV1,
    WorkbenchPreviewV1,
};
pub use package::{CAPY_DIR, ProjectPackage, ProjectPackageError, ProjectPackageResult};
pub use selection_context::{SelectionBoundsV1, SelectionContextV1};
pub use video_clip_feedback::{
    ProjectVideoClipFeedbackItemV1, ProjectVideoClipFeedbackManifestV1,
    VIDEO_CLIP_FEEDBACK_SCHEMA_VERSION,
};
pub use video_clip_proposal_types::{
    ProjectVideoClipProposalChangeV1, ProjectVideoClipProposalConflictV1,
    ProjectVideoClipProposalDecisionResultV1, ProjectVideoClipProposalDecisionV1,
    ProjectVideoClipProposalHistoryEntryV1, ProjectVideoClipProposalHistoryV1,
    ProjectVideoClipProposalV1, VIDEO_CLIP_PROPOSAL_DECISION_SCHEMA_VERSION,
    VIDEO_CLIP_PROPOSAL_HISTORY_SCHEMA_VERSION, VIDEO_CLIP_PROPOSAL_SCHEMA_VERSION,
};
pub use video_clip_queue::{
    ProjectVideoClipQueueItemV1, ProjectVideoClipQueueManifestV1, VIDEO_CLIP_QUEUE_SCHEMA_VERSION,
};
pub use video_clip_semantics::{
    ProjectVideoClipSemanticItemV1, ProjectVideoClipSemanticsManifestV1,
    VIDEO_CLIP_SEMANTICS_SCHEMA_VERSION,
};
pub use video_clip_suggestion::{
    ProjectVideoClipSuggestionItemV1, ProjectVideoClipSuggestionV1,
    VIDEO_CLIP_SUGGESTION_SCHEMA_VERSION,
};
pub use video_import::{VIDEO_IMPORT_SCHEMA_VERSION, VideoImportMetadataV1, VideoImportResultV1};
