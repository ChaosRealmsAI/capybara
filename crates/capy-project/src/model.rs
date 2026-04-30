use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PROJECT_SCHEMA_VERSION: &str = "capy.project.v1";
pub const ARTIFACT_REGISTRY_SCHEMA_VERSION: &str = "capy.artifacts.v1";
pub const DESIGN_LANGUAGE_SCHEMA_VERSION: &str = "capy.design-language.v1";
pub const CONTEXT_SCHEMA_VERSION: &str = "capy.context.v1";
pub const PATCH_SCHEMA_VERSION: &str = "capy.patch.v1";
pub const PATCH_RUN_SCHEMA_VERSION: &str = "capy.patch-run.v1";
pub const WORKBENCH_SCHEMA_VERSION: &str = "capy.project-workbench.v1";
pub const SURFACE_NODES_SCHEMA_VERSION: &str = "capy.surface-nodes.v1";
pub const GENERATE_RUN_SCHEMA_VERSION: &str = "capy.project-generate-run.v1";
pub const PROJECT_AI_PROMPT_SCHEMA_VERSION: &str = "capy.project-ai-prompt.v1";
pub const PROJECT_AI_RESPONSE_SCHEMA_VERSION: &str = "capy.project-ai-response.v1";
pub const DESIGN_LANGUAGE_VALIDATION_SCHEMA_VERSION: &str = "capy.design-language.validation.v1";
pub const DESIGN_LANGUAGE_INSPECTION_SCHEMA_VERSION: &str = "capy.design-language.inspection.v1";
pub const PROJECT_REVIEW_MODE: &str = "review";

fn default_design_language_id() -> String {
    "dlpkg_default".to_string()
}

fn default_design_language_name() -> String {
    "Project Design Language".to_string()
}

fn default_design_language_version() -> String {
    "0.1.0".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArtifactKind {
    Html,
    Css,
    Js,
    Markdown,
    Image,
    Audio,
    Video,
    PosterJson,
    PptJson,
    CompositionJson,
    Other,
}

impl ArtifactKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Html => "html",
            Self::Css => "css",
            Self::Js => "js",
            Self::Markdown => "markdown",
            Self::Image => "image",
            Self::Audio => "audio",
            Self::Video => "video",
            Self::PosterJson => "poster-json",
            Self::PptJson => "ppt-json",
            Self::CompositionJson => "composition-json",
            Self::Other => "other",
        }
    }
}

impl std::str::FromStr for ArtifactKind {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "html" => Ok(Self::Html),
            "css" => Ok(Self::Css),
            "js" | "javascript" => Ok(Self::Js),
            "markdown" | "md" => Ok(Self::Markdown),
            "image" => Ok(Self::Image),
            "audio" => Ok(Self::Audio),
            "video" => Ok(Self::Video),
            "poster-json" | "poster_json" => Ok(Self::PosterJson),
            "ppt-json" | "ppt_json" | "deck-json" | "deck_json" => Ok(Self::PptJson),
            "composition-json" | "composition_json" => Ok(Self::CompositionJson),
            "other" => Ok(Self::Other),
            other => Err(format!("invalid artifact kind: {other}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectManifestV1 {
    pub schema_version: String,
    pub id: String,
    pub name: String,
    pub root: String,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignLanguageManifestV1 {
    pub schema_version: String,
    #[serde(default = "default_design_language_id")]
    pub id: String,
    #[serde(default = "default_design_language_name")]
    pub name: String,
    #[serde(default = "default_design_language_version")]
    pub version: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub updated_at: u64,
    #[serde(default)]
    pub assets: Vec<DesignLanguageAssetV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignLanguageAssetV1 {
    pub id: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    pub path: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignLanguageSummaryV1 {
    pub name: String,
    pub version: String,
    pub summary: String,
    pub design_language_ref: String,
    pub asset_count: usize,
    pub token_count: usize,
    pub reference_image_count: usize,
    pub rule_count: usize,
    pub example_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignLanguageAssetStatusV1 {
    pub id: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    pub path: String,
    pub title: String,
    pub exists: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignLanguageValidationV1 {
    pub schema_version: String,
    pub ok: bool,
    pub project_id: String,
    pub project_name: String,
    pub design_language_ref: String,
    pub summary: DesignLanguageSummaryV1,
    #[serde(default)]
    pub assets: Vec<DesignLanguageAssetStatusV1>,
    #[serde(default)]
    pub errors: Vec<String>,
    pub generated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignLanguageInspectionV1 {
    pub schema_version: String,
    pub project_id: String,
    pub project_name: String,
    pub design_language_ref: String,
    pub summary: DesignLanguageSummaryV1,
    pub manifest: DesignLanguageManifestV1,
    #[serde(default)]
    pub assets: Vec<DesignLanguageAssetStatusV1>,
    pub generated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactRegistryV1 {
    pub schema_version: String,
    #[serde(default)]
    pub artifacts: Vec<ArtifactRefV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactRefV1 {
    pub id: String,
    pub kind: ArtifactKind,
    pub title: String,
    pub source_path: String,
    #[serde(default)]
    pub source_refs: Vec<String>,
    #[serde(default)]
    pub output_refs: Vec<String>,
    #[serde(default)]
    pub design_language_refs: Vec<String>,
    #[serde(default)]
    pub asset_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Value>,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInspectionV1 {
    pub manifest: ProjectManifestV1,
    pub design_language: DesignLanguageManifestV1,
    pub design_language_summary: DesignLanguageSummaryV1,
    pub artifacts: ArtifactRegistryV1,
}

#[derive(Debug, Clone)]
pub struct ContextBuildRequest {
    pub artifact_id: String,
    pub selector: Option<String>,
    pub canvas_node: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextPackageV1 {
    pub schema_version: String,
    pub context_id: String,
    pub project_id: String,
    pub artifact_id: String,
    pub artifact_kind: ArtifactKind,
    pub source_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canvas_node: Option<String>,
    pub artifact: ArtifactRefV1,
    pub design_language_ref: String,
    pub design_language_summary: DesignLanguageSummaryV1,
    #[serde(default)]
    pub design_language_refs: Vec<DesignLanguageAssetV1>,
    #[serde(default)]
    pub verification_requirements: Vec<String>,
    pub generated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchDocumentV1 {
    pub schema_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_context_ref: Option<String>,
    #[serde(default)]
    pub actor: Option<String>,
    pub operations: Vec<ReplaceExactTextOperationV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplaceExactTextOperationV1 {
    pub op: String,
    pub artifact_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    pub old_text: String,
    pub new_text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selector_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchRunV1 {
    pub schema_version: String,
    pub id: String,
    pub project_id: String,
    pub actor: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_context_ref: Option<String>,
    #[serde(default)]
    pub patch_refs: Vec<String>,
    #[serde(default)]
    pub changed_artifact_refs: Vec<String>,
    pub status: String,
    pub trace_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    pub dry_run: bool,
    pub generated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchApplyResultV1 {
    pub run: PatchRunV1,
    pub run_path: String,
    #[serde(default)]
    pub changed_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkbenchPreviewV1 {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectWorkbenchCardV1 {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    #[serde(default)]
    pub source_refs: Vec<String>,
    #[serde(default)]
    pub output_refs: Vec<String>,
    #[serde(default)]
    pub design_language_refs: Vec<String>,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    pub preview: WorkbenchPreviewV1,
    #[serde(default)]
    pub next_actions: Vec<String>,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectWorkbenchV1 {
    pub schema_version: String,
    pub project_id: String,
    pub project_name: String,
    pub design_language_summary: DesignLanguageSummaryV1,
    #[serde(default)]
    pub cards: Vec<ProjectWorkbenchCardV1>,
    pub generated_at: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SurfaceGeometryV1 {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSurfaceNodeV1 {
    pub id: String,
    pub surface: String,
    pub artifact_id: String,
    pub geometry: SurfaceGeometryV1,
    pub status: String,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSurfaceNodesV1 {
    pub schema_version: String,
    pub project_id: String,
    #[serde(default)]
    pub nodes: Vec<ProjectSurfaceNodeV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectGenerateRequestV1 {
    pub artifact_id: String,
    pub provider: String,
    pub prompt: String,
    pub dry_run: bool,
    #[serde(default)]
    pub review: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectGenerateRunV1 {
    pub schema_version: String,
    pub id: String,
    pub project_id: String,
    pub artifact_id: String,
    pub provider: String,
    pub prompt: String,
    pub status: String,
    pub trace_id: String,
    pub dry_run: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design_language_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design_language_summary: Option<DesignLanguageSummaryV1>,
    #[serde(default)]
    pub command_preview: Vec<String>,
    #[serde(default)]
    pub changed_artifact_refs: Vec<String>,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<ProjectRunReviewV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub generated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectGenerateResultV1 {
    pub run: ProjectGenerateRunV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact: Option<ArtifactRefV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRunReviewV1 {
    pub mode: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_run_id: Option<String>,
    pub base_hash: String,
    pub proposed_hash: String,
    pub diff_summary: ProjectDiffSummaryV1,
    #[serde(default)]
    pub decisions: Vec<ProjectRunDecisionV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDiffSummaryV1 {
    pub artifact_id: String,
    pub source_path: String,
    pub old_hash: String,
    pub new_hash: String,
    pub old_bytes: usize,
    pub new_bytes: usize,
    pub removed_lines: usize,
    pub added_lines: usize,
    pub changed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old_preview: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_preview: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRunDecisionV1 {
    pub actor: String,
    pub decision: String,
    pub status: String,
    #[serde(default)]
    pub artifact_refs: Vec<String>,
    pub decided_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRunDecisionResultV1 {
    pub run: ProjectGenerateRunV1,
    pub run_path: String,
    #[serde(default)]
    pub changed_files: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectAiPromptV1 {
    pub schema_version: String,
    pub context_id: String,
    pub project_id: String,
    pub artifact_id: String,
    pub source_path: String,
    pub provider: String,
    pub design_language_ref: String,
    pub design_language_summary: DesignLanguageSummaryV1,
    pub prompt: String,
    pub output_schema: Value,
    pub generated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectAiResponseV1 {
    pub schema_version: String,
    pub summary_zh: String,
    #[serde(default)]
    pub artifacts: Vec<ProjectAiArtifactV1>,
    #[serde(default)]
    pub verify_notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectAiArtifactV1 {
    pub artifact_id: String,
    pub source_path: String,
    pub new_source: String,
}
