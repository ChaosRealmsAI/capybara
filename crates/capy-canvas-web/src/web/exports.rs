use std::sync::Arc;

use wasm_bindgen::prelude::*;

use winit::event_loop::EventLoop;
use winit::platform::web::EventLoopExtWebSys;

use capy_canvas_core::state::{CanvasContentKind, Tool};

use super::{WebApp, downloads, idb_store, log, redraw_via_shared, shared_state};

/// JS-callable entry. Boot the winit web event loop pointed at `canvas_id`.
#[wasm_bindgen]
pub fn start(canvas_id: String) {
    console_error_panic_hook::set_once();
    log(&format!("[capy-canvas-web] start canvas_id={canvas_id}"));

    let event_loop = match EventLoop::new() {
        Ok(el) => el,
        Err(error) => {
            log(&format!("[capy-canvas-web] EventLoop::new: {error}"));
            return;
        }
    };
    let app = WebApp::new(canvas_id);
    // EventLoopExtWebSys: spawn_app returns immediately on web; the loop
    // runs as JS microtasks driven by requestAnimationFrame & event handlers.
    event_loop.spawn_app(app);
}

/// JS-callable save. Skips the keyboard pending-flag plumbing and writes
/// straight to IndexedDB. Exists because Chrome/Firefox eat Cmd+S as
/// "Save Page As" before winit ever sees it; Playwright drives this path.
#[wasm_bindgen]
pub async fn save() -> Result<(), JsValue> {
    let state_arc = shared_state()
        .ok_or_else(|| JsValue::from_str("save(): no shared state · call start() first"))?;
    let json = {
        let state = state_arc
            .lock()
            .map_err(|_| JsValue::from_str("save(): state lock poisoned"))?;
        state
            .to_json_string()
            .map_err(|e| JsValue::from_str(&format!("save(): {e}")))?
    };
    idb_store::idb_save(json)
        .await
        .map_err(|e| JsValue::from_str(&e))?;
    log("[capy-canvas-web] save() ok");
    Ok(())
}

/// JS-callable SVG export. Generates the SVG from the current AppState
/// and triggers a browser download of `canvas.svg`. Bypasses the
/// keyboard-flag drain path for reliability under Playwright (some browsers
/// intercept Cmd+Shift+E for menu shortcuts before winit sees the keydown).
#[wasm_bindgen]
pub fn export_svg() -> Result<(), JsValue> {
    let state_arc = shared_state()
        .ok_or_else(|| JsValue::from_str("export_svg(): no shared state · call start() first"))?;
    let svg = {
        let state = state_arc
            .lock()
            .map_err(|_| JsValue::from_str("export_svg(): state lock poisoned"))?;
        state.export_svg()
    };
    downloads::trigger_download(svg.as_bytes(), "image/svg+xml", "canvas.svg")
        .map_err(|e| JsValue::from_str(&e))?;
    log("[capy-canvas-web] export_svg() ok");
    Ok(())
}

/// JS-callable PNG export. Renders current AppState to an offscreen RGBA
/// texture, reads back via `map_async`, encodes PNG, triggers download.
#[wasm_bindgen]
pub async fn export_png() -> Result<(), JsValue> {
    downloads::perform_png_export()
        .await
        .map_err(|e| JsValue::from_str(&e))?;
    log("[capy-canvas-web] export_png() ok");
    Ok(())
}

/// JS-callable load. Mirror image of `save()`.
#[wasm_bindgen]
pub async fn load() -> Result<bool, JsValue> {
    let state_arc = shared_state()
        .ok_or_else(|| JsValue::from_str("load(): no shared state · call start() first"))?;
    let json = idb_store::idb_load()
        .await
        .map_err(|e| JsValue::from_str(&e))?;
    let Some(json) = json else {
        log("[capy-canvas-web] load(): no snapshot");
        return Ok(false);
    };
    {
        let mut state = state_arc
            .lock()
            .map_err(|_| JsValue::from_str("load(): state lock poisoned"))?;
        state
            .load_from_json_str(&json)
            .map_err(|e| JsValue::from_str(&format!("load(): {e}")))?;
    }
    redraw_via_shared();
    log("[capy-canvas-web] load() ok");
    Ok(true)
}

/// JS-callable image insert. Decodes the byte slice (PNG/JPEG/WebP via the
/// `image` crate) and inserts a new `ShapeKind::Image` shape at `(x, y)`
/// with natural dimensions clamped to a reasonable on-screen size.
///
/// This is the stable contract the headless verify script drives.
/// Drag-drop in a real browser is too flaky to script reliably; we expose
/// the same byte-decode → state-insert path as a function call so the
/// pixel test can pump a known-good PNG into the canvas without simulating
/// `DragEvent`.
#[wasm_bindgen]
pub fn add_image_at(x: f64, y: f64, bytes: &[u8]) -> Result<u32, JsValue> {
    add_image_asset_at(x, y, bytes, "", "", "", "")
}

#[wasm_bindgen]
pub fn add_image_asset_at(
    x: f64,
    y: f64,
    bytes: &[u8],
    title: &str,
    source_path: &str,
    generation_provider: &str,
    generation_prompt: &str,
) -> Result<u32, JsValue> {
    let state_arc = shared_state().ok_or_else(|| {
        JsValue::from_str("add_image_asset_at(): no shared state · call start() first")
    })?;
    let decoded = image::load_from_memory(bytes)
        .map_err(|e| JsValue::from_str(&format!("decode image: {e}")))?
        .to_rgba8();
    let (w, h) = decoded.dimensions();
    let rgba = Arc::new(decoded.into_raw());
    // Sniff mime from header for round-trip metadata. The renderer doesn't
    // care — `peniko::ImageFormat::Rgba8` is set unconditionally — but
    // this lets a future save/load path encode back to the original codec.
    let mime = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png".to_string()
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "image/jpeg".to_string()
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        "image/webp".to_string()
    } else {
        "application/octet-stream".to_string()
    };
    let idx = {
        let mut state = state_arc
            .lock()
            .map_err(|_| JsValue::from_str("add_image_asset_at(): state lock poisoned"))?;
        let idx =
            state.import_image_asset_bytes(capy_canvas_core::state_shapes::ImageAssetImport {
                x,
                y,
                rgba,
                width: w,
                height: h,
                mime,
                title: optional_string(title),
                source_path: optional_string(source_path),
                generation_provider: optional_string(generation_provider),
                generation_prompt: optional_string(generation_prompt),
            });
        state.selected = vec![idx];
        state.tool = Tool::Select;
        idx
    };
    redraw_via_shared();
    log(&format!(
        "[capy-canvas-web] add_image_at({x}, {y}) ok · {w}x{h} idx={idx}"
    ));
    Ok(idx as u32)
}

/// JS-callable Lovart-style content card creation. This creates a real
/// canvas node with semantic metadata so the AI snapshot sees a product
/// object, not just pixels.
#[wasm_bindgen]
pub fn create_content_card(kind: &str, title: &str, x: f64, y: f64) -> Result<u32, JsValue> {
    let state_arc = shared_state().ok_or_else(|| {
        JsValue::from_str("create_content_card(): no shared state · call start() first")
    })?;
    let kind = kind
        .parse::<CanvasContentKind>()
        .map_err(|e| JsValue::from_str(&format!("create_content_card(): {e}")))?;
    let (idx, id) = {
        let mut state = state_arc
            .lock()
            .map_err(|_| JsValue::from_str("create_content_card(): state lock poisoned"))?;
        let idx = state.create_content_card(kind, title, x, y);
        state.tool = Tool::Select;
        (idx, state.shapes[idx].id)
    };
    redraw_via_shared();
    log(&format!(
        "[capy-canvas-web] create_content_card({kind:?}, id={id}, idx={idx}) ok"
    ));
    Ok(idx as u32)
}

#[wasm_bindgen]
pub fn create_poster_document_card(
    title: &str,
    x: f64,
    y: f64,
    source_path: &str,
) -> Result<u32, JsValue> {
    let state_arc = shared_state().ok_or_else(|| {
        JsValue::from_str("create_poster_document_card(): no shared state · call start() first")
    })?;
    let (idx, id) = {
        let mut state = state_arc
            .lock()
            .map_err(|_| JsValue::from_str("create_poster_document_card(): state lock poisoned"))?;
        let idx = state.create_poster_document_card(title, x, y, source_path);
        state.tool = Tool::Select;
        (idx, state.shapes[idx].id)
    };
    redraw_via_shared();
    log(&format!(
        "[capy-canvas-web] create_poster_document_card(id={id}, idx={idx}) ok"
    ));
    Ok(idx as u32)
}

/// JS-callable selection bridge for DOM labels and desktop verification.
#[wasm_bindgen]
pub fn select_node(id: u32) -> Result<bool, JsValue> {
    let state_arc = shared_state()
        .ok_or_else(|| JsValue::from_str("select_node(): no shared state · call start() first"))?;
    let found = {
        let mut state = state_arc
            .lock()
            .map_err(|_| JsValue::from_str("select_node(): state lock poisoned"))?;
        state.select_shape_ids(&[u64::from(id)]).is_ok()
    };
    if found {
        redraw_via_shared();
        log(&format!("[capy-canvas-web] select_node(id={id}) ok"));
    }
    Ok(found)
}

#[wasm_bindgen]
pub fn focus_node(id: u32) -> Result<bool, JsValue> {
    let state_arc = shared_state()
        .ok_or_else(|| JsValue::from_str("focus_node(): no shared state · call start() first"))?;
    let mut state = state_arc
        .lock()
        .map_err(|_| JsValue::from_str("focus_node(): state lock poisoned"))?;
    let Some(idx) = state
        .shapes
        .iter()
        .position(|shape| shape.id == u64::from(id))
    else {
        return Ok(false);
    };
    let shape = &state.shapes[idx];
    let cx = shape.x + shape.w.min(0.0) + shape.w.abs() / 2.0;
    let cy = shape.y + shape.h.min(0.0) + shape.h.abs() / 2.0;
    let zoom = state.camera.zoom;
    state.selected = vec![idx];
    state.tool = Tool::Select;
    state.camera.offset_x = state.viewport_w / 2.0 - cx * zoom;
    state.camera.offset_y = state.viewport_h / 2.0 - cy * zoom;
    state.target_zoom = zoom;
    drop(state);
    redraw_via_shared();
    Ok(true)
}

/// JS-callable absolute move bridge for AI actions and desktop verification.
#[wasm_bindgen]
pub fn move_node_by_id(id: u32, x: f64, y: f64) -> Result<bool, JsValue> {
    let state_arc = shared_state().ok_or_else(|| {
        JsValue::from_str("move_node_by_id(): no shared state · call start() first")
    })?;
    let moved = {
        let mut state = state_arc
            .lock()
            .map_err(|_| JsValue::from_str("move_node_by_id(): state lock poisoned"))?;
        state.move_shape_by_id(u64::from(id), x, y).is_ok()
    };
    if moved {
        redraw_via_shared();
        log(&format!(
            "[capy-canvas-web] move_node_by_id(id={id}, x={x:.1}, y={y:.1}) ok"
        ));
    }
    Ok(moved)
}

// v0.8 introspection exports used by desktop state-key scripts.

/// Number of shapes currently on the canvas. Returns 0 before `start()`
/// finishes wiring up `SHARED_STATE`, so the desktop state-key script can
/// fall back to "0" without throwing.
#[wasm_bindgen]
pub fn shape_count() -> usize {
    shared_state()
        .and_then(|arc| arc.lock().ok().map(|s| s.shapes.len()))
        .unwrap_or(0)
}

/// Snake_case label of the active tool (e.g. "rect", "select"). Returns
/// "select" as the default if state isn't ready yet — matches the initial
/// `Tool::Select` constructor in `AppState::new()`.
#[wasm_bindgen]
pub fn current_tool() -> String {
    shared_state()
        .and_then(|arc| arc.lock().ok().map(|s| tool_label(s.tool).to_string()))
        .unwrap_or_else(|| "select".to_string())
}

/// Whether dark mode is active in AppState. Returns false if state isn't
/// ready (matches `AppState::new()` default).
#[wasm_bindgen]
pub fn dark_mode() -> bool {
    shared_state()
        .and_then(|arc| arc.lock().ok().map(|s| s.dark_mode))
        .unwrap_or(false)
}

/// Serialize the full shape list as a JS array. The desktop shell wraps
/// this in `JSON.stringify(...)` for transport over IPC.
#[wasm_bindgen]
pub fn list_shapes() -> JsValue {
    if let Some(arc) = shared_state() {
        if let Ok(state) = arc.lock() {
            return serde_wasm_bindgen::to_value(&state.shapes).unwrap_or(JsValue::NULL);
        }
    }
    JsValue::NULL
}

/// Product context for the current selection. Planner chat uses this to
/// receive structured canvas context without scraping pixels.
#[wasm_bindgen]
pub fn selected_context() -> JsValue {
    if let Some(arc) = shared_state() {
        if let Ok(state) = arc.lock() {
            return serde_wasm_bindgen::to_value(&state.selected_context())
                .unwrap_or(JsValue::NULL);
        }
    }
    JsValue::NULL
}

/// Human-readable selection summary for prompt injection.
#[wasm_bindgen]
pub fn selected_context_text() -> String {
    shared_state()
        .and_then(|arc| arc.lock().ok().map(|state| state.selected_context_text()))
        .unwrap_or_default()
}

/// Full AI-facing canvas snapshot: layout, nodes, connectors, groups,
/// selection, and stable id-based action names.
#[wasm_bindgen]
pub fn ai_snapshot() -> JsValue {
    if let Some(arc) = shared_state() {
        if let Ok(state) = arc.lock() {
            return serde_wasm_bindgen::to_value(&state.ai_snapshot()).unwrap_or(JsValue::NULL);
        }
    }
    JsValue::NULL
}

/// Human-readable whole-canvas summary for agent prompts or CLI inspection.
#[wasm_bindgen]
pub fn ai_snapshot_text() -> String {
    shared_state()
        .and_then(|arc| arc.lock().ok().map(|state| state.ai_snapshot_text()))
        .unwrap_or_default()
}

fn tool_label(tool: Tool) -> &'static str {
    match tool {
        Tool::Select => "select",
        Tool::Rect => "rect",
        Tool::Ellipse => "ellipse",
        Tool::Triangle => "triangle",
        Tool::Diamond => "diamond",
        Tool::Line => "line",
        Tool::Arrow => "arrow",
        Tool::Freehand => "freehand",
        Tool::Highlighter => "highlighter",
        Tool::StickyNote => "sticky_note",
        Tool::Text => "text",
        Tool::Eraser => "eraser",
        Tool::Lasso => "lasso",
    }
}

fn optional_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
