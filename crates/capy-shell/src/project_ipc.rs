use std::fs;
use std::path::PathBuf;

use capy_contracts::ipc::{IpcRequest, IpcResponse};
use capy_contracts::project::{
    OP_ARTIFACT_READ, OP_ARTIFACT_REGISTER, OP_CONTEXT_BUILD, OP_PATCH_APPLY, OP_PROJECT_GENERATE,
    OP_PROJECT_INSPECT, OP_PROJECT_WORKBENCH,
};
use capy_project::{
    ArtifactKind, ContextBuildRequest, PatchDocumentV1, ProjectGenerateRequestV1, ProjectPackage,
};
use serde_json::{Value, json};

pub(crate) fn handles(op: &str) -> bool {
    matches!(
        op,
        OP_PROJECT_INSPECT
            | OP_ARTIFACT_REGISTER
            | OP_ARTIFACT_READ
            | OP_CONTEXT_BUILD
            | OP_PATCH_APPLY
            | OP_PROJECT_WORKBENCH
            | OP_PROJECT_GENERATE
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
        OP_PROJECT_GENERATE => project_generate(&request.params),
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
    let package =
        ProjectPackage::open(required_path(params, "project")?).map_err(|err| err.to_string())?;
    let provider = optional_string(params, "provider").unwrap_or_else(|| "fixture".to_string());
    let dry_run = params
        .get("dry_run")
        .or_else(|| params.get("dryRun"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let result = package
        .generate(ProjectGenerateRequestV1 {
            artifact_id: required_string(params, "artifact")?,
            provider,
            prompt: required_string(params, "prompt")?,
            dry_run,
        })
        .map_err(|err| err.to_string())?;
    serde_json::to_value(result).map_err(|err| err.to_string())
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
