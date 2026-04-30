use std::path::PathBuf;

use crate::model::{PATCH_RUN_SCHEMA_VERSION, PatchApplyResultV1, PatchDocumentV1, PatchRunV1};
use crate::package::{
    ProjectPackage, ProjectPackageError, ProjectPackageResult, changed_file_map, count_matches,
    dedupe_sorted, new_id, now_ms, read_to_string, write_string,
};

pub(crate) fn apply_patch(
    package: &ProjectPackage,
    patch: PatchDocumentV1,
    patch_ref: Option<String>,
    dry_run: bool,
) -> ProjectPackageResult<PatchApplyResultV1> {
    if patch.schema_version != crate::model::PATCH_SCHEMA_VERSION {
        return Err(ProjectPackageError::Invalid(format!(
            "unsupported patch schema_version: {}",
            patch.schema_version
        )));
    }
    if patch.operations.is_empty() {
        return Err(ProjectPackageError::Invalid(
            "patch operations must not be empty".to_string(),
        ));
    }

    let inspection = package.inspect()?;
    if let Some(project_id) = patch.project_id.as_deref() {
        if project_id != inspection.manifest.id {
            return Err(ProjectPackageError::Invalid(format!(
                "patch project_id {project_id} does not match {}",
                inspection.manifest.id
            )));
        }
    }

    let mut changed_artifacts = Vec::new();
    let mut staged_contents = changed_file_map(Vec::new());
    for operation in &patch.operations {
        if operation.op != "replace_exact_text" {
            return Err(ProjectPackageError::Invalid(format!(
                "unsupported patch op: {}",
                operation.op
            )));
        }
        if operation.old_text.is_empty() {
            return Err(ProjectPackageError::Invalid(
                "replace_exact_text old_text must not be empty".to_string(),
            ));
        }
        let artifact = package.artifact(&operation.artifact_id)?;
        let source_path = package.source_path_for(&artifact, operation.source_path.as_deref())?;
        let current = match staged_contents.get(&source_path) {
            Some(contents) => contents.clone(),
            None => read_to_string(&source_path, "read patch source")?,
        };
        let matches = count_matches(&current, &operation.old_text);
        if matches != 1 {
            return Err(ProjectPackageError::Invalid(format!(
                "replace_exact_text for artifact {} expected one match, found {}",
                artifact.id, matches
            )));
        }
        let updated = current.replacen(&operation.old_text, &operation.new_text, 1);
        staged_contents.insert(source_path, updated);
        changed_artifacts.push(artifact.id);
    }

    if !dry_run {
        for (path, contents) in &staged_contents {
            write_string(path, contents)?;
        }
    }

    let changed_files = relative_changed_files(package.root(), staged_contents.keys().cloned())?;
    let run = PatchRunV1 {
        schema_version: PATCH_RUN_SCHEMA_VERSION.to_string(),
        id: new_id("run"),
        project_id: inspection.manifest.id,
        actor: patch.actor.unwrap_or_else(|| "cli".to_string()),
        input_context_ref: patch.input_context_ref,
        patch_refs: patch_ref.into_iter().collect(),
        changed_artifact_refs: dedupe_sorted(changed_artifacts),
        status: if dry_run { "dry-run" } else { "applied" }.to_string(),
        trace_id: new_id("trace"),
        error: None,
        evidence_refs: Vec::new(),
        dry_run,
        generated_at: now_ms(),
    };
    let run_path = package.write_run(&run)?;

    Ok(PatchApplyResultV1 {
        run,
        run_path,
        changed_files,
    })
}

fn relative_changed_files(
    root: &std::path::Path,
    paths: impl Iterator<Item = PathBuf>,
) -> ProjectPackageResult<Vec<String>> {
    let mut files = Vec::new();
    for path in paths {
        let relative = path.strip_prefix(root).map_err(|err| {
            ProjectPackageError::Invalid(format!(
                "changed file {} is not under project root: {err}",
                path.display()
            ))
        })?;
        files.push(relative.display().to_string());
    }
    files.sort();
    Ok(files)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::ProjectPackage;
    use crate::model::{
        ArtifactKind, PATCH_SCHEMA_VERSION, PatchDocumentV1, ReplaceExactTextOperationV1,
    };

    #[test]
    fn dry_run_records_run_without_changing_source() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let project = ProjectPackage::init(temp.path(), Some("Patch Test".to_string()))?;
        fs::create_dir_all(temp.path().join("web"))?;
        fs::write(temp.path().join("web/index.html"), "<h1>Before</h1>")?;
        let artifact = project.add_artifact(
            ArtifactKind::Html,
            "web/index.html",
            "Home".to_string(),
            Vec::new(),
        )?;
        let patch = PatchDocumentV1 {
            schema_version: PATCH_SCHEMA_VERSION.to_string(),
            project_id: None,
            input_context_ref: None,
            actor: Some("test".to_string()),
            operations: vec![ReplaceExactTextOperationV1 {
                op: "replace_exact_text".to_string(),
                artifact_id: artifact.id,
                source_path: None,
                old_text: "Before".to_string(),
                new_text: "After".to_string(),
                selector_hint: None,
            }],
        };

        let result = project.apply_patch(patch, None, true)?;

        assert_eq!(result.run.status, "dry-run");
        assert_eq!(
            fs::read_to_string(temp.path().join("web/index.html"))?,
            "<h1>Before</h1>"
        );
        assert!(temp.path().join(result.run_path).exists());
        Ok(())
    }

    #[test]
    fn apply_requires_unique_old_text() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let project = ProjectPackage::init(temp.path(), Some("Patch Test".to_string()))?;
        fs::write(temp.path().join("index.html"), "Same Same")?;
        let artifact = project.add_artifact(
            ArtifactKind::Html,
            "index.html",
            "Home".to_string(),
            Vec::new(),
        )?;
        let patch = PatchDocumentV1 {
            schema_version: PATCH_SCHEMA_VERSION.to_string(),
            project_id: None,
            input_context_ref: None,
            actor: None,
            operations: vec![ReplaceExactTextOperationV1 {
                op: "replace_exact_text".to_string(),
                artifact_id: artifact.id,
                source_path: None,
                old_text: "Same".to_string(),
                new_text: "After".to_string(),
                selector_hint: None,
            }],
        };

        let error = project.apply_patch(patch, None, false).err();

        assert!(format!("{error:?}").contains("expected one match, found 2"));
        Ok(())
    }
}

#[cfg(test)]
mod package_tests {
    use std::fs;

    use crate::ProjectPackage;
    use crate::model::{ArtifactKind, ContextBuildRequest};

    #[test]
    fn project_context_includes_artifact_and_design_assets()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let project = ProjectPackage::init(temp.path(), Some("Context Test".to_string()))?;
        fs::write(temp.path().join("tokens.css"), ":root { --brand: red; }")?;
        fs::write(temp.path().join("index.html"), "<h1>Hello</h1>")?;
        let design = project.add_design_asset(
            "css".to_string(),
            Some("tokens".to_string()),
            "tokens.css",
            "Tokens".to_string(),
            None,
        )?;
        let artifact = project.add_artifact(
            ArtifactKind::Html,
            "index.html",
            "Home".to_string(),
            vec![design.id.clone()],
        )?;

        let context = project.build_context(ContextBuildRequest {
            artifact_id: artifact.id,
            selector: Some("h1".to_string()),
            canvas_node: Some("42".to_string()),
        })?;

        assert_eq!(context.selector.as_deref(), Some("h1"));
        assert_eq!(context.design_language_refs.len(), 1);
        assert_eq!(context.design_language_refs[0].id, design.id);
        Ok(())
    }
}
