//! Project package core for Capybara.
//!
//! This crate owns the file-backed `.capy/` project truth used by CLI, shell,
//! frontend surfaces, and future AI context builders.

mod generate;
mod model;
mod package;
mod patch;
mod workbench;

pub use model::{
    ArtifactKind, ArtifactRefV1, ArtifactRegistryV1, ContextBuildRequest, ContextPackageV1,
    DesignLanguageAssetV1, DesignLanguageManifestV1, PatchApplyResultV1, PatchDocumentV1,
    PatchRunV1, ProjectGenerateRequestV1, ProjectGenerateResultV1, ProjectGenerateRunV1,
    ProjectInspectionV1, ProjectManifestV1, ProjectWorkbenchCardV1, ProjectWorkbenchV1,
    ReplaceExactTextOperationV1, WorkbenchPreviewV1,
};
pub use package::{CAPY_DIR, ProjectPackage, ProjectPackageError, ProjectPackageResult};
