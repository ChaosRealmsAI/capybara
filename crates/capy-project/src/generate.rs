use serde_json::{Value, json};

use crate::model::{
    ArtifactKind, ArtifactRefV1, GENERATE_RUN_SCHEMA_VERSION, ProjectGenerateRequestV1,
    ProjectGenerateResultV1, ProjectGenerateRunV1,
};
use crate::package::{ProjectPackage, ProjectPackageError, ProjectPackageResult};
use crate::package::{new_id, now_ms, write_string};

const PROVIDERS: &[&str] = &["fixture", "codex", "claude"];

impl ProjectPackage {
    pub fn generate(
        &self,
        request: ProjectGenerateRequestV1,
    ) -> ProjectPackageResult<ProjectGenerateResultV1> {
        validate_request(&request)?;
        let manifest = self.project_manifest()?;
        let artifact = self.artifact(&request.artifact_id)?;
        let command_preview = command_preview(&request.provider, self.root(), &artifact, &request);
        let generated_at = now_ms();
        let mut run = ProjectGenerateRunV1 {
            schema_version: GENERATE_RUN_SCHEMA_VERSION.to_string(),
            id: new_id("gen"),
            project_id: manifest.id,
            artifact_id: artifact.id.clone(),
            provider: request.provider.clone(),
            prompt: request.prompt.clone(),
            status: if request.dry_run {
                "planned".to_string()
            } else {
                "completed".to_string()
            },
            trace_id: new_id("trace"),
            dry_run: request.dry_run,
            command_preview,
            changed_artifact_refs: Vec::new(),
            evidence_refs: Vec::new(),
            output: None,
            error: None,
            generated_at,
        };

        if request.dry_run || request.provider != "fixture" {
            run.output = Some(json!({
                "mode": "plan",
                "message": "No source file was changed. Use --write with provider fixture for no-spend writes, or run command_preview for a live CLI provider."
            }));
            return Ok(ProjectGenerateResultV1 {
                run,
                run_path: None,
                artifact: Some(artifact),
                preview_source: None,
            });
        }

        let new_source = fixture_source(
            &artifact,
            &self.read_artifact_source(&artifact.id)?,
            &request.prompt,
        )?;
        let source_path = self.source_path_for(&artifact, None)?;
        write_string(&source_path, &new_source)?;
        run.changed_artifact_refs.push(artifact.id.clone());
        let run_path = self.write_generate_run(&run)?;
        let updated_artifact =
            self.mark_generated_artifact(&artifact.id, &run_path, generated_at)?;
        Ok(ProjectGenerateResultV1 {
            run,
            run_path: Some(run_path),
            artifact: Some(updated_artifact),
            preview_source: Some(new_source),
        })
    }

    fn write_generate_run(&self, run: &ProjectGenerateRunV1) -> ProjectPackageResult<String> {
        let relative = format!(".capy/runs/{}.json", run.id);
        self.write_json(&self.root().join(&relative), run)?;
        Ok(relative)
    }

    pub fn record_external_generate_run(
        &self,
        run: ProjectGenerateRunV1,
        preview_source: Option<String>,
        mark_artifact: bool,
    ) -> ProjectPackageResult<ProjectGenerateResultV1> {
        let manifest = self.project_manifest()?;
        if run.project_id != manifest.id {
            return Err(ProjectPackageError::Invalid(format!(
                "generate run project_id {} does not match {}",
                run.project_id, manifest.id
            )));
        }
        let run_path = self.write_generate_run(&run)?;
        let artifact = if mark_artifact {
            self.mark_generated_artifact(&run.artifact_id, &run_path, run.generated_at)?
        } else {
            self.artifact(&run.artifact_id)?
        };
        Ok(ProjectGenerateResultV1 {
            run,
            run_path: Some(run_path),
            artifact: Some(artifact),
            preview_source,
        })
    }

    fn mark_generated_artifact(
        &self,
        artifact_id: &str,
        run_path: &str,
        generated_at: u64,
    ) -> ProjectPackageResult<ArtifactRefV1> {
        let mut registry = self.artifacts()?;
        let artifact = registry
            .artifacts
            .iter_mut()
            .find(|artifact| artifact.id == artifact_id)
            .ok_or_else(|| {
                ProjectPackageError::Invalid(format!("unknown artifact id: {artifact_id}"))
            })?;
        if !artifact.evidence_refs.iter().any(|value| value == run_path) {
            artifact.evidence_refs.push(run_path.to_string());
        }
        artifact.updated_at = generated_at;
        artifact.provenance = Some(json!({
            "source": "capy.project.generate",
            "last_run": run_path
        }));
        let updated = artifact.clone();
        self.write_artifacts(&registry)?;
        self.touch_project_manifest()?;
        Ok(updated)
    }
}

fn validate_request(request: &ProjectGenerateRequestV1) -> ProjectPackageResult<()> {
    if request.artifact_id.trim().is_empty() {
        return Err(ProjectPackageError::Invalid(
            "artifact id is required".to_string(),
        ));
    }
    if request.prompt.trim().is_empty() {
        return Err(ProjectPackageError::Invalid(
            "prompt is required".to_string(),
        ));
    }
    if !PROVIDERS.contains(&request.provider.as_str()) {
        return Err(ProjectPackageError::Invalid(format!(
            "invalid provider {}; expected fixture, codex, or claude",
            request.provider
        )));
    }
    Ok(())
}

fn command_preview(
    provider: &str,
    root: &std::path::Path,
    artifact: &ArtifactRefV1,
    request: &ProjectGenerateRequestV1,
) -> Vec<String> {
    if provider == "fixture" {
        return vec![
            "capy".to_string(),
            "project".to_string(),
            "generate".to_string(),
            "--provider".to_string(),
            "fixture".to_string(),
            "--artifact".to_string(),
            artifact.id.clone(),
        ];
    }
    vec![
        "target/debug/capy".to_string(),
        "agent".to_string(),
        "sdk".to_string(),
        "run".to_string(),
        "--provider".to_string(),
        provider.to_string(),
        "--cwd".to_string(),
        root.display().to_string(),
        "--write-code".to_string(),
        "--prompt".to_string(),
        live_prompt(artifact, &request.prompt),
    ]
}

fn live_prompt(artifact: &ArtifactRefV1, prompt: &str) -> String {
    format!(
        "Use Capybara project context. Update only artifact {} at {}. User request: {}",
        artifact.id, artifact.source_path, prompt
    )
}

fn fixture_source(
    artifact: &ArtifactRefV1,
    current: &str,
    prompt: &str,
) -> ProjectPackageResult<String> {
    let safe_prompt = prompt.replace('<', "&lt;").replace('>', "&gt;");
    match artifact.kind {
        ArtifactKind::Html => Ok(current.replace(
            "</body>",
            &format!(
                "  <section data-capy-generated=\"fixture\"><h2>Capybara CLI draft</h2><p>{safe_prompt}</p></section>\n</body>"
            ),
        )),
        ArtifactKind::Markdown => Ok(format!(
            "{current}\n\n## Capybara CLI Draft\n\n{prompt}\n"
        )),
        ArtifactKind::Image => Ok(current.replace(
            "</svg>",
            &format!("<desc>Capybara fixture generation: {safe_prompt}</desc></svg>"),
        )),
        ArtifactKind::PosterJson | ArtifactKind::PptJson | ArtifactKind::CompositionJson => {
            let mut value: Value = serde_json::from_str(current).map_err(|source| {
                ProjectPackageError::Json {
                    context: format!("parse artifact JSON {}", artifact.source_path),
                    source,
                }
            })?;
            value["capy_generation"] = json!({
                "provider": "fixture",
                "prompt": prompt,
                "generated_at": now_ms()
            });
            serde_json::to_string_pretty(&value)
                .map(|payload| format!("{payload}\n"))
                .map_err(|source| ProjectPackageError::Json {
                    context: format!("serialize artifact JSON {}", artifact.source_path),
                    source,
                })
        }
        _ => Ok(format!("{current}\n\nCapybara fixture generation: {prompt}\n")),
    }
}

#[cfg(test)]
mod tests {
    use crate::{ProjectGenerateRequestV1, ProjectPackage};
    use std::error::Error;
    use std::fs;
    use std::io;

    #[test]
    fn fixture_generation_writes_source_and_run_record() -> Result<(), Box<dyn Error>> {
        let temp = tempfile::tempdir()?;
        copy_dir(&fixture_root(), temp.path())?;
        let package = ProjectPackage::open(temp.path())?;
        let result = package.generate(ProjectGenerateRequestV1 {
            artifact_id: "art_00000000000000000000000000000001".to_string(),
            provider: "fixture".to_string(),
            prompt: "Make launch copy clearer".to_string(),
            dry_run: false,
        })?;
        assert_eq!(result.run.status, "completed");
        let run_path = result
            .run_path
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "missing run path"))?;
        assert!(temp.path().join(run_path).is_file());
        let source = fs::read_to_string(temp.path().join("web/index.html"))?;
        assert!(source.contains("Capybara CLI draft"));
        Ok(())
    }

    fn fixture_root() -> std::path::PathBuf {
        let root = std::path::PathBuf::from("fixtures/project/html-context");
        if root.exists() {
            root
        } else {
            std::path::PathBuf::from("../../fixtures/project/html-context")
        }
    }

    fn copy_dir(from: &std::path::Path, to: &std::path::Path) -> Result<(), Box<dyn Error>> {
        for entry in walkdir(from)? {
            let relative = entry.strip_prefix(from)?;
            let target = to.join(relative);
            if entry.is_dir() {
                fs::create_dir_all(&target)?;
            } else {
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(&entry, &target)?;
            }
        }
        Ok(())
    }

    fn walkdir(root: &std::path::Path) -> Result<Vec<std::path::PathBuf>, Box<dyn Error>> {
        let mut out = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(path) = stack.pop() {
            out.push(path.clone());
            if path.is_dir() {
                for entry in fs::read_dir(path)? {
                    stack.push(entry?.path());
                }
            }
        }
        Ok(out)
    }
}
