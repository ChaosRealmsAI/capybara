use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use capy_agent_runtime::{AgentSdkRunRequest, run_sdk_json};
use capy_project::{
    GENERATE_RUN_SCHEMA_VERSION, ProjectGenerateRequestV1, ProjectGenerateRunV1, ProjectPackage,
    parse_project_ai_response,
};
use serde_json::{Value, json};
use uuid::Uuid;

use super::{ProjectGenerateArgs, write_json_file};

pub(super) fn generate_live(
    args: ProjectGenerateArgs,
) -> Result<capy_project::ProjectGenerateResultV1, String> {
    let provider = args.provider.as_str();
    if provider == "fixture" {
        return Err("--live requires --provider codex or --provider claude".to_string());
    }
    let project_root = fs::canonicalize(&args.project).map_err(|err| {
        format!(
            "canonicalize project {} failed: {err}",
            args.project.display()
        )
    })?;
    let package = ProjectPackage::open(&project_root).map_err(|err| err.to_string())?;
    let request = ProjectGenerateRequestV1 {
        artifact_id: args.artifact.clone(),
        provider: provider.to_string(),
        prompt: args.prompt.clone(),
        dry_run: if args.review { true } else { !args.write },
        review: args.review,
    };
    let prompt = package
        .build_ai_prompt(&request)
        .map_err(|err| err.to_string())?;
    if let Some(save_prompt) = args.save_prompt.as_ref() {
        write_json_file(save_prompt, &prompt)?;
    }
    let sdk_output = run_sdk_json(AgentSdkRunRequest {
        provider: provider.to_string(),
        cwd: project_root.clone(),
        prompt: prompt.prompt.clone(),
        output_schema: prompt.output_schema.clone(),
        model: args.model,
        effort: args.effort,
        fake_response: args
            .sdk_response
            .or_else(|| std::env::var_os("CAPY_PROJECT_AI_RESPONSE_FIXTURE").map(PathBuf::from)),
    })
    .map_err(|err| err.to_string())?;
    let ai_response = parse_project_ai_response(&sdk_output).map_err(|err| err.to_string())?;
    let patch = package
        .patch_from_ai_response(
            &request.artifact_id,
            Some(prompt.context_id.clone()),
            format!("project-ai:{provider}"),
            ai_response.clone(),
        )
        .map_err(|err| err.to_string())?;
    let preview_source = ai_response
        .artifacts
        .first()
        .map(|artifact| artifact.new_source.clone());
    if request.review {
        return package
            .record_review_proposal(
                &request,
                patch,
                json!({
                    "context_id": prompt.context_id,
                    "design_language_ref": prompt.design_language_ref,
                    "design_language_summary": prompt.design_language_summary,
                    "summary_zh": ai_response.summary_zh,
                    "verify_notes": ai_response.verify_notes,
                    "sdk": summarize_sdk_output(&sdk_output)
                }),
                preview_source,
                None,
            )
            .map_err(|err| err.to_string());
    }
    let patch_result = package
        .apply_patch(patch.clone(), None, request.dry_run)
        .map_err(|err| err.to_string())?;
    let inspection = package.inspect().map_err(|err| err.to_string())?;
    let run = ProjectGenerateRunV1 {
        schema_version: GENERATE_RUN_SCHEMA_VERSION.to_string(),
        id: new_id("gen"),
        project_id: inspection.manifest.id,
        artifact_id: request.artifact_id.clone(),
        provider: provider.to_string(),
        prompt: request.prompt,
        status: if request.dry_run {
            "planned"
        } else {
            "completed"
        }
        .to_string(),
        trace_id: new_id("trace"),
        dry_run: request.dry_run,
        design_language_ref: Some(prompt.design_language_ref.clone()),
        design_language_summary: Some(prompt.design_language_summary.clone()),
        command_preview: live_command_preview(provider, &project_root, &request.artifact_id),
        changed_artifact_refs: patch_result.run.changed_artifact_refs.clone(),
        evidence_refs: vec![patch_result.run_path.clone()],
        output: Some(json!({
            "mode": "live",
            "context_id": prompt.context_id,
            "design_language_ref": prompt.design_language_ref,
            "design_language_summary": prompt.design_language_summary,
            "summary_zh": ai_response.summary_zh,
            "verify_notes": ai_response.verify_notes,
            "patch_run": patch_result,
            "patch": patch,
            "sdk": summarize_sdk_output(&sdk_output)
        })),
        review: None,
        error: None,
        generated_at: now_ms(),
    };
    package
        .record_external_generate_run(run, preview_source, !request.dry_run)
        .map_err(|err| err.to_string())
}

fn live_command_preview(provider: &str, project_root: &Path, artifact: &str) -> Vec<String> {
    vec![
        "target/debug/capy".to_string(),
        "agent".to_string(),
        "sdk".to_string(),
        "run".to_string(),
        "--provider".to_string(),
        provider.to_string(),
        "--cwd".to_string(),
        project_root.display().to_string(),
        "--output-schema".to_string(),
        "capy.project-ai-response.v1".to_string(),
        "--prompt".to_string(),
        format!("Project artifact {artifact} generation prompt"),
    ]
}

fn summarize_sdk_output(value: &Value) -> Value {
    json!({
        "ok": value.get("ok").and_then(Value::as_bool),
        "provider": value.get("provider").and_then(Value::as_str),
        "thread_id": value.get("thread_id").and_then(Value::as_str),
        "session_id": value.get("session_id").and_then(Value::as_str),
        "usage": value.get("usage").cloned().unwrap_or(Value::Null),
        "total_cost_usd": value.get("total_cost_usd").cloned().unwrap_or(Value::Null)
    })
}

fn new_id(prefix: &str) -> String {
    format!("{prefix}_{}", Uuid::new_v4().simple())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}
