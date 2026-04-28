//! Vello scene builder: orchestrates canvas background, shapes, connectors,
//! overlays, and UI drawing.
//!
//! Heavy lifting lives in sibling modules:
//! - `render_shapes`: individual shape rendering
//! - `render_lines`: arrows/connectors/shared line primitives
//! - `render_ui`: grid, selection handles, guides
//! - `render_overlay`: preview, lasso, binding indicator, group bounds, etc.

use vello::Scene;
use vello::kurbo::{Affine, Rect};
use vello::peniko::{Color, Fill};

use crate::state::{AppState, DragMode, Tool};
use crate::ui;

mod private {
    //! Re-export internal constants so sibling crate modules can use them
    //! via `crate::render::CONST_NAME`.
    use vello::peniko::Color;

    pub const CANVAS_BG: Color = Color::from_rgba8(0xf8, 0xf7, 0xf4, 0xff);
    pub const CANVAS_BG_DARK: Color = Color::from_rgba8(0x1a, 0x1a, 0x22, 0xff);
    pub const SELECTION_COLOR: Color = Color::from_rgba8(0x8a, 0x6f, 0xae, 0xff);
    pub const GRID_STEP: f64 = 20.0;
    pub const HANDLE_SIZE: f64 = 8.0;
    pub const SHAPE_FILL_ALPHA: f32 = 0.15;
    pub const SHAPE_FILL_SELECTED_ALPHA: f32 = 0.22;
}

// Re-export constants for sibling modules
pub(crate) use private::{
    GRID_STEP, HANDLE_SIZE, SELECTION_COLOR, SHAPE_FILL_ALPHA, SHAPE_FILL_SELECTED_ALPHA,
};

use private::{CANVAS_BG, CANVAS_BG_DARK};

// Import sibling render modules
use crate::render_overlay;
use crate::render_shapes;
use crate::render_ui;

/// Build the full scene.
pub fn build_scene(state: &AppState) -> Scene {
    let mut scene = Scene::new();
    let vw = state.viewport_w;
    let vh = state.viewport_h;
    let cam = &state.camera;
    let camera_tf = Affine::translate((cam.offset_x, cam.offset_y)) * Affine::scale(cam.zoom);

    // Canvas background
    let bg_color = if state.dark_mode {
        CANVAS_BG_DARK
    } else {
        CANVAS_BG
    };
    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        bg_color,
        None,
        &Rect::new(0.0, 0.0, vw, vh),
    );

    // Grid dots
    render_ui::draw_grid(&mut scene, cam, vw, vh, state.dark_mode);

    // Alignment guides
    if matches!(state.drag_mode, DragMode::Moving { .. }) && !state.selected.is_empty() {
        let guides = state.alignment_guides(&state.selected);
        render_ui::draw_guides(&mut scene, &guides, cam, vw, vh);
    }

    // Connectors
    for (ci, conn) in state.connectors.iter().enumerate() {
        crate::render_lines::draw_connector_indexed(&mut scene, state, conn, ci, camera_tf);
    }

    // Shapes
    for (i, shape) in state.shapes.iter().enumerate() {
        let is_selected = state.selected.contains(&i);
        let is_hovered = state.hovered_shape == Some(i);
        render_shapes::draw_shape(
            &mut scene,
            state,
            shape,
            i,
            camera_tf,
            is_selected,
            is_hovered,
        );
        if is_selected {
            let sel_alpha = selection_handle_alpha(state);
            if matches!(
                shape.kind,
                crate::state::ShapeKind::Line | crate::state::ShapeKind::Arrow
            ) {
                crate::render_line_ui::draw_shape_selection(
                    &mut scene, state, shape, camera_tf, cam.zoom, sel_alpha,
                );
            } else {
                render_ui::draw_selection(&mut scene, shape, camera_tf, cam.zoom, sel_alpha);
            }
        }
    }

    // Group bounding boxes
    render_overlay::draw_group_bounds(&mut scene, state, camera_tf);

    // Shape creation preview
    if state.drag_mode == DragMode::Creating {
        render_overlay::draw_creation_preview(&mut scene, state, camera_tf);
    }

    // Rubber-band selection rectangle
    render_overlay::draw_rubber_band(&mut scene, state, camera_tf);

    // Lasso selection path
    render_overlay::draw_lasso(&mut scene, state, camera_tf);

    // Arrow/Line tool: draw anchor points on nearby shapes for binding preview
    if matches!(state.tool, Tool::Arrow | Tool::Line) {
        let (mouse_wx, mouse_wy) = state.camera.screen_to_world(state.cursor_x, state.cursor_y);
        let mut drew_anchors = false;
        // Find all shapes within 30px of the mouse cursor and draw anchors
        for shape in &state.shapes {
            let near_x = mouse_wx >= shape.x - 30.0 && mouse_wx <= shape.x + shape.w + 30.0;
            let near_y = mouse_wy >= shape.y - 30.0 && mouse_wy <= shape.y + shape.h + 30.0;
            if near_x && near_y && (shape.w > 1.0 || shape.h > 1.0) {
                render_overlay::draw_connector_anchors(
                    &mut scene, shape, mouse_wx, mouse_wy, camera_tf,
                );
                drew_anchors = true;
                // No break: show anchors on all nearby shapes during drag
            }
        }
        if !drew_anchors {
            if let Some((bx, by)) = state.binding_indicator {
                render_overlay::draw_binding_indicator(&mut scene, bx, by, camera_tf);
            }
        }
    }

    // Bound arrow drag preview line
    if state.connector_preview.is_some() {
        render_overlay::draw_connector_preview(&mut scene, state, camera_tf);
    }

    // Eraser cursor
    if state.tool == Tool::Eraser {
        render_overlay::draw_eraser_cursor(&mut scene, state);
    }

    // Rotation angle tooltip
    if state.drag_mode == DragMode::Rotating {
        if let Some(&idx) = state.selected.first() {
            if idx < state.shapes.len() {
                render_overlay::draw_rotation_tooltip(&mut scene, state, &state.shapes[idx]);
            }
        }
    }

    // UI overlays (screen space)
    // v0.12 · suppress legacy WASM chrome (toolbar / minimap / style_panel / status_bar)
    // because the HTML shell now provides Arc-glass replacements.
    // ui::draw_toolbar(&mut scene, state);
    // ui::draw_style_panel(&mut scene, state);
    // ui::draw_minimap(&mut scene, state);
    ui::draw_context_menu(&mut scene, state);
    ui::draw_tooltip(&mut scene, state);
    // ui::draw_status_bar(&mut scene, state);
    ui::draw_toasts(&mut scene, state);
    ui::draw_help_overlay(&mut scene, state);

    scene
}

pub fn color_from_hex(hex: u32, alpha: f32) -> Color {
    Color::from_rgba8(
        (hex >> 16) as u8,
        ((hex >> 8) & 0xff) as u8,
        (hex & 0xff) as u8,
        (alpha * 255.0) as u8,
    )
}

/// Compute handle alpha for selection fade-in animation (100ms).
fn selection_handle_alpha(state: &AppState) -> f32 {
    let elapsed = state.selection_time.elapsed().as_millis() as f32;
    (elapsed / 100.0).min(1.0)
}
