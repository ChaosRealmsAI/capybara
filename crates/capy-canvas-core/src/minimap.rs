//! Minimap: bottom-right corner overview of all shapes + viewport.

use crate::state::AppState;

/// Minimap dimensions.
pub const MINIMAP_W: f64 = 160.0;
pub const MINIMAP_H: f64 = 100.0;
pub const MINIMAP_MARGIN: f64 = 16.0;
pub const STATUS_H: f64 = 28.0;
pub const MINIMAP_PAD: f64 = 10.0;

/// Returns (mx, my) screen position of minimap top-left.
pub fn minimap_origin(state: &AppState) -> (f64, f64) {
    let scale = crate::ui::overlay_scale(state.viewport_w, state.viewport_h);
    let mx = state.viewport_w - (MINIMAP_MARGIN + MINIMAP_W) * scale;
    let my = state.viewport_h - (MINIMAP_MARGIN + STATUS_H + MINIMAP_H) * scale;
    (mx, my)
}

/// Compute the bounding box of all shapes for the minimap.
/// Returns ((x, y, w, h), has_shapes).
pub fn compute_bounds(state: &AppState) -> ((f64, f64, f64, f64), bool) {
    if state.shapes.is_empty() {
        return ((0.0, 0.0, 1000.0, 700.0), false);
    }
    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;
    for s in &state.shapes {
        min_x = min_x.min(s.x);
        min_y = min_y.min(s.y);
        max_x = max_x.max(s.x + s.w);
        max_y = max_y.max(s.y + s.h);
    }
    // Include viewport range
    let cam = &state.camera;
    let vw_world = state.viewport_w / cam.zoom;
    let vh_world = state.viewport_h / cam.zoom;
    let cam_x = -cam.offset_x / cam.zoom;
    let cam_y = -cam.offset_y / cam.zoom;
    min_x = min_x.min(cam_x);
    min_y = min_y.min(cam_y);
    max_x = max_x.max(cam_x + vw_world);
    max_y = max_y.max(cam_y + vh_world);

    let pad_w = (max_x - min_x) * 0.1;
    let pad_h = (max_y - min_y) * 0.1;
    (
        (
            min_x - pad_w,
            min_y - pad_h,
            max_x - min_x + pad_w * 2.0,
            max_y - min_y + pad_h * 2.0,
        ),
        true,
    )
}

/// Returns world coordinates if click is inside the minimap area.
pub fn hit_test(state: &AppState, sx: f64, sy: f64) -> Option<(f64, f64)> {
    let ui_scale = crate::ui::overlay_scale(state.viewport_w, state.viewport_h);
    let sx = sx / ui_scale;
    let sy = sy / ui_scale;
    let mx = state.viewport_w / ui_scale - MINIMAP_MARGIN - MINIMAP_W;
    let my = state.viewport_h / ui_scale - MINIMAP_MARGIN - STATUS_H - MINIMAP_H;

    if sx < mx || sx > mx + MINIMAP_W || sy < my || sy > my + MINIMAP_H {
        return None;
    }

    let (world_bounds, _) = compute_bounds(state);
    let (bx, by, bw, bh) = world_bounds;
    if bw < 1.0 || bh < 1.0 {
        return None;
    }

    let draw_w = MINIMAP_W - MINIMAP_PAD * 2.0;
    let draw_h = MINIMAP_H - MINIMAP_PAD * 2.0;
    let scale = (draw_w / bw).min(draw_h / bh);
    let off_x = (draw_w - bw * scale) / 2.0;
    let off_y = (draw_h - bh * scale) / 2.0;

    let local_x = sx - mx - MINIMAP_PAD - off_x;
    let local_y = sy - my - MINIMAP_PAD - off_y;
    let world_x = bx + local_x / scale;
    let world_y = by + local_y / scale;
    Some((world_x, world_y))
}
