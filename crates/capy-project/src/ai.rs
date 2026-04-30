use serde_json::{Value, json};

use crate::design_language::selected_design_assets;
use crate::model::{
    PATCH_SCHEMA_VERSION, PROJECT_AI_PROMPT_SCHEMA_VERSION, PROJECT_AI_RESPONSE_SCHEMA_VERSION,
    PatchDocumentV1, ProjectAiPromptV1, ProjectAiResponseV1, ProjectGenerateRequestV1,
    ReplaceExactTextOperationV1,
};
use crate::package::{
    ProjectPackage, ProjectPackageError, ProjectPackageResult, new_id, now_ms, read_to_string,
};

impl ProjectPackage {
    pub fn build_ai_prompt(
        &self,
        request: &ProjectGenerateRequestV1,
    ) -> ProjectPackageResult<ProjectAiPromptV1> {
        let manifest = self.project_manifest()?;
        let artifact = self.artifact(&request.artifact_id)?;
        let current_source = self.read_artifact_source(&artifact.id)?;
        let design_language = self.design_language()?;
        let design_language_summary = self.design_language_summary_for(&design_language);
        let selected_assets =
            selected_design_assets(&design_language, &artifact.design_language_refs);
        let design_asset_refs = selected_assets
            .iter()
            .map(|asset| {
                format!(
                    "- id: {}; role: {}; kind: {}; path: {}; title: {}",
                    asset.id,
                    asset.role.as_deref().unwrap_or("context"),
                    asset.kind,
                    asset.path,
                    asset.title
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let design_sources = selected_assets
            .iter()
            .map(|asset| bounded_design_source(self.root(), asset))
            .collect::<ProjectPackageResult<Vec<_>>>()?
            .join("\n");
        let output_schema = project_ai_output_schema();
        let prompt = format!(
            r#"You are Capybara Project AI. Return only valid JSON that matches the supplied schema.

Task:
- Update exactly one project source artifact.
- Preserve the artifact kind and produce a complete replacement source file, not a diff.
- Follow the project design language, tokens, and examples.
- Do not edit .capy metadata, generated evidence, or derived export files.
- Do not invent external network assets.
- Keep paths, IDs, schema names, and technical identifiers literal.

Project:
- name: {project_name}
- id: {project_id}

Active design language package:
- design_language_ref: {design_language_ref}
- name: {design_language_name}
- version: {design_language_version}
- summary: {design_language_summary_text}
- asset_count: {design_language_asset_count}
- token_count: {design_language_token_count}
- reference_image_count: {design_language_reference_image_count}
- rule_count: {design_language_rule_count}
- example_count: {design_language_example_count}

Target artifact:
- artifact_id: {artifact_id}
- kind: {artifact_kind}
- source_path: {source_path}
- title: {artifact_title}

User request:
{user_prompt}

Selected design language asset refs:
{design_asset_refs}

Bounded design language context excerpts:
{design_sources}

Current source file:
----- BEGIN {source_path} -----
{current_source}
----- END {source_path} -----

Output JSON rules:
- schema_version must be "capy.project-ai-response.v1".
- artifacts must contain exactly one item for artifact_id "{artifact_id}" and source_path "{source_path}".
- new_source must be the complete updated file contents.
- summary_zh should explain the visible change in concise Chinese.
- verify_notes should list practical verification checks in Chinese.
"#,
            project_name = manifest.name,
            project_id = manifest.id,
            design_language_ref = design_language_summary.design_language_ref,
            design_language_name = design_language_summary.name,
            design_language_version = design_language_summary.version,
            design_language_summary_text = design_language_summary.summary,
            design_language_asset_count = design_language_summary.asset_count,
            design_language_token_count = design_language_summary.token_count,
            design_language_reference_image_count = design_language_summary.reference_image_count,
            design_language_rule_count = design_language_summary.rule_count,
            design_language_example_count = design_language_summary.example_count,
            artifact_id = artifact.id,
            artifact_kind = artifact.kind.as_str(),
            source_path = artifact.source_path,
            artifact_title = artifact.title,
            user_prompt = request.prompt,
            design_asset_refs = if design_asset_refs.is_empty() {
                "(No design-language asset refs selected.)".to_string()
            } else {
                design_asset_refs
            },
            design_sources = if design_sources.is_empty() {
                "(No design-language files registered.)".to_string()
            } else {
                design_sources
            },
            current_source = current_source,
        );
        Ok(ProjectAiPromptV1 {
            schema_version: PROJECT_AI_PROMPT_SCHEMA_VERSION.to_string(),
            context_id: new_id("ctx"),
            project_id: manifest.id,
            artifact_id: artifact.id,
            source_path: artifact.source_path,
            provider: request.provider.clone(),
            design_language_ref: design_language_summary.design_language_ref.clone(),
            design_language_summary,
            prompt,
            output_schema,
            generated_at: now_ms(),
        })
    }

    pub fn patch_from_ai_response(
        &self,
        target_artifact_id: &str,
        input_context_ref: Option<String>,
        actor: String,
        response: ProjectAiResponseV1,
    ) -> ProjectPackageResult<PatchDocumentV1> {
        if response.schema_version != PROJECT_AI_RESPONSE_SCHEMA_VERSION {
            return Err(ProjectPackageError::Invalid(format!(
                "unsupported project AI response schema_version: {}",
                response.schema_version
            )));
        }
        if response.artifacts.len() != 1 {
            return Err(ProjectPackageError::Invalid(format!(
                "project AI response must contain exactly one artifact, got {}",
                response.artifacts.len()
            )));
        }
        let generated = response.artifacts.into_iter().next().ok_or_else(|| {
            ProjectPackageError::Invalid("project AI response missing artifact".to_string())
        })?;
        if generated.artifact_id != target_artifact_id {
            return Err(ProjectPackageError::Invalid(format!(
                "project AI response artifact {} does not match target {}",
                generated.artifact_id, target_artifact_id
            )));
        }
        if generated.new_source.trim().is_empty() {
            return Err(ProjectPackageError::Invalid(
                "project AI response new_source must not be empty".to_string(),
            ));
        }
        let artifact = self.artifact(target_artifact_id)?;
        if generated.source_path != artifact.source_path {
            return Err(ProjectPackageError::Invalid(format!(
                "project AI response source_path {} does not match artifact source_path {}",
                generated.source_path, artifact.source_path
            )));
        }
        let old_source = self.read_artifact_source(target_artifact_id)?;
        if old_source == generated.new_source {
            return Err(ProjectPackageError::Invalid(
                "project AI response did not change the source".to_string(),
            ));
        }
        Ok(PatchDocumentV1 {
            schema_version: PATCH_SCHEMA_VERSION.to_string(),
            project_id: Some(self.project_manifest()?.id),
            input_context_ref,
            actor: Some(actor),
            operations: vec![ReplaceExactTextOperationV1 {
                op: "replace_exact_text".to_string(),
                artifact_id: target_artifact_id.to_string(),
                source_path: Some(artifact.source_path),
                old_text: old_source,
                new_text: generated.new_source,
                selector_hint: None,
            }],
        })
    }
}

fn bounded_design_source(
    root: &std::path::Path,
    asset: &crate::model::DesignLanguageAssetV1,
) -> ProjectPackageResult<String> {
    let path = root.join(&asset.path);
    let source = read_to_string(&path, "read design language source")?;
    let excerpt = if source.chars().count() > 2400 {
        format!(
            "{}...\n[truncated to 2400 chars]",
            source.chars().take(2400).collect::<String>()
        )
    } else {
        source
    };
    Ok(format!(
        "### {} ({}, role {}, id {}, path {})\n{}\n",
        asset.title,
        asset.kind,
        asset.role.as_deref().unwrap_or("context"),
        asset.id,
        asset.path,
        excerpt
    ))
}

pub fn project_ai_output_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["schema_version", "summary_zh", "artifacts", "verify_notes"],
        "properties": {
            "schema_version": {
                "type": "string",
                "enum": [PROJECT_AI_RESPONSE_SCHEMA_VERSION]
            },
            "summary_zh": {
                "type": "string",
                "minLength": 1
            },
            "artifacts": {
                "type": "array",
                "minItems": 1,
                "maxItems": 1,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["artifact_id", "source_path", "new_source"],
                    "properties": {
                        "artifact_id": { "type": "string", "minLength": 1 },
                        "source_path": { "type": "string", "minLength": 1 },
                        "new_source": { "type": "string", "minLength": 1 }
                    }
                }
            },
            "verify_notes": {
                "type": "array",
                "items": { "type": "string" }
            }
        }
    })
}

pub fn parse_project_ai_response(value: &Value) -> ProjectPackageResult<ProjectAiResponseV1> {
    if value.get("schema_version").and_then(Value::as_str)
        == Some(PROJECT_AI_RESPONSE_SCHEMA_VERSION)
    {
        return serde_json::from_value(value.clone()).map_err(|source| ProjectPackageError::Json {
            context: "parse project AI response".to_string(),
            source,
        });
    }
    for key in ["primary_content", "content"] {
        if let Some(text) = value.get(key).and_then(Value::as_str) {
            let cleaned = strip_json_fence(text);
            if let Ok(response) = serde_json::from_str::<ProjectAiResponseV1>(&cleaned) {
                return Ok(response);
            }
        }
    }
    Err(ProjectPackageError::Invalid(
        "SDK output did not contain a capy.project-ai-response.v1 JSON response".to_string(),
    ))
}

fn strip_json_fence(text: &str) -> String {
    let trimmed = text.trim();
    if !trimmed.starts_with("```") {
        return trimmed.to_string();
    }
    let without_open = trimmed
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_start();
    without_open.trim_end_matches("```").trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::json;

    use crate::model::{ArtifactKind, PROJECT_AI_RESPONSE_SCHEMA_VERSION, ProjectAiResponseV1};
    use crate::{ProjectGenerateRequestV1, ProjectPackage, parse_project_ai_response};

    #[test]
    fn ai_prompt_includes_design_language_and_source() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let project = ProjectPackage::init(temp.path(), Some("AI Prompt Test".to_string()))?;
        fs::write(temp.path().join("tokens.css"), ":root { --brand: red; }")?;
        fs::write(temp.path().join("index.html"), "<h1>Before</h1>")?;
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
            vec![design.id],
        )?;

        let prompt = project.build_ai_prompt(&ProjectGenerateRequestV1 {
            artifact_id: artifact.id,
            provider: "codex".to_string(),
            prompt: "Improve the headline".to_string(),
            dry_run: true,
            review: false,
        })?;

        assert!(prompt.prompt.contains(":root { --brand: red; }"));
        assert!(prompt.prompt.contains("<h1>Before</h1>"));
        assert!(prompt.prompt.contains("Improve the headline"));
        assert_eq!(prompt.output_schema["type"], "object");
        Ok(())
    }

    #[test]
    fn ai_response_becomes_exact_text_patch() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let project = ProjectPackage::init(temp.path(), Some("AI Patch Test".to_string()))?;
        fs::write(temp.path().join("index.html"), "<h1>Before</h1>")?;
        let artifact =
            project.add_artifact(ArtifactKind::Html, "index.html", "Home".to_string(), vec![])?;
        let response = ProjectAiResponseV1 {
            schema_version: PROJECT_AI_RESPONSE_SCHEMA_VERSION.to_string(),
            summary_zh: "改了标题".to_string(),
            artifacts: vec![crate::ProjectAiArtifactV1 {
                artifact_id: artifact.id.clone(),
                source_path: "index.html".to_string(),
                new_source: "<h1>After</h1>".to_string(),
            }],
            verify_notes: vec![],
        };

        let patch = project.patch_from_ai_response(
            &artifact.id,
            Some("ctx_1".to_string()),
            "test".to_string(),
            response,
        )?;

        assert_eq!(patch.operations.len(), 1);
        assert_eq!(patch.operations[0].old_text, "<h1>Before</h1>");
        assert_eq!(patch.operations[0].new_text, "<h1>After</h1>");
        Ok(())
    }

    #[test]
    fn parses_sdk_primary_content_json() -> Result<(), Box<dyn std::error::Error>> {
        let output = json!({
            "ok": true,
            "primary_content": "{\"schema_version\":\"capy.project-ai-response.v1\",\"summary_zh\":\"ok\",\"artifacts\":[],\"verify_notes\":[]}"
        });

        let parsed = parse_project_ai_response(&output)?;

        assert_eq!(parsed.schema_version, PROJECT_AI_RESPONSE_SCHEMA_VERSION);
        assert_eq!(parsed.summary_zh, "ok");
        Ok(())
    }
}
