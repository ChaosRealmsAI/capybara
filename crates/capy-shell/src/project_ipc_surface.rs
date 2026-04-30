use std::path::PathBuf;

use capy_project::{ProjectPackage, SurfaceGeometryV1};
use serde_json::Value;

pub(crate) fn nodes(params: &Value) -> Result<Value, String> {
    let package =
        ProjectPackage::open(required_path(params, "project")?).map_err(|err| err.to_string())?;
    serde_json::to_value(
        package
            .ensure_surface_nodes()
            .map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())
}

pub(crate) fn update(params: &Value) -> Result<Value, String> {
    let package =
        ProjectPackage::open(required_path(params, "project")?).map_err(|err| err.to_string())?;
    let node_id = optional_string(params, "node_id")
        .or_else(|| optional_string(params, "nodeId"))
        .ok_or_else(|| "missing required parameter: node_id".to_string())?;
    let geometry = params
        .get("geometry")
        .cloned()
        .ok_or_else(|| "missing required parameter: geometry".to_string())
        .and_then(|value| {
            serde_json::from_value::<SurfaceGeometryV1>(value)
                .map_err(|err| format!("parse geometry failed: {err}"))
        })?;
    serde_json::to_value(
        package
            .update_surface_node_geometry(&node_id, geometry)
            .map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())
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
