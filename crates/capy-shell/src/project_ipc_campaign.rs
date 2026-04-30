use std::path::PathBuf;

use capy_project::{ProjectCampaignRequestV1, ProjectPackage};
use serde_json::Value;

pub(crate) fn campaign_plan(params: &Value) -> Result<Value, String> {
    let package =
        ProjectPackage::open(required_path(params, "project")?).map_err(|err| err.to_string())?;
    serde_json::to_value(
        package
            .campaign_plan(request(params)?)
            .map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())
}

pub(crate) fn campaign_generate(params: &Value) -> Result<Value, String> {
    let package =
        ProjectPackage::open(required_path(params, "project")?).map_err(|err| err.to_string())?;
    serde_json::to_value(
        package
            .campaign_generate(request(params)?)
            .map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())
}

pub(crate) fn campaign_show(params: &Value) -> Result<Value, String> {
    let package =
        ProjectPackage::open(required_path(params, "project")?).map_err(|err| err.to_string())?;
    serde_json::to_value(
        package
            .campaign_show(&required_string(params, "run_id")?)
            .map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())
}

fn request(params: &Value) -> Result<ProjectCampaignRequestV1, String> {
    Ok(ProjectCampaignRequestV1 {
        brief: required_string(params, "brief")?,
        artifact_ids: params
            .get("artifacts")
            .or_else(|| params.get("artifact_ids"))
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(ToString::to_string)
                    .collect()
            })
            .unwrap_or_default(),
    })
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
