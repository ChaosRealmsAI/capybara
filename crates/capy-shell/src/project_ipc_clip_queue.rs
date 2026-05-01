use std::path::PathBuf;

use capy_contracts::project::{
    OP_PROJECT_VIDEO_CLIP_FEEDBACK_GET, OP_PROJECT_VIDEO_CLIP_FEEDBACK_SET,
    OP_PROJECT_VIDEO_CLIP_PROPOSAL_DECIDE, OP_PROJECT_VIDEO_CLIP_PROPOSAL_GENERATE,
    OP_PROJECT_VIDEO_CLIP_PROPOSAL_GET, OP_PROJECT_VIDEO_CLIP_PROPOSAL_HISTORY_GET,
    OP_PROJECT_VIDEO_CLIP_QUEUE_GET, OP_PROJECT_VIDEO_CLIP_QUEUE_SET,
    OP_PROJECT_VIDEO_CLIP_QUEUE_SUGGEST, OP_PROJECT_VIDEO_CLIP_SEMANTICS_ANALYZE,
    OP_PROJECT_VIDEO_CLIP_SEMANTICS_GET,
};
use capy_project::{ProjectPackage, ProjectVideoClipQueueItemV1};
use serde_json::Value;

pub(crate) fn handles(op: &str) -> bool {
    matches!(
        op,
        OP_PROJECT_VIDEO_CLIP_QUEUE_GET
            | OP_PROJECT_VIDEO_CLIP_QUEUE_SET
            | OP_PROJECT_VIDEO_CLIP_QUEUE_SUGGEST
            | OP_PROJECT_VIDEO_CLIP_SEMANTICS_GET
            | OP_PROJECT_VIDEO_CLIP_SEMANTICS_ANALYZE
            | OP_PROJECT_VIDEO_CLIP_FEEDBACK_GET
            | OP_PROJECT_VIDEO_CLIP_FEEDBACK_SET
            | OP_PROJECT_VIDEO_CLIP_PROPOSAL_GET
            | OP_PROJECT_VIDEO_CLIP_PROPOSAL_GENERATE
            | OP_PROJECT_VIDEO_CLIP_PROPOSAL_HISTORY_GET
            | OP_PROJECT_VIDEO_CLIP_PROPOSAL_DECIDE
    )
}

pub(crate) fn response(op: &str, params: &Value) -> Result<Value, String> {
    match op {
        OP_PROJECT_VIDEO_CLIP_QUEUE_GET => get(params),
        OP_PROJECT_VIDEO_CLIP_QUEUE_SET => set(params),
        OP_PROJECT_VIDEO_CLIP_QUEUE_SUGGEST => suggest(params),
        OP_PROJECT_VIDEO_CLIP_SEMANTICS_GET => semantics_get(params),
        OP_PROJECT_VIDEO_CLIP_SEMANTICS_ANALYZE => semantics_analyze(params),
        OP_PROJECT_VIDEO_CLIP_FEEDBACK_GET => feedback_get(params),
        OP_PROJECT_VIDEO_CLIP_FEEDBACK_SET => feedback_set(params),
        OP_PROJECT_VIDEO_CLIP_PROPOSAL_GET => proposal_get(params),
        OP_PROJECT_VIDEO_CLIP_PROPOSAL_GENERATE => proposal_generate(params),
        OP_PROJECT_VIDEO_CLIP_PROPOSAL_HISTORY_GET => proposal_history_get(params),
        OP_PROJECT_VIDEO_CLIP_PROPOSAL_DECIDE => proposal_decide(params),
        _ => Err(format!("unknown video clip queue op: {op}")),
    }
}

pub(crate) fn get(params: &Value) -> Result<Value, String> {
    let package =
        ProjectPackage::open(required_path(params, "project")?).map_err(|err| err.to_string())?;
    serde_json::to_value(package.video_clip_queue().map_err(|err| err.to_string())?)
        .map_err(|err| err.to_string())
}

pub(crate) fn set(params: &Value) -> Result<Value, String> {
    let package =
        ProjectPackage::open(required_path(params, "project")?).map_err(|err| err.to_string())?;
    let items_value = params
        .get("items")
        .or_else(|| params.get("queue"))
        .cloned()
        .ok_or_else(|| "missing required parameter: items".to_string())?;
    let items = serde_json::from_value::<Vec<ProjectVideoClipQueueItemV1>>(items_value)
        .map_err(|err| format!("parse video clip queue items failed: {err}"))?;
    serde_json::to_value(
        package
            .write_video_clip_queue(items)
            .map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())
}

pub(crate) fn suggest(params: &Value) -> Result<Value, String> {
    let package =
        ProjectPackage::open(required_path(params, "project")?).map_err(|err| err.to_string())?;
    serde_json::to_value(
        package
            .suggest_video_clip_queue()
            .map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())
}

pub(crate) fn semantics_get(params: &Value) -> Result<Value, String> {
    let package =
        ProjectPackage::open(required_path(params, "project")?).map_err(|err| err.to_string())?;
    serde_json::to_value(
        package
            .video_clip_semantics()
            .map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())
}

pub(crate) fn semantics_analyze(params: &Value) -> Result<Value, String> {
    let package =
        ProjectPackage::open(required_path(params, "project")?).map_err(|err| err.to_string())?;
    serde_json::to_value(
        package
            .analyze_video_clip_semantics()
            .map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())
}

pub(crate) fn feedback_get(params: &Value) -> Result<Value, String> {
    let package =
        ProjectPackage::open(required_path(params, "project")?).map_err(|err| err.to_string())?;
    serde_json::to_value(
        package
            .video_clip_feedback()
            .map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())
}

pub(crate) fn feedback_set(params: &Value) -> Result<Value, String> {
    let package =
        ProjectPackage::open(required_path(params, "project")?).map_err(|err| err.to_string())?;
    let queue_item_id = required_string(params, "queue_item_id")
        .or_else(|_| required_string(params, "queueItemId"))?;
    let feedback =
        required_string(params, "feedback").or_else(|_| required_string(params, "text"))?;
    serde_json::to_value(
        package
            .record_video_clip_feedback(&queue_item_id, &feedback)
            .map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())
}

pub(crate) fn proposal_get(params: &Value) -> Result<Value, String> {
    let package =
        ProjectPackage::open(required_path(params, "project")?).map_err(|err| err.to_string())?;
    serde_json::to_value(
        package
            .video_clip_proposal()
            .map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())
}

pub(crate) fn proposal_generate(params: &Value) -> Result<Value, String> {
    let package =
        ProjectPackage::open(required_path(params, "project")?).map_err(|err| err.to_string())?;
    serde_json::to_value(
        package
            .generate_video_clip_proposal()
            .map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())
}

pub(crate) fn proposal_history_get(params: &Value) -> Result<Value, String> {
    let package =
        ProjectPackage::open(required_path(params, "project")?).map_err(|err| err.to_string())?;
    serde_json::to_value(
        package
            .video_clip_proposal_history()
            .map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())
}

pub(crate) fn proposal_decide(params: &Value) -> Result<Value, String> {
    let package =
        ProjectPackage::open(required_path(params, "project")?).map_err(|err| err.to_string())?;
    let proposal = required_string(params, "proposal")
        .or_else(|_| required_string(params, "proposal_id"))
        .or_else(|_| required_string(params, "proposalId"))?;
    let decision = required_string(params, "decision")?;
    let revision = optional_u64(params, "revision");
    let reason = params
        .get("reason")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    serde_json::to_value(
        package
            .decide_video_clip_proposal_for_revision(&proposal, revision, &decision, &reason)
            .map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())
}

fn required_path(params: &Value, key: &str) -> Result<PathBuf, String> {
    params
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| format!("missing required parameter: {key}"))
}

fn required_string(params: &Value, key: &str) -> Result<String, String> {
    params
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| format!("missing required parameter: {key}"))
}

fn optional_u64(params: &Value, key: &str) -> Option<u64> {
    params.get(key).and_then(|value| {
        value
            .as_u64()
            .or_else(|| value.as_str().and_then(|raw| raw.parse::<u64>().ok()))
    })
}
