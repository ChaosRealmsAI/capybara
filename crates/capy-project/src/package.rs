use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use thiserror::Error;
use uuid::Uuid;

use crate::design_language::selected_design_assets;
use crate::model::{
    ARTIFACT_REGISTRY_SCHEMA_VERSION, ArtifactKind, ArtifactRefV1, ArtifactRegistryV1,
    CONTEXT_SCHEMA_VERSION, ContextBuildRequest, ContextPackageV1, DESIGN_LANGUAGE_SCHEMA_VERSION,
    DesignLanguageAssetV1, DesignLanguageManifestV1, PROJECT_SCHEMA_VERSION, PatchApplyResultV1,
    PatchDocumentV1, PatchRunV1, ProjectInspectionV1, ProjectManifestV1,
};

pub const CAPY_DIR: &str = ".capy";

pub type ProjectPackageResult<T> = Result<T, ProjectPackageError>;

#[derive(Debug, Error)]
pub enum ProjectPackageError {
    #[error("{0}")]
    Invalid(String),
    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{context}: {source}")]
    Json {
        context: String,
        #[source]
        source: serde_json::Error,
    },
}

pub struct ProjectPackage {
    root: PathBuf,
}

impl ProjectPackage {
    pub fn init(root: impl AsRef<Path>, name: Option<String>) -> ProjectPackageResult<Self> {
        let root = root.as_ref();
        fs::create_dir_all(root).map_err(|source| ProjectPackageError::Io {
            context: format!("create project root {}", root.display()),
            source,
        })?;
        let package = Self {
            root: canonicalize_existing(root)?,
        };
        fs::create_dir_all(package.capy_dir()).map_err(|source| ProjectPackageError::Io {
            context: format!("create {}", package.capy_dir().display()),
            source,
        })?;
        fs::create_dir_all(package.runs_dir()).map_err(|source| ProjectPackageError::Io {
            context: format!("create {}", package.runs_dir().display()),
            source,
        })?;
        fs::create_dir_all(package.evidence_dir()).map_err(|source| ProjectPackageError::Io {
            context: format!("create {}", package.evidence_dir().display()),
            source,
        })?;

        if !package.project_manifest_path().exists() {
            let project_name = name.unwrap_or_else(|| {
                package
                    .root
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("Capybara Project")
                    .to_string()
            });
            let now = now_ms();
            package.write_json(
                &package.project_manifest_path(),
                &ProjectManifestV1 {
                    schema_version: PROJECT_SCHEMA_VERSION.to_string(),
                    id: new_id("proj"),
                    name: project_name,
                    root: package.root.display().to_string(),
                    created_at: now,
                    updated_at: now,
                },
            )?;
        }
        if !package.artifacts_path().exists() {
            package.write_json(
                &package.artifacts_path(),
                &ArtifactRegistryV1 {
                    schema_version: ARTIFACT_REGISTRY_SCHEMA_VERSION.to_string(),
                    artifacts: Vec::new(),
                },
            )?;
        }
        if !package.design_language_path().exists() {
            let now = now_ms();
            package.write_json(
                &package.design_language_path(),
                &DesignLanguageManifestV1 {
                    schema_version: DESIGN_LANGUAGE_SCHEMA_VERSION.to_string(),
                    id: "dlpkg_default".to_string(),
                    name: "Project Design Language".to_string(),
                    version: "0.1.0".to_string(),
                    summary:
                        "Project-level tokens, rules, references, and examples for AI generation."
                            .to_string(),
                    updated_at: now,
                    assets: Vec::new(),
                },
            )?;
        }
        let evidence_manifest = package.evidence_dir().join("manifest.json");
        if !evidence_manifest.exists() {
            package.write_json(
                &evidence_manifest,
                &serde_json::json!({
                    "schema_version": "capy.evidence.v1",
                    "records": []
                }),
            )?;
        }
        Ok(package)
    }

    pub fn open(root: impl AsRef<Path>) -> ProjectPackageResult<Self> {
        let root = canonicalize_existing(root.as_ref())?;
        let package = Self { root };
        if !package.project_manifest_path().exists() {
            return Err(ProjectPackageError::Invalid(format!(
                "project package missing {}; run `capy project init --project {}`",
                package.project_manifest_path().display(),
                package.root.display()
            )));
        }
        Ok(package)
    }

    pub fn inspect(&self) -> ProjectPackageResult<ProjectInspectionV1> {
        let design_language = self.design_language()?;
        let design_language_summary = self.design_language_summary_for(&design_language);
        Ok(ProjectInspectionV1 {
            manifest: self.project_manifest()?,
            design_language,
            design_language_summary,
            artifacts: self.artifacts()?,
        })
    }

    pub fn add_design_asset(
        &self,
        kind: String,
        role: Option<String>,
        path: impl AsRef<Path>,
        title: String,
        description: Option<String>,
    ) -> ProjectPackageResult<DesignLanguageAssetV1> {
        let mut manifest = self.design_language()?;
        let asset = DesignLanguageAssetV1 {
            id: new_id("dl"),
            kind,
            role,
            path: self.relative_existing_path(path.as_ref())?,
            title,
            description,
        };
        manifest.assets.push(asset.clone());
        self.write_json(&self.design_language_path(), &manifest)?;
        self.touch_project_manifest()?;
        Ok(asset)
    }

    pub fn add_artifact(
        &self,
        kind: ArtifactKind,
        source_path: impl AsRef<Path>,
        title: String,
        design_language_refs: Vec<String>,
    ) -> ProjectPackageResult<ArtifactRefV1> {
        let mut registry = self.artifacts()?;
        validate_design_refs(&self.design_language()?, &design_language_refs)?;
        let artifact = ArtifactRefV1 {
            id: new_id("art"),
            kind,
            title,
            source_path: self.relative_existing_path(source_path.as_ref())?,
            source_refs: Vec::new(),
            output_refs: Vec::new(),
            design_language_refs,
            asset_refs: Vec::new(),
            provenance: None,
            evidence_refs: Vec::new(),
            updated_at: now_ms(),
        };
        registry.artifacts.push(artifact.clone());
        self.write_json(&self.artifacts_path(), &registry)?;
        self.touch_project_manifest()?;
        Ok(artifact)
    }

    pub fn build_context(
        &self,
        request: ContextBuildRequest,
    ) -> ProjectPackageResult<ContextPackageV1> {
        let manifest = self.project_manifest()?;
        let design_language = self.design_language()?;
        let registry = self.artifacts()?;
        let artifact = registry
            .artifacts
            .iter()
            .find(|item| item.id == request.artifact_id)
            .cloned()
            .ok_or_else(|| {
                ProjectPackageError::Invalid(format!(
                    "unknown artifact id: {}",
                    request.artifact_id
                ))
            })?;
        let design_language_summary = self.design_language_summary_for(&design_language);
        let design_refs = selected_design_assets(&design_language, &artifact.design_language_refs);
        let selection_context = self.build_selection_context(&artifact, &request)?;
        Ok(ContextPackageV1 {
            schema_version: CONTEXT_SCHEMA_VERSION.to_string(),
            context_id: new_id("ctx"),
            project_id: manifest.id,
            artifact_id: artifact.id.clone(),
            artifact_kind: artifact.kind.clone(),
            source_path: artifact.source_path.clone(),
            selector: request.selector.clone(),
            canvas_node: request.canvas_node.clone(),
            selection_context,
            artifact,
            design_language_ref: design_language_summary.design_language_ref.clone(),
            design_language_summary,
            design_language_refs: design_refs,
            verification_requirements: vec![
                "Run a visible preview check after patching.".to_string(),
                "Save screenshot/state evidence for PM review.".to_string(),
            ],
            generated_at: now_ms(),
        })
    }

    pub fn read_artifact_source(&self, artifact_id: &str) -> ProjectPackageResult<String> {
        let artifact = self.artifact(artifact_id)?;
        let path = self.root.join(&artifact.source_path);
        read_to_string(&path, "read artifact source")
    }

    pub fn apply_patch(
        &self,
        patch: PatchDocumentV1,
        patch_ref: Option<String>,
        dry_run: bool,
    ) -> ProjectPackageResult<PatchApplyResultV1> {
        crate::patch::apply_patch(self, patch, patch_ref, dry_run)
    }

    pub(crate) fn artifact(&self, id: &str) -> ProjectPackageResult<ArtifactRefV1> {
        self.artifacts()?
            .artifacts
            .into_iter()
            .find(|artifact| artifact.id == id)
            .ok_or_else(|| ProjectPackageError::Invalid(format!("unknown artifact id: {id}")))
    }

    pub(crate) fn source_path_for(
        &self,
        artifact: &ArtifactRefV1,
        requested: Option<&str>,
    ) -> ProjectPackageResult<PathBuf> {
        if let Some(requested) = requested {
            let normalized = self.relative_existing_path(Path::new(requested))?;
            if normalized != artifact.source_path {
                return Err(ProjectPackageError::Invalid(format!(
                    "patch source_path {} does not match artifact {} source_path {}",
                    normalized, artifact.id, artifact.source_path
                )));
            }
        }
        Ok(self.root.join(&artifact.source_path))
    }

    pub(crate) fn write_run(&self, run: &PatchRunV1) -> ProjectPackageResult<String> {
        fs::create_dir_all(self.runs_dir()).map_err(|source| ProjectPackageError::Io {
            context: format!("create {}", self.runs_dir().display()),
            source,
        })?;
        let relative = format!("{CAPY_DIR}/runs/{}.json", run.id);
        self.write_json(&self.root.join(&relative), run)?;
        Ok(relative)
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn project_manifest(&self) -> ProjectPackageResult<ProjectManifestV1> {
        read_json(&self.project_manifest_path(), "read project manifest")
    }

    pub(crate) fn design_language(&self) -> ProjectPackageResult<DesignLanguageManifestV1> {
        read_json(
            &self.design_language_path(),
            "read design language manifest",
        )
    }

    pub(crate) fn artifacts(&self) -> ProjectPackageResult<ArtifactRegistryV1> {
        read_json(&self.artifacts_path(), "read artifact registry")
    }

    pub(crate) fn write_artifacts(
        &self,
        registry: &ArtifactRegistryV1,
    ) -> ProjectPackageResult<()> {
        self.write_json(&self.artifacts_path(), registry)
    }

    pub(crate) fn touch_project_manifest(&self) -> ProjectPackageResult<()> {
        let mut manifest = self.project_manifest()?;
        manifest.updated_at = now_ms();
        self.write_json(&self.project_manifest_path(), &manifest)
    }

    pub(crate) fn write_json<T: Serialize>(
        &self,
        path: &Path,
        value: &T,
    ) -> ProjectPackageResult<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| ProjectPackageError::Io {
                context: format!("create {}", parent.display()),
                source,
            })?;
        }
        let payload =
            serde_json::to_string_pretty(value).map_err(|source| ProjectPackageError::Json {
                context: format!("serialize {}", path.display()),
                source,
            })?;
        fs::write(path, format!("{payload}\n")).map_err(|source| ProjectPackageError::Io {
            context: format!("write {}", path.display()),
            source,
        })
    }

    fn relative_existing_path(&self, path: &Path) -> ProjectPackageResult<String> {
        let candidate = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root.join(path)
        };
        let resolved = canonicalize_existing(&candidate)?;
        if !resolved.starts_with(&self.root) {
            return Err(ProjectPackageError::Invalid(format!(
                "path must live inside project root: {}",
                resolved.display()
            )));
        }
        resolved
            .strip_prefix(&self.root)
            .map_err(|err| {
                ProjectPackageError::Invalid(format!(
                    "path {} is not project-relative: {err}",
                    resolved.display()
                ))
            })
            .map(|path| path.display().to_string())
    }

    fn capy_dir(&self) -> PathBuf {
        self.root.join(CAPY_DIR)
    }

    fn project_manifest_path(&self) -> PathBuf {
        self.capy_dir().join("project.json")
    }

    pub(crate) fn artifacts_path(&self) -> PathBuf {
        self.capy_dir().join("artifacts.json")
    }

    fn design_language_path(&self) -> PathBuf {
        self.capy_dir().join("design-language.json")
    }

    fn runs_dir(&self) -> PathBuf {
        self.capy_dir().join("runs")
    }

    fn evidence_dir(&self) -> PathBuf {
        self.capy_dir().join("evidence")
    }
}

pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

pub(crate) fn new_id(prefix: &str) -> String {
    format!("{}_{}", prefix, Uuid::new_v4().simple())
}

pub(crate) fn read_to_string(path: &Path, context: &str) -> ProjectPackageResult<String> {
    fs::read_to_string(path).map_err(|source| ProjectPackageError::Io {
        context: format!("{context} {}", path.display()),
        source,
    })
}

pub(crate) fn write_string(path: &Path, contents: &str) -> ProjectPackageResult<()> {
    fs::write(path, contents).map_err(|source| ProjectPackageError::Io {
        context: format!("write {}", path.display()),
        source,
    })
}

fn read_json<T: for<'de> serde::Deserialize<'de>>(
    path: &Path,
    context: &str,
) -> ProjectPackageResult<T> {
    let raw = read_to_string(path, context)?;
    serde_json::from_str(&raw).map_err(|source| ProjectPackageError::Json {
        context: format!("parse {}", path.display()),
        source,
    })
}

fn canonicalize_existing(path: &Path) -> ProjectPackageResult<PathBuf> {
    path.canonicalize()
        .map_err(|source| ProjectPackageError::Io {
            context: format!("resolve {}", path.display()),
            source,
        })
}

fn validate_design_refs(
    manifest: &DesignLanguageManifestV1,
    refs: &[String],
) -> ProjectPackageResult<()> {
    let known: BTreeSet<&str> = manifest
        .assets
        .iter()
        .map(|asset| asset.id.as_str())
        .collect();
    let unknown: Vec<&str> = refs
        .iter()
        .map(String::as_str)
        .filter(|id| !known.contains(id))
        .collect();
    if unknown.is_empty() {
        Ok(())
    } else {
        Err(ProjectPackageError::Invalid(format!(
            "unknown design language refs: {}",
            unknown.join(", ")
        )))
    }
}

pub(crate) fn count_matches(contents: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    contents.match_indices(needle).count()
}

pub(crate) fn dedupe_sorted(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(crate) fn changed_file_map(paths: Vec<(PathBuf, String)>) -> BTreeMap<PathBuf, String> {
    paths.into_iter().collect()
}
