use crate::model::{
    PATCH_SCHEMA_VERSION, PROJECT_REVIEW_MODE, PatchDocumentV1, ProjectDiffSummaryV1,
    ProjectGenerateRequestV1, ProjectGenerateResultV1, ProjectGenerateRunV1,
    ProjectRunDecisionResultV1, ProjectRunDecisionV1, ProjectRunReviewV1,
    ReplaceExactTextOperationV1,
};
use crate::package::{
    CAPY_DIR, ProjectPackage, ProjectPackageError, ProjectPackageResult, dedupe_sorted, new_id,
    now_ms,
};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
const STATUS_PROPOSED: &str = "proposed";
const STATUS_ACCEPTED: &str = "accepted";
const STATUS_REJECTED: &str = "rejected";
const STATUS_REVERTED: &str = "reverted";
impl ProjectPackage {
    pub fn record_review_proposal(
        &self,
        request: &ProjectGenerateRequestV1,
        patch: PatchDocumentV1,
        mut output: Value,
        preview_source: Option<String>,
        parent_run_id: Option<String>,
    ) -> ProjectPackageResult<ProjectGenerateResultV1> {
        let inspection = self.inspect()?;
        let summary = diff_summary_from_patch(&patch)?;
        output["mode"] = json!(PROJECT_REVIEW_MODE);
        output["patch"] =
            serde_json::to_value(&patch).map_err(|source| ProjectPackageError::Json {
                context: "serialize review patch".to_string(),
                source,
            })?;
        output["diff_summary"] =
            serde_json::to_value(&summary).map_err(|source| ProjectPackageError::Json {
                context: "serialize review diff summary".to_string(),
                source,
            })?;
        output["review_status"] = json!(STATUS_PROPOSED);
        if let Some(parent) = parent_run_id.as_deref() {
            output["parent_run_id"] = json!(parent);
        }

        let changed_refs = dedupe_sorted(
            patch
                .operations
                .iter()
                .map(|operation| operation.artifact_id.clone())
                .collect(),
        );
        let generated_at = now_ms();
        let review = ProjectRunReviewV1 {
            mode: PROJECT_REVIEW_MODE.to_string(),
            status: STATUS_PROPOSED.to_string(),
            parent_run_id,
            base_hash: summary.old_hash.clone(),
            proposed_hash: summary.new_hash.clone(),
            diff_summary: summary,
            decisions: vec![decision(
                "system",
                "propose",
                STATUS_PROPOSED,
                changed_refs.clone(),
                generated_at,
            )],
        };
        let run = ProjectGenerateRunV1 {
            schema_version: crate::model::GENERATE_RUN_SCHEMA_VERSION.to_string(),
            id: new_id("gen"),
            project_id: inspection.manifest.id,
            artifact_id: request.artifact_id.clone(),
            provider: request.provider.clone(),
            prompt: request.prompt.clone(),
            status: STATUS_PROPOSED.to_string(),
            trace_id: new_id("trace"),
            dry_run: true,
            design_language_ref: Some(
                inspection
                    .design_language_summary
                    .design_language_ref
                    .clone(),
            ),
            design_language_summary: Some(inspection.design_language_summary.clone()),
            command_preview: Vec::new(),
            changed_artifact_refs: changed_refs,
            evidence_refs: Vec::new(),
            output: Some(output),
            review: Some(review),
            error: None,
            generated_at,
        };
        let run_path = self.write_generate_run(&run)?;
        Ok(ProjectGenerateResultV1 {
            run,
            run_path: Some(run_path),
            artifact: Some(self.artifact(&request.artifact_id)?),
            preview_source,
        })
    }

    pub fn list_project_runs(&self) -> ProjectPackageResult<Vec<ProjectGenerateRunV1>> {
        let runs_dir = self.root().join(CAPY_DIR).join("runs");
        if !runs_dir.exists() {
            return Ok(Vec::new());
        }
        let mut runs = Vec::new();
        for entry in fs::read_dir(&runs_dir).map_err(|source| ProjectPackageError::Io {
            context: format!("read {}", runs_dir.display()),
            source,
        })? {
            let path = entry
                .map_err(|source| ProjectPackageError::Io {
                    context: format!("read {}", runs_dir.display()),
                    source,
                })?
                .path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            if let Ok(run) = read_generate_run_path(&path) {
                runs.push(run);
            }
        }
        runs.sort_by_key(|run| run.generated_at);
        Ok(runs)
    }

    pub fn show_project_run(&self, run_id: &str) -> ProjectPackageResult<ProjectGenerateRunV1> {
        self.read_project_generate_run(run_id).map(|(run, _)| run)
    }

    pub fn accept_review_run(
        &self,
        run_id: &str,
        actor: &str,
    ) -> ProjectPackageResult<ProjectRunDecisionResultV1> {
        let (mut run, run_path) = self.read_project_generate_run(run_id)?;
        ensure_status(&run, STATUS_PROPOSED)?;
        let patch = review_patch(&run)?;
        let review = run.review.as_mut().ok_or_else(|| {
            ProjectPackageError::Invalid(format!("run {} has no review state", run.id))
        })?;
        ensure_current_hash(self, &patch, &review.base_hash, "accept")?;
        let patch_result = self.apply_patch(patch, Some(run_path.clone()), false)?;
        let decided_at = now_ms();
        review.status = STATUS_ACCEPTED.to_string();
        review.decisions.push(decision(
            actor,
            "accept",
            STATUS_ACCEPTED,
            run.changed_artifact_refs.clone(),
            decided_at,
        ));
        run.status = STATUS_ACCEPTED.to_string();
        run.dry_run = false;
        run.evidence_refs.push(patch_result.run_path.clone());
        run.changed_artifact_refs = patch_result.run.changed_artifact_refs.clone();
        if let Some(output) = run.output.as_mut() {
            output["review_status"] = json!(STATUS_ACCEPTED);
            output["decision"] = json!("accept");
            output["accepted_patch_run"] =
                serde_json::to_value(&patch_result).map_err(|source| {
                    ProjectPackageError::Json {
                        context: "serialize accepted patch run".to_string(),
                        source,
                    }
                })?;
        }
        self.write_generate_run(&run)?;
        self.mark_generated_artifact(&run.artifact_id, &run_path, decided_at)?;
        let preview_source = Some(self.read_artifact_source(&run.artifact_id)?);
        Ok(ProjectRunDecisionResultV1 {
            run,
            run_path,
            changed_files: patch_result.changed_files,
            preview_source,
        })
    }

    pub fn reject_review_run(
        &self,
        run_id: &str,
        actor: &str,
    ) -> ProjectPackageResult<ProjectRunDecisionResultV1> {
        let (mut run, run_path) = self.read_project_generate_run(run_id)?;
        ensure_status(&run, STATUS_PROPOSED)?;
        let decided_at = now_ms();
        let refs = run.changed_artifact_refs.clone();
        let review = run.review.as_mut().ok_or_else(|| {
            ProjectPackageError::Invalid(format!("run {} has no review state", run.id))
        })?;
        review.status = STATUS_REJECTED.to_string();
        review
            .decisions
            .push(decision(actor, "reject", STATUS_REJECTED, refs, decided_at));
        run.status = STATUS_REJECTED.to_string();
        if let Some(output) = run.output.as_mut() {
            output["review_status"] = json!(STATUS_REJECTED);
            output["decision"] = json!("reject");
        }
        self.write_generate_run(&run)?;
        let preview_source = Some(self.read_artifact_source(&run.artifact_id)?);
        Ok(ProjectRunDecisionResultV1 {
            run,
            run_path,
            changed_files: Vec::new(),
            preview_source,
        })
    }

    pub fn undo_review_run(
        &self,
        run_id: &str,
        actor: &str,
    ) -> ProjectPackageResult<ProjectRunDecisionResultV1> {
        let (mut run, run_path) = self.read_project_generate_run(run_id)?;
        ensure_status(&run, STATUS_ACCEPTED)?;
        let patch = review_patch(&run)?;
        let review = run.review.as_mut().ok_or_else(|| {
            ProjectPackageError::Invalid(format!("run {} has no review state", run.id))
        })?;
        ensure_current_hash(self, &patch, &review.proposed_hash, "undo")?;
        let reverse = reverse_patch(&patch, actor.to_string())?;
        let patch_result = self.apply_patch(reverse, Some(run_path.clone()), false)?;
        let decided_at = now_ms();
        review.status = STATUS_REVERTED.to_string();
        review.decisions.push(decision(
            actor,
            "undo",
            STATUS_REVERTED,
            run.changed_artifact_refs.clone(),
            decided_at,
        ));
        run.status = STATUS_REVERTED.to_string();
        run.evidence_refs.push(patch_result.run_path.clone());
        if let Some(output) = run.output.as_mut() {
            output["review_status"] = json!(STATUS_REVERTED);
            output["decision"] = json!("undo");
            output["undo_patch_run"] = serde_json::to_value(&patch_result).map_err(|source| {
                ProjectPackageError::Json {
                    context: "serialize undo patch run".to_string(),
                    source,
                }
            })?;
        }
        self.write_generate_run(&run)?;
        self.mark_generated_artifact(&run.artifact_id, &run_path, decided_at)?;
        let preview_source = Some(self.read_artifact_source(&run.artifact_id)?);
        Ok(ProjectRunDecisionResultV1 {
            run,
            run_path,
            changed_files: patch_result.changed_files,
            preview_source,
        })
    }

    pub fn retry_review_run(
        &self,
        run_id: &str,
        actor: &str,
    ) -> ProjectPackageResult<ProjectGenerateResultV1> {
        let (mut parent, parent_path) = self.read_project_generate_run(run_id)?;
        if parent.status != STATUS_PROPOSED && parent.status != STATUS_REJECTED {
            return Err(ProjectPackageError::Invalid(format!(
                "run {} cannot be retried from status {}",
                parent.id, parent.status
            )));
        }
        let patch = review_patch(&parent)?;
        let review = parent.review.as_ref().ok_or_else(|| {
            ProjectPackageError::Invalid(format!("run {} has no review state", parent.id))
        })?;
        ensure_current_hash(self, &patch, &review.base_hash, "retry")?;
        let request = ProjectGenerateRequestV1 {
            artifact_id: parent.artifact_id.clone(),
            provider: parent.provider.clone(),
            prompt: parent.prompt.clone(),
            dry_run: true,
            review: true,
            selector: None,
            canvas_node: None,
            json_pointer: None,
        };
        let preview_source = patch
            .operations
            .first()
            .map(|operation| operation.new_text.clone());
        let mut output = parent.output.clone().unwrap_or_else(|| json!({}));
        output["retry_of"] = json!(parent.id.clone());
        output["decision"] = json!("retry");
        let result = self.record_review_proposal(
            &request,
            patch,
            output,
            preview_source,
            Some(parent.id.clone()),
        )?;
        let decided_at = now_ms();
        let refs = parent.changed_artifact_refs.clone();
        if let Some(parent_review) = parent.review.as_mut() {
            parent_review.decisions.push(decision(
                actor,
                "retry",
                &parent.status,
                refs,
                decided_at,
            ));
        }
        self.write_generate_run(&parent)?;
        let _ = parent_path;
        Ok(result)
    }

    fn read_project_generate_run(
        &self,
        run_id: &str,
    ) -> ProjectPackageResult<(ProjectGenerateRunV1, String)> {
        let id = normalize_run_id(run_id)?;
        let relative = format!("{CAPY_DIR}/runs/{id}.json");
        let path = self.root().join(&relative);
        let run = read_generate_run_path(&path)?;
        Ok((run, relative))
    }
}

fn read_generate_run_path(path: &Path) -> ProjectPackageResult<ProjectGenerateRunV1> {
    let raw = fs::read_to_string(path).map_err(|source| ProjectPackageError::Io {
        context: format!("read run {}", path.display()),
        source,
    })?;
    serde_json::from_str(&raw).map_err(|source| ProjectPackageError::Json {
        context: format!("parse run {}", path.display()),
        source,
    })
}

fn normalize_run_id(value: &str) -> ProjectPackageResult<String> {
    let path = PathBuf::from(value);
    let file = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or(value)
        .to_string();
    if file.is_empty()
        || !file
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return Err(ProjectPackageError::Invalid(format!(
            "invalid run id: {value}"
        )));
    }
    Ok(file)
}

fn review_patch(run: &ProjectGenerateRunV1) -> ProjectPackageResult<PatchDocumentV1> {
    let value = run
        .output
        .as_ref()
        .and_then(|output| output.get("patch"))
        .ok_or_else(|| ProjectPackageError::Invalid(format!("run {} has no patch", run.id)))?;
    serde_json::from_value(value.clone()).map_err(|source| ProjectPackageError::Json {
        context: format!("parse review patch for {}", run.id),
        source,
    })
}

fn ensure_status(run: &ProjectGenerateRunV1, wanted: &str) -> ProjectPackageResult<()> {
    if run.status == wanted {
        return Ok(());
    }
    Err(ProjectPackageError::Invalid(format!(
        "run {} status is {}, expected {}",
        run.id, run.status, wanted
    )))
}

fn ensure_current_hash(
    package: &ProjectPackage,
    patch: &PatchDocumentV1,
    expected: &str,
    action: &str,
) -> ProjectPackageResult<()> {
    let operation = single_operation(patch)?;
    let current = package.read_artifact_source(&operation.artifact_id)?;
    let actual = source_hash(&current);
    if actual == expected {
        return Ok(());
    }
    Err(ProjectPackageError::Invalid(format!(
        "{action} refused because source hash changed: expected {expected}, got {actual}"
    )))
}

fn reverse_patch(patch: &PatchDocumentV1, actor: String) -> ProjectPackageResult<PatchDocumentV1> {
    let operation = single_operation(patch)?;
    Ok(PatchDocumentV1 {
        schema_version: PATCH_SCHEMA_VERSION.to_string(),
        project_id: patch.project_id.clone(),
        input_context_ref: patch.input_context_ref.clone(),
        actor: Some(actor),
        operations: vec![ReplaceExactTextOperationV1 {
            op: operation.op.clone(),
            artifact_id: operation.artifact_id.clone(),
            source_path: operation.source_path.clone(),
            old_text: operation.new_text.clone(),
            new_text: operation.old_text.clone(),
            selector_hint: operation.selector_hint.clone(),
        }],
    })
}

fn diff_summary_from_patch(patch: &PatchDocumentV1) -> ProjectPackageResult<ProjectDiffSummaryV1> {
    let operation = single_operation(patch)?;
    let old_hash = source_hash(&operation.old_text);
    let new_hash = source_hash(&operation.new_text);
    let old_lines: Vec<&str> = operation.old_text.lines().collect();
    let new_lines: Vec<&str> = operation.new_text.lines().collect();
    let mut prefix = 0;
    while prefix < old_lines.len()
        && prefix < new_lines.len()
        && old_lines[prefix] == new_lines[prefix]
    {
        prefix += 1;
    }
    let mut suffix = 0;
    while suffix + prefix < old_lines.len()
        && suffix + prefix < new_lines.len()
        && old_lines[old_lines.len() - 1 - suffix] == new_lines[new_lines.len() - 1 - suffix]
    {
        suffix += 1;
    }
    let removed_lines = old_lines.len().saturating_sub(prefix + suffix);
    let added_lines = new_lines.len().saturating_sub(prefix + suffix);
    Ok(ProjectDiffSummaryV1 {
        artifact_id: operation.artifact_id.clone(),
        source_path: operation.source_path.clone().unwrap_or_default(),
        old_hash: old_hash.clone(),
        new_hash: new_hash.clone(),
        old_bytes: operation.old_text.len(),
        new_bytes: operation.new_text.len(),
        removed_lines,
        added_lines,
        changed: old_hash != new_hash,
        old_preview: changed_preview(&old_lines, prefix),
        new_preview: changed_preview(&new_lines, prefix),
    })
}

fn single_operation(patch: &PatchDocumentV1) -> ProjectPackageResult<&ReplaceExactTextOperationV1> {
    if patch.operations.len() != 1 {
        return Err(ProjectPackageError::Invalid(format!(
            "review supports exactly one operation, got {}",
            patch.operations.len()
        )));
    }
    Ok(&patch.operations[0])
}

fn changed_preview(lines: &[&str], index: usize) -> Option<String> {
    lines.get(index).map(|line| truncate(line.trim(), 160))
}

fn truncate(value: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (index, ch) in value.chars().enumerate() {
        if index >= max_chars {
            out.push_str("...");
            return out;
        }
        out.push(ch);
    }
    out
}

fn decision(
    actor: &str,
    decision: &str,
    status: &str,
    artifact_refs: Vec<String>,
    decided_at: u64,
) -> ProjectRunDecisionV1 {
    ProjectRunDecisionV1 {
        actor: actor.to_string(),
        decision: decision.to_string(),
        status: status.to_string(),
        artifact_refs,
        decided_at,
    }
}

fn source_hash(contents: &str) -> String {
    let mut hash = 14_695_981_039_346_656_037_u64;
    for byte in contents.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(1_099_511_628_211);
    }
    format!("fnv1a64-{hash:016x}")
}
