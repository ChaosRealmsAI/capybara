use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use capy_agent_runtime::{AgentSdkRunRequest, run_sdk_json};
use capy_contracts::ipc::{IpcRequest, IpcResponse};
use capy_contracts::project::{
    OP_ARTIFACT_READ, OP_ARTIFACT_REGISTER, OP_CONTEXT_BUILD, OP_PATCH_APPLY,
    OP_PROJECT_CAMPAIGN_GENERATE, OP_PROJECT_CAMPAIGN_PLAN, OP_PROJECT_CAMPAIGN_SHOW,
    OP_PROJECT_GENERATE, OP_PROJECT_INSPECT, OP_PROJECT_RUN_ACCEPT, OP_PROJECT_RUN_LIST,
    OP_PROJECT_RUN_REJECT, OP_PROJECT_RUN_RETRY, OP_PROJECT_RUN_SHOW, OP_PROJECT_RUN_UNDO,
    OP_PROJECT_SURFACE_NODE_UPDATE, OP_PROJECT_SURFACE_NODES, OP_PROJECT_VIDEO_CLIP_FEEDBACK_GET,
    OP_PROJECT_VIDEO_CLIP_FEEDBACK_SET, OP_PROJECT_VIDEO_CLIP_QUEUE_GET,
    OP_PROJECT_VIDEO_CLIP_QUEUE_SET, OP_PROJECT_VIDEO_CLIP_QUEUE_SUGGEST,
    OP_PROJECT_VIDEO_CLIP_SEMANTICS_ANALYZE, OP_PROJECT_VIDEO_CLIP_SEMANTICS_GET,
    OP_PROJECT_WORKBENCH,
};
use capy_project::{
    ArtifactKind, ContextBuildRequest, GENERATE_RUN_SCHEMA_VERSION, PatchDocumentV1,
    ProjectGenerateRequestV1, ProjectGenerateRunV1, ProjectPackage, parse_project_ai_response,
};
use serde_json::{Value, json};
use uuid::Uuid;

pub(crate) fn handles(op: &str) -> bool {
    matches!(
        op,
        OP_PROJECT_INSPECT
            | OP_ARTIFACT_REGISTER
            | OP_ARTIFACT_READ
            | OP_CONTEXT_BUILD
            | OP_PATCH_APPLY
            | OP_PROJECT_WORKBENCH
            | OP_PROJECT_SURFACE_NODES
            | OP_PROJECT_SURFACE_NODE_UPDATE
            | OP_PROJECT_VIDEO_CLIP_QUEUE_GET
            | OP_PROJECT_VIDEO_CLIP_QUEUE_SET
            | OP_PROJECT_VIDEO_CLIP_QUEUE_SUGGEST
            | OP_PROJECT_VIDEO_CLIP_SEMANTICS_GET
            | OP_PROJECT_VIDEO_CLIP_SEMANTICS_ANALYZE
            | OP_PROJECT_VIDEO_CLIP_FEEDBACK_GET
            | OP_PROJECT_VIDEO_CLIP_FEEDBACK_SET
            | OP_PROJECT_GENERATE
            | OP_PROJECT_RUN_LIST
            | OP_PROJECT_RUN_SHOW
            | OP_PROJECT_RUN_ACCEPT
            | OP_PROJECT_RUN_REJECT
            | OP_PROJECT_RUN_RETRY
            | OP_PROJECT_RUN_UNDO
            | OP_PROJECT_CAMPAIGN_PLAN
            | OP_PROJECT_CAMPAIGN_GENERATE
            | OP_PROJECT_CAMPAIGN_SHOW
    )
}

pub(crate) fn response(request: IpcRequest) -> IpcResponse {
    let result = match request.op.as_str() {
        OP_PROJECT_INSPECT => project_inspect(&request.params),
        OP_ARTIFACT_REGISTER => artifact_register(&request.params),
        OP_ARTIFACT_READ => artifact_read(&request.params),
        OP_CONTEXT_BUILD => context_build(&request.params),
        OP_PATCH_APPLY => patch_apply(&request.params),
        OP_PROJECT_WORKBENCH => project_workbench(&request.params),
        OP_PROJECT_SURFACE_NODES => crate::project_ipc_surface::nodes(&request.params),
        OP_PROJECT_SURFACE_NODE_UPDATE => crate::project_ipc_surface::update(&request.params),
        OP_PROJECT_VIDEO_CLIP_QUEUE_GET => crate::project_ipc_clip_queue::get(&request.params),
        OP_PROJECT_VIDEO_CLIP_QUEUE_SET => crate::project_ipc_clip_queue::set(&request.params),
        OP_PROJECT_VIDEO_CLIP_QUEUE_SUGGEST => {
            crate::project_ipc_clip_queue::suggest(&request.params)
        }
        OP_PROJECT_VIDEO_CLIP_SEMANTICS_GET => {
            crate::project_ipc_clip_queue::semantics_get(&request.params)
        }
        OP_PROJECT_VIDEO_CLIP_SEMANTICS_ANALYZE => {
            crate::project_ipc_clip_queue::semantics_analyze(&request.params)
        }
        OP_PROJECT_VIDEO_CLIP_FEEDBACK_GET => {
            crate::project_ipc_clip_queue::feedback_get(&request.params)
        }
        OP_PROJECT_VIDEO_CLIP_FEEDBACK_SET => {
            crate::project_ipc_clip_queue::feedback_set(&request.params)
        }
        OP_PROJECT_GENERATE => project_generate(&request.params),
        OP_PROJECT_RUN_LIST => project_run_list(&request.params),
        OP_PROJECT_RUN_SHOW => project_run_show(&request.params),
        OP_PROJECT_RUN_ACCEPT => project_run_accept(&request.params),
        OP_PROJECT_RUN_REJECT => project_run_reject(&request.params),
        OP_PROJECT_RUN_RETRY => project_run_retry(&request.params),
        OP_PROJECT_RUN_UNDO => project_run_undo(&request.params),
        OP_PROJECT_CAMPAIGN_PLAN => crate::project_ipc_campaign::campaign_plan(&request.params),
        OP_PROJECT_CAMPAIGN_GENERATE => {
            crate::project_ipc_campaign::campaign_generate(&request.params)
        }
        OP_PROJECT_CAMPAIGN_SHOW => crate::project_ipc_campaign::campaign_show(&request.params),
        _ => Err(format!("unknown project op: {}", request.op)),
    };
    match result {
        Ok(data) => IpcResponse::ok(request.req_id, data),
        Err(error) => IpcResponse {
            req_id: request.req_id,
            ok: false,
            data: None,
            error: Some(json!({
                "error": "project operation failed",
                "detail": error,
                "exit_code": 1
            })),
        },
    }
}

fn project_inspect(params: &Value) -> Result<Value, String> {
    let package =
        ProjectPackage::open(required_path(params, "project")?).map_err(|err| err.to_string())?;
    serde_json::to_value(package.inspect().map_err(|err| err.to_string())?)
        .map_err(|err| err.to_string())
}
fn project_workbench(params: &Value) -> Result<Value, String> {
    let package =
        ProjectPackage::open(required_path(params, "project")?).map_err(|err| err.to_string())?;
    serde_json::to_value(package.workbench().map_err(|err| err.to_string())?)
        .map_err(|err| err.to_string())
}
fn project_generate(params: &Value) -> Result<Value, String> {
    let project = required_path(params, "project")?;
    let package = ProjectPackage::open(&project).map_err(|err| err.to_string())?;
    let provider = optional_string(params, "provider").unwrap_or_else(|| "fixture".to_string());
    let dry_run = params
        .get("dry_run")
        .or_else(|| params.get("dryRun"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let live = params.get("live").and_then(Value::as_bool).unwrap_or(false)
        || optional_string(params, "sdk_response").is_some()
        || optional_string(params, "sdkResponse").is_some();
    let request = ProjectGenerateRequestV1 {
        artifact_id: required_string(params, "artifact")?,
        provider,
        prompt: required_string(params, "prompt")?,
        dry_run,
        review: params
            .get("review")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        selector: optional_string(params, "selector"),
        canvas_node: optional_string(params, "canvas_node")
            .or_else(|| optional_string(params, "canvasNode")),
        json_pointer: optional_string(params, "json_pointer")
            .or_else(|| optional_string(params, "jsonPointer")),
    };
    let result = if live {
        project_generate_live(&package, &project, request, params)?
    } else {
        package.generate(request).map_err(|err| err.to_string())?
    };
    serde_json::to_value(result).map_err(|err| err.to_string())
}

fn project_generate_live(
    package: &ProjectPackage,
    project: &std::path::Path,
    request: ProjectGenerateRequestV1,
    params: &Value,
) -> Result<capy_project::ProjectGenerateResultV1, String> {
    if request.provider == "fixture" {
        return Err("live project generation requires provider codex or claude".to_string());
    }
    let project_root = fs::canonicalize(project)
        .map_err(|err| format!("canonicalize project {} failed: {err}", project.display()))?;
    let prompt = package
        .build_ai_prompt(&request)
        .map_err(|err| err.to_string())?;
    let sdk_output = run_sdk_json(AgentSdkRunRequest {
        provider: request.provider.clone(),
        cwd: project_root.clone(),
        prompt: prompt.prompt.clone(),
        output_schema: prompt.output_schema.clone(),
        model: optional_string(params, "model"),
        effort: optional_string(params, "effort"),
        fake_response: optional_string(params, "sdk_response")
            .or_else(|| optional_string(params, "sdkResponse"))
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("CAPY_PROJECT_AI_RESPONSE_FIXTURE").map(PathBuf::from)),
    })
    .map_err(|err| err.to_string())?;
    let ai_response = parse_project_ai_response(&sdk_output).map_err(|err| err.to_string())?;
    let mut patch = package
        .patch_from_ai_response(
            &request.artifact_id,
            Some(prompt.context_id.clone()),
            format!("project-ai:{}", request.provider),
            ai_response.clone(),
        )
        .map_err(|err| err.to_string())?;
    if let Some(operation) = patch.operations.first_mut() {
        operation.selector_hint = request
            .selector
            .clone()
            .or_else(|| request.json_pointer.clone());
    }
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
                    "selection_context": prompt.selection_context,
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
        provider: request.provider.clone(),
        prompt: request.prompt.clone(),
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
        command_preview: live_command_preview(
            &request.provider,
            &project_root,
            &request.artifact_id,
        ),
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

fn project_run_list(params: &Value) -> Result<Value, String> {
    let package =
        ProjectPackage::open(required_path(params, "project")?).map_err(|err| err.to_string())?;
    serde_json::to_value(package.list_project_runs().map_err(|err| err.to_string())?)
        .map_err(|err| err.to_string())
}

fn project_run_show(params: &Value) -> Result<Value, String> {
    let package =
        ProjectPackage::open(required_path(params, "project")?).map_err(|err| err.to_string())?;
    serde_json::to_value(
        package
            .show_project_run(&required_string(params, "run_id")?)
            .map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())
}

fn project_run_accept(params: &Value) -> Result<Value, String> {
    let package =
        ProjectPackage::open(required_path(params, "project")?).map_err(|err| err.to_string())?;
    serde_json::to_value(
        package
            .accept_review_run(
                &required_string(params, "run_id")?,
                &optional_string(params, "actor").unwrap_or_else(|| "desktop".to_string()),
            )
            .map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())
}

fn project_run_reject(params: &Value) -> Result<Value, String> {
    let package =
        ProjectPackage::open(required_path(params, "project")?).map_err(|err| err.to_string())?;
    serde_json::to_value(
        package
            .reject_review_run(
                &required_string(params, "run_id")?,
                &optional_string(params, "actor").unwrap_or_else(|| "desktop".to_string()),
            )
            .map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())
}

fn project_run_retry(params: &Value) -> Result<Value, String> {
    let package =
        ProjectPackage::open(required_path(params, "project")?).map_err(|err| err.to_string())?;
    serde_json::to_value(
        package
            .retry_review_run(
                &required_string(params, "run_id")?,
                &optional_string(params, "actor").unwrap_or_else(|| "desktop".to_string()),
            )
            .map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())
}

fn project_run_undo(params: &Value) -> Result<Value, String> {
    let package =
        ProjectPackage::open(required_path(params, "project")?).map_err(|err| err.to_string())?;
    serde_json::to_value(
        package
            .undo_review_run(
                &required_string(params, "run_id")?,
                &optional_string(params, "actor").unwrap_or_else(|| "desktop".to_string()),
            )
            .map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())
}

fn live_command_preview(
    provider: &str,
    project_root: &std::path::Path,
    artifact: &str,
) -> Vec<String> {
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

fn artifact_register(params: &Value) -> Result<Value, String> {
    let package =
        ProjectPackage::open(required_path(params, "project")?).map_err(|err| err.to_string())?;
    let kind = required_string(params, "kind")?
        .parse::<ArtifactKind>()
        .map_err(|err| err.to_string())?;
    let refs = params
        .get("design_refs")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default();
    let artifact = package
        .add_artifact(
            kind,
            required_path(params, "path")?,
            required_string(params, "title")?,
            refs,
        )
        .map_err(|err| err.to_string())?;
    serde_json::to_value(artifact).map_err(|err| err.to_string())
}

fn artifact_read(params: &Value) -> Result<Value, String> {
    let package =
        ProjectPackage::open(required_path(params, "project")?).map_err(|err| err.to_string())?;
    let artifact_id = required_string(params, "artifact")?;
    let source = package
        .read_artifact_source(&artifact_id)
        .map_err(|err| err.to_string())?;
    Ok(json!({
        "artifact": artifact_id,
        "source": source
    }))
}

fn context_build(params: &Value) -> Result<Value, String> {
    let package =
        ProjectPackage::open(required_path(params, "project")?).map_err(|err| err.to_string())?;
    let context = package
        .build_context(ContextBuildRequest {
            artifact_id: required_string(params, "artifact")?,
            selector: optional_string(params, "selector"),
            canvas_node: optional_string(params, "canvas_node")
                .or_else(|| optional_string(params, "canvasNode")),
            json_pointer: optional_string(params, "json_pointer")
                .or_else(|| optional_string(params, "jsonPointer")),
        })
        .map_err(|err| err.to_string())?;
    serde_json::to_value(context).map_err(|err| err.to_string())
}

fn patch_apply(params: &Value) -> Result<Value, String> {
    let package =
        ProjectPackage::open(required_path(params, "project")?).map_err(|err| err.to_string())?;
    let patch_ref = optional_string(params, "patch_path");
    let patch = if let Some(path) = patch_ref.as_deref() {
        let raw = fs::read_to_string(path).map_err(|err| format!("read patch failed: {err}"))?;
        serde_json::from_str::<PatchDocumentV1>(&raw)
            .map_err(|err| format!("parse patch failed: {err}"))?
    } else {
        let Some(value) = params.get("patch") else {
            return Err("missing patch or patch_path".to_string());
        };
        serde_json::from_value::<PatchDocumentV1>(value.clone())
            .map_err(|err| format!("parse patch failed: {err}"))?
    };
    let result = package
        .apply_patch(
            patch,
            patch_ref,
            params
                .get("dry_run")
                .or_else(|| params.get("dryRun"))
                .and_then(Value::as_bool)
                .unwrap_or(false),
        )
        .map_err(|err| err.to_string())?;
    serde_json::to_value(result).map_err(|err| err.to_string())
}

fn required_path(params: &Value, key: &str) -> Result<PathBuf, String> {
    required_string(params, key).map(PathBuf::from)
}

fn required_string(params: &Value, key: &str) -> Result<String, String> {
    params
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| format!("missing required parameter: {key}"))
}

fn optional_string(params: &Value, key: &str) -> Option<String> {
    params
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}
