use std::fs;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::generate::fixture_source;
use crate::model::{
    ArtifactKind, ArtifactRefV1, PATCH_SCHEMA_VERSION, PatchDocumentV1, ProjectGenerateRequestV1,
    ProjectGenerateResultV1, ReplaceExactTextOperationV1,
};
use crate::package::{
    CAPY_DIR, ProjectPackage, ProjectPackageError, ProjectPackageResult, new_id, now_ms,
};

pub const CAMPAIGN_PLAN_SCHEMA_VERSION: &str = "capy.project-campaign-plan.v1";
pub const CAMPAIGN_RUN_SCHEMA_VERSION: &str = "capy.project-campaign-run.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectCampaignRequestV1 {
    pub brief: String,
    #[serde(default)]
    pub artifact_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectCampaignTargetV1 {
    pub artifact_id: String,
    pub kind: String,
    pub title: String,
    pub source_path: String,
    #[serde(default)]
    pub dependencies: Vec<String>,
    pub review_mode: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectCampaignPlanV1 {
    pub schema_version: String,
    pub id: String,
    pub project_id: String,
    pub brief: String,
    pub design_language_ref: String,
    #[serde(default)]
    pub targets: Vec<ProjectCampaignTargetV1>,
    pub generated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectCampaignArtifactRunV1 {
    pub artifact_id: String,
    pub source_path: String,
    pub review_run_id: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectCampaignRunV1 {
    pub schema_version: String,
    pub id: String,
    pub project_id: String,
    pub plan_id: String,
    pub brief: String,
    pub design_language_ref: String,
    pub status: String,
    #[serde(default)]
    pub artifact_runs: Vec<ProjectCampaignArtifactRunV1>,
    pub generated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectCampaignGenerateResultV1 {
    pub run: ProjectCampaignRunV1,
    pub run_path: String,
    pub plan: ProjectCampaignPlanV1,
    #[serde(default)]
    pub proposals: Vec<ProjectGenerateResultV1>,
}

impl ProjectPackage {
    pub fn campaign_plan(
        &self,
        request: ProjectCampaignRequestV1,
    ) -> ProjectPackageResult<ProjectCampaignPlanV1> {
        let inspection = self.inspect()?;
        let targets = campaign_targets(&inspection.artifacts.artifacts, &request.artifact_ids)?;
        if targets.is_empty() {
            return Err(ProjectPackageError::Invalid(
                "campaign plan needs at least one editable artifact".to_string(),
            ));
        }
        Ok(ProjectCampaignPlanV1 {
            schema_version: CAMPAIGN_PLAN_SCHEMA_VERSION.to_string(),
            id: new_id("campplan"),
            project_id: inspection.manifest.id,
            brief: request.brief,
            design_language_ref: inspection.design_language_summary.design_language_ref,
            targets,
            generated_at: now_ms(),
        })
    }

    pub fn campaign_generate(
        &self,
        request: ProjectCampaignRequestV1,
    ) -> ProjectPackageResult<ProjectCampaignGenerateResultV1> {
        let plan = self.campaign_plan(request)?;
        let mut proposals = Vec::new();
        let mut artifact_runs = Vec::new();
        for target in &plan.targets {
            let artifact = self.artifact(&target.artifact_id)?;
            let current_source = self.read_artifact_source(&artifact.id)?;
            let new_source = campaign_fixture_source(&artifact, &current_source, &plan.brief)?;
            let patch = PatchDocumentV1 {
                schema_version: PATCH_SCHEMA_VERSION.to_string(),
                project_id: Some(plan.project_id.clone()),
                input_context_ref: Some(plan.id.clone()),
                actor: Some("project-campaign:fixture".to_string()),
                operations: vec![ReplaceExactTextOperationV1 {
                    op: "replace_exact_text".to_string(),
                    artifact_id: artifact.id.clone(),
                    source_path: Some(artifact.source_path.clone()),
                    old_text: current_source,
                    new_text: new_source.clone(),
                    selector_hint: None,
                }],
            };
            let generate_request = ProjectGenerateRequestV1 {
                artifact_id: artifact.id.clone(),
                provider: "fixture".to_string(),
                prompt: format!("Campaign brief: {}", plan.brief),
                dry_run: true,
                review: true,
                selector: None,
                canvas_node: None,
                json_pointer: None,
            };
            let proposal = self.record_review_proposal(
                &generate_request,
                patch,
                json!({
                    "mode": "campaign-review",
                    "campaign_plan_id": plan.id,
                    "design_language_ref": plan.design_language_ref,
                    "summary_zh": "Campaign fixture 已生成待审阅的 artifact 修改。",
                    "verify_notes": ["逐个接受或拒绝 artifact proposal。"]
                }),
                Some(new_source),
                None,
            )?;
            artifact_runs.push(ProjectCampaignArtifactRunV1 {
                artifact_id: artifact.id.clone(),
                source_path: artifact.source_path.clone(),
                review_run_id: proposal.run.id.clone(),
                status: proposal.run.status.clone(),
            });
            proposals.push(proposal);
        }
        let run = ProjectCampaignRunV1 {
            schema_version: CAMPAIGN_RUN_SCHEMA_VERSION.to_string(),
            id: new_id("camp"),
            project_id: plan.project_id.clone(),
            plan_id: plan.id.clone(),
            brief: plan.brief.clone(),
            design_language_ref: plan.design_language_ref.clone(),
            status: "proposed".to_string(),
            artifact_runs,
            generated_at: now_ms(),
        };
        let run_path = self.write_campaign_run(&run)?;
        Ok(ProjectCampaignGenerateResultV1 {
            run,
            run_path,
            plan,
            proposals,
        })
    }

    pub fn campaign_show(&self, run_id: &str) -> ProjectPackageResult<ProjectCampaignRunV1> {
        let path = self
            .root()
            .join(CAPY_DIR)
            .join("campaigns")
            .join(format!("{run_id}.json"));
        let raw = fs::read_to_string(&path).map_err(|source| ProjectPackageError::Io {
            context: format!("read {}", path.display()),
            source,
        })?;
        serde_json::from_str(&raw).map_err(|source| ProjectPackageError::Json {
            context: format!("parse {}", path.display()),
            source,
        })
    }

    fn write_campaign_run(&self, run: &ProjectCampaignRunV1) -> ProjectPackageResult<String> {
        let relative = format!("{CAPY_DIR}/campaigns/{}.json", run.id);
        self.write_json(&self.root().join(&relative), run)?;
        Ok(relative)
    }
}

fn campaign_targets(
    artifacts: &[ArtifactRefV1],
    requested: &[String],
) -> ProjectPackageResult<Vec<ProjectCampaignTargetV1>> {
    let selected: Vec<&ArtifactRefV1> = if requested.is_empty() {
        artifacts
            .iter()
            .filter(|artifact| is_campaign_kind(&artifact.kind))
            .collect()
    } else {
        requested
            .iter()
            .map(|id| {
                artifacts
                    .iter()
                    .find(|artifact| artifact.id == *id)
                    .ok_or_else(|| {
                        ProjectPackageError::Invalid(format!("unknown campaign artifact id: {id}"))
                    })
            })
            .collect::<ProjectPackageResult<Vec<_>>>()?
    };
    Ok(selected
        .into_iter()
        .map(|artifact| ProjectCampaignTargetV1 {
            artifact_id: artifact.id.clone(),
            kind: artifact.kind.as_str().to_string(),
            title: artifact.title.clone(),
            source_path: artifact.source_path.clone(),
            dependencies: artifact.source_refs.clone(),
            review_mode: "review".to_string(),
            status: "planned".to_string(),
        })
        .collect())
}

fn is_campaign_kind(kind: &ArtifactKind) -> bool {
    matches!(
        kind,
        ArtifactKind::Html
            | ArtifactKind::PosterJson
            | ArtifactKind::PptJson
            | ArtifactKind::CompositionJson
    )
}

fn campaign_fixture_source(
    artifact: &ArtifactRefV1,
    current: &str,
    brief: &str,
) -> ProjectPackageResult<String> {
    if artifact.kind == ArtifactKind::Html {
        return fixture_source(artifact, current, brief);
    }
    let mut json_value =
        serde_json::from_str::<Value>(current).map_err(|source| ProjectPackageError::Json {
            context: format!("parse campaign JSON artifact {}", artifact.source_path),
            source,
        })?;
    if let Some(object) = json_value.as_object_mut() {
        object.insert("capy_campaign_brief".to_string(), json!(brief));
        object.insert(
            "capy_campaign_status".to_string(),
            json!("fixture-proposed"),
        );
    }
    serde_json::to_string_pretty(&json_value).map_err(|source| ProjectPackageError::Json {
        context: format!("serialize campaign JSON artifact {}", artifact.source_path),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn fixture_campaign_generates_review_runs() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        copy_dir(&fixture_root(), temp.path())?;
        let package = ProjectPackage::open(temp.path())?;

        let result = package.campaign_generate(ProjectCampaignRequestV1 {
            brief: "Launch a coherent AI design campaign".to_string(),
            artifact_ids: Vec::new(),
        })?;

        assert_eq!(result.run.status, "proposed");
        assert!(result.proposals.len() >= 3);
        assert!(
            result
                .proposals
                .iter()
                .all(|proposal| proposal.run.status == "proposed")
        );
        assert!(temp.path().join(result.run_path).is_file());
        Ok(())
    }

    fn fixture_root() -> std::path::PathBuf {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/project/html-context");
        if root.exists() {
            return root;
        }
        std::path::PathBuf::from("fixtures/project/html-context")
    }

    fn copy_dir(from: &std::path::Path, to: &std::path::Path) -> io::Result<()> {
        fs::create_dir_all(to)?;
        for entry in fs::read_dir(from)? {
            let entry = entry?;
            let target = to.join(entry.file_name());
            if entry.file_type()?.is_dir() {
                copy_dir(&entry.path(), &target)?;
            } else {
                fs::copy(entry.path(), target)?;
            }
        }
        Ok(())
    }
}
