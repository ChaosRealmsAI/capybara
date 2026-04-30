use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PROJECT_SCHEMA_VERSION: &str = "capy.project.v1";
pub const ARTIFACT_REGISTRY_SCHEMA_VERSION: &str = "capy.artifacts.v1";
pub const DESIGN_LANGUAGE_SCHEMA_VERSION: &str = "capy.design-language.v1";
pub const CONTEXT_SCHEMA_VERSION: &str = "capy.context.v1";
pub const PATCH_SCHEMA_VERSION: &str = "capy.patch.v1";
pub const PATCH_RUN_SCHEMA_VERSION: &str = "capy.patch-run.v1";

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
    #[serde(default)]
    pub assets: Vec<DesignLanguageAssetV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignLanguageAssetV1 {
    pub id: String,
    pub kind: String,
    pub path: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
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
