use wasm_bindgen::prelude::*;

use super::{redraw_via_shared, shared_state};

/// Zoom the canvas around a screen-space point. Wheel input uses the same
/// camera math; this bridge powers visible controls and AI probes.
#[wasm_bindgen]
pub fn zoom_view_at(screen_x: f64, screen_y: f64, factor: f64) -> Result<f64, JsValue> {
    let state_arc = shared_state()
        .ok_or_else(|| JsValue::from_str("zoom_view_at(): no shared state · call start() first"))?;
    let zoom = {
        let mut state = state_arc
            .lock()
            .map_err(|_| JsValue::from_str("zoom_view_at(): state lock poisoned"))?;
        if !screen_x.is_finite() || !screen_y.is_finite() || !factor.is_finite() || factor <= 0.0 {
            return Err(JsValue::from_str(
                "zoom_view_at(): invalid coordinates or factor",
            ));
        }
        state.camera.zoom_at(screen_x, screen_y, factor);
        state.target_zoom = state.camera.zoom;
        state.camera.zoom
    };
    redraw_via_shared();
    Ok(zoom)
}

/// Pan the canvas by a screen-space delta.
#[wasm_bindgen]
pub fn pan_view_by(dx: f64, dy: f64) -> Result<bool, JsValue> {
    let state_arc = shared_state()
        .ok_or_else(|| JsValue::from_str("pan_view_by(): no shared state · call start() first"))?;
    {
        let mut state = state_arc
            .lock()
            .map_err(|_| JsValue::from_str("pan_view_by(): state lock poisoned"))?;
        if !dx.is_finite() || !dy.is_finite() {
            return Ok(false);
        }
        state.camera.pan(dx, dy);
    }
    redraw_via_shared();
    Ok(true)
}

/// Reset viewport to 100% at the origin.
#[wasm_bindgen]
pub fn reset_view() -> Result<bool, JsValue> {
    let state_arc = shared_state()
        .ok_or_else(|| JsValue::from_str("reset_view(): no shared state · call start() first"))?;
    {
        let mut state = state_arc
            .lock()
            .map_err(|_| JsValue::from_str("reset_view(): state lock poisoned"))?;
        state.camera.zoom = 1.0;
        state.target_zoom = 1.0;
        state.camera.offset_x = 0.0;
        state.camera.offset_y = 0.0;
    }
    redraw_via_shared();
    Ok(true)
}

/// Fit all canvas objects in the current viewport; empty canvases reset.
#[wasm_bindgen]
pub fn fit_view_to_content() -> Result<bool, JsValue> {
    let state_arc = shared_state().ok_or_else(|| {
        JsValue::from_str("fit_view_to_content(): no shared state · call start() first")
    })?;
    {
        let mut state = state_arc
            .lock()
            .map_err(|_| JsValue::from_str("fit_view_to_content(): state lock poisoned"))?;
        if state.shapes.is_empty() {
            state.camera.zoom = 1.0;
            state.target_zoom = 1.0;
            state.camera.offset_x = 0.0;
            state.camera.offset_y = 0.0;
        } else {
            state.zoom_fit();
        }
    }
    redraw_via_shared();
    Ok(true)
}
