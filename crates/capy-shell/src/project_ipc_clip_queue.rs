use std::path::PathBuf;

use capy_project::{ProjectPackage, ProjectVideoClipQueueItemV1};
use serde_json::Value;

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
