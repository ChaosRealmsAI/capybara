use wasm_bindgen::prelude::*;

use capy_canvas_core::state::Tool;

use super::{log, redraw_via_shared, shared_state};

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn create_project_artifact_card(
    title: &str,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    project_id: &str,
    surface_node_id: &str,
    artifact_id: &str,
    artifact_kind: &str,
    source_path: &str,
) -> Result<u32, JsValue> {
    let state_arc = shared_state().ok_or_else(|| {
        JsValue::from_str("create_project_artifact_card(): no shared state · call start() first")
    })?;
    let (idx, id) = {
        let mut state = state_arc.lock().map_err(|_| {
            JsValue::from_str("create_project_artifact_card(): state lock poisoned")
        })?;
        let idx = state.create_project_artifact_card(
            title,
            x,
            y,
            w,
            h,
            project_id,
            surface_node_id,
            artifact_id,
            artifact_kind,
            source_path,
        );
        state.tool = Tool::Select;
        (idx, state.shapes[idx].id)
    };
    redraw_via_shared();
    log(&format!(
        "[capy-canvas-web] create_project_artifact_card(id={id}, idx={idx}) ok"
    ));
    Ok(idx as u32)
}

#[wasm_bindgen]
pub fn resize_node_by_id(id: u32, x: f64, y: f64, w: f64, h: f64) -> Result<bool, JsValue> {
    let state_arc = shared_state().ok_or_else(|| {
        JsValue::from_str("resize_node_by_id(): no shared state · call start() first")
    })?;
    let resized = {
        let mut state = state_arc
            .lock()
            .map_err(|_| JsValue::from_str("resize_node_by_id(): state lock poisoned"))?;
        state.resize_shape_by_id(u64::from(id), x, y, w, h).is_ok()
    };
    if resized {
        redraw_via_shared();
        log(&format!(
            "[capy-canvas-web] resize_node_by_id(id={id}, x={x:.1}, y={y:.1}, w={w:.1}, h={h:.1}) ok"
        ));
    }
    Ok(resized)
}
