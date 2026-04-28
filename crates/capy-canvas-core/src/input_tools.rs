//! Tool-specific input handlers: select, draw, text, eraser, connector,
//! sticky note, lasso, resize, edge scrolling, cursor style.

use crate::state::{AppState, CursorStyle, DragMode, Shape, ShapeKind, Tool};
use crate::text_edit::begin_text_edit;
use crate::ui;

const GRID_SNAP: f64 = 20.0;

pub(crate) fn snap_to_grid(v: f64) -> f64 {
    (v / GRID_SNAP).round() * GRID_SNAP
}

pub(crate) fn handle_select_down(state: &mut AppState, wx: f64, wy: f64, shift: bool) -> bool {
    if state.selected.len() == 1 {
        let idx = state.selected[0];
        if idx < state.shapes.len() {
            if crate::line_edit::begin_handle_drag(state, wx, wy) {
                return true;
            }
            if hit_rotation_handle(&state.shapes[idx], wx, wy, state.camera.zoom) {
                state.drag_mode = DragMode::Rotating;
                state.push_undo();
                return true;
            }
            if let Some(handle) = hit_handle(&state.shapes[idx], wx, wy, state.camera.zoom) {
                state.drag_mode = DragMode::Resizing { handle };
                state.push_undo();
                state.drag_shape_origins = vec![(state.shapes[idx].x, state.shapes[idx].y)];
                return true;
            }
        }
    }

    if let Some(idx) = state.hit_test(wx, wy) {
        state.selected_connector = None;
        if shift {
            if let Some(pos) = state.selected.iter().position(|&i| i == idx) {
                state.selected.remove(pos);
            } else {
                state.selected.push(idx);
            }
        } else if !state.selected.contains(&idx) {
            state.selected = vec![idx];
        }
        state.expand_selection_to_groups();
        state.selection_time = web_time::Instant::now();
        state.drag_shape_origins = state
            .selected
            .iter()
            .map(|&i| (state.shapes[i].x, state.shapes[i].y))
            .collect();
        state.drag_mode = DragMode::Moving {
            start_wx: wx,
            start_wy: wy,
        };
        state.push_undo();
        true
    } else {
        // No shape hit — check connectors
        let threshold = 8.0;
        let mut hit_conn = None;
        for (i, conn) in state.connectors.iter().enumerate() {
            if crate::render_lines::connector_hit_test(state, conn, wx, wy, threshold) {
                hit_conn = Some(i);
                break;
            }
        }
        if let Some(conn_idx) = hit_conn {
            state.selected.clear();
            state.selected_connector = Some(conn_idx);
            return true;
        }

        state.selected_connector = None;
        if !shift {
            state.selected.clear();
        }
        state.drag_mode = DragMode::RubberBand {
            start_wx: wx,
            start_wy: wy,
        };
        true
    }
}

/// Test if a world point hits one of the 8 resize handles.
pub(crate) fn hit_handle(shape: &Shape, wx: f64, wy: f64, zoom: f64) -> Option<usize> {
    let hs = 8.0 / zoom;
    let handles = [
        (shape.x, shape.y),
        (shape.x + shape.w, shape.y),
        (shape.x, shape.y + shape.h),
        (shape.x + shape.w, shape.y + shape.h),
        (shape.x + shape.w / 2.0, shape.y),
        (shape.x + shape.w / 2.0, shape.y + shape.h),
        (shape.x, shape.y + shape.h / 2.0),
        (shape.x + shape.w, shape.y + shape.h / 2.0),
    ];
    for (i, (hx, hy)) in handles.iter().enumerate() {
        if (wx - hx).abs() <= hs && (wy - hy).abs() <= hs {
            return Some(i);
        }
    }
    None
}

/// Test if a world point hits the rotation handle.
pub(crate) fn hit_rotation_handle(shape: &Shape, wx: f64, wy: f64, zoom: f64) -> bool {
    let hs = 10.0 / zoom;
    let cx = shape.x + shape.w / 2.0;
    let cy = shape.y - 20.0 / zoom;
    let dx = wx - cx;
    let dy = wy - cy;
    dx * dx + dy * dy <= hs * hs
}

pub(crate) fn handle_eraser(state: &mut AppState, wx: f64, wy: f64) -> bool {
    state.drag_mode = DragMode::Erasing;
    if let Some(idx) = state.hit_test(wx, wy) {
        state.push_undo();
        let removed_id = state.shapes[idx].id;
        state.shapes.remove(idx);
        crate::connector::remove_connectors_for_shape(state, removed_id);
        crate::connector::clear_bindings_for_shape(state, removed_id);
        state.selected.clear();
        true
    } else {
        true
    }
}

/// Arrow/Line tool: if mouse down is on or near a shape, start a bound arrow
/// drag (like Excalidraw). Otherwise, create a normal free-floating arrow/line.
pub(crate) fn handle_arrow_or_line_down(state: &mut AppState, wx: f64, wy: f64) -> bool {
    let near_shape = state.shapes.iter().enumerate().find(|(_, s)| {
        let near_x = wx >= s.x - 30.0 && wx <= s.x + s.w + 30.0;
        let near_y = wy >= s.y - 30.0 && wy <= s.y + s.h + 30.0;
        near_x && near_y && (s.w > 1.0 || s.h > 1.0)
    });

    if let Some((idx, _)) = near_shape {
        let shape = &state.shapes[idx];
        let from_id = shape.id;
        let anchors = shape.anchor_points();
        let best = anchors
            .iter()
            .copied()
            .min_by(|a, b| {
                let da = (wx - a.0).powi(2) + (wy - a.1).powi(2);
                let db = (wx - b.0).powi(2) + (wy - b.1).powi(2);
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap_or((wx, wy));
        state.connector_from = Some(from_id);
        state.connector_preview = Some((best.0, best.1, wx, wy));
        state.drag_mode = DragMode::ConnectorDrag { from_id };
        true
    } else {
        handle_draw_start(state, wx, wy)
    }
}

pub(crate) fn handle_text_click(state: &mut AppState, wx: f64, wy: f64) -> bool {
    state.push_undo();
    let snapped_x = if state.alt_held { wx } else { snap_to_grid(wx) };
    let snapped_y = if state.alt_held { wy } else { snap_to_grid(wy) };
    let mut shape = Shape::new(
        ShapeKind::Text,
        snapped_x,
        snapped_y,
        Shape::default_color_for_kind(ShapeKind::Text),
    );
    shape.w = 120.0;
    shape.h = 30.0;
    apply_style_from_state(state, &mut shape);
    let idx = state.add_shape(shape);
    begin_text_edit(state, idx);
    true
}

pub(crate) fn handle_draw_start(state: &mut AppState, wx: f64, wy: f64) -> bool {
    state.push_undo();
    let kind = match state.tool {
        Tool::Rect => ShapeKind::Rect,
        Tool::Ellipse => ShapeKind::Ellipse,
        Tool::Triangle => ShapeKind::Triangle,
        Tool::Diamond => ShapeKind::Diamond,
        Tool::Line => ShapeKind::Line,
        Tool::Arrow => ShapeKind::Arrow,
        Tool::Freehand => ShapeKind::Freehand,
        Tool::Highlighter => ShapeKind::Highlighter,
        _ => return false,
    };
    let is_freeform = matches!(kind, ShapeKind::Freehand | ShapeKind::Highlighter);
    let (start_x, start_y) = if is_freeform || state.alt_held {
        (wx, wy)
    } else {
        (snap_to_grid(wx), snap_to_grid(wy))
    };
    let mut shape = Shape::new(kind, start_x, start_y, Shape::default_color_for_kind(kind));
    if is_freeform {
        shape.points.push((wx, wy));
    }
    apply_style_from_state(state, &mut shape);
    let idx = state.add_shape(shape);
    state.selected = vec![idx];
    state.drag_shape_origins = vec![(start_x, start_y)];
    state.drag_mode = DragMode::Creating;
    true
}

pub(crate) fn handle_sticky_click(state: &mut AppState, wx: f64, wy: f64) -> bool {
    state.push_undo();
    let snapped_x = if state.alt_held { wx } else { snap_to_grid(wx) };
    let snapped_y = if state.alt_held { wy } else { snap_to_grid(wy) };
    let mut shape = Shape::new(
        ShapeKind::StickyNote,
        snapped_x,
        snapped_y,
        Shape::default_color_for_kind(ShapeKind::StickyNote),
    );
    shape.w = 200.0;
    shape.h = 200.0;
    apply_style_from_state(state, &mut shape);
    let idx = state.add_shape(shape);
    begin_text_edit(state, idx);
    true
}

pub(crate) fn handle_lasso_start(state: &mut AppState, wx: f64, wy: f64) -> bool {
    state.lasso_points.clear();
    state.lasso_points.push((wx, wy));
    state.drag_mode = DragMode::Lasso;
    true
}

/// Apply current AppState style properties to a new shape.
pub(crate) fn apply_style_from_state(state: &AppState, shape: &mut Shape) {
    shape.stroke_color = state.color;
    shape.color = state.fill_color;
    shape.stroke_width = state.stroke_width;
    shape.stroke_style = state.stroke_style;
    shape.fill_style = state.fill_style;
    shape.rounded = state.rounded;
    shape.opacity = state.opacity;
    shape.font_family = state.current_font_family;
    shape.font_size = state.current_font_size;
    shape.text_align = state.current_text_align;
    shape.bold = state.current_bold;
    shape.italic = state.current_italic;
}

/// Resize a shape by moving one of its 8 handles to a new world position.
pub(crate) fn resize_shape(shape: &mut Shape, handle: usize, wx: f64, wy: f64) {
    let (mut x1, mut y1) = (shape.x, shape.y);
    let (mut x2, mut y2) = (shape.x + shape.w, shape.y + shape.h);

    match handle {
        0 => {
            x1 = wx;
            y1 = wy;
        }
        1 => {
            x2 = wx;
            y1 = wy;
        }
        2 => {
            x1 = wx;
            y2 = wy;
        }
        3 => {
            x2 = wx;
            y2 = wy;
        }
        4 => {
            y1 = wy;
        }
        5 => {
            y2 = wy;
        }
        6 => {
            x1 = wx;
        }
        7 => {
            x2 = wx;
        }
        _ => {}
    }

    if x1 > x2 {
        std::mem::swap(&mut x1, &mut x2);
    }
    if y1 > y2 {
        std::mem::swap(&mut y1, &mut y2);
    }

    shape.x = x1;
    shape.y = y1;
    shape.w = (x2 - x1).max(1.0);
    shape.h = (y2 - y1).max(1.0);
}

/// Compute the appropriate cursor style based on current tool and context.
pub(crate) fn compute_cursor_style(state: &AppState, wx: f64, wy: f64) -> CursorStyle {
    if state.space_held {
        return CursorStyle::Grab;
    }

    match state.tool {
        Tool::Select => {
            if state.selected.len() == 1 {
                let idx = state.selected[0];
                if idx < state.shapes.len() {
                    if let Some(cursor) =
                        crate::line_edit::cursor_for_selected_handle(state, idx, wx, wy)
                    {
                        return cursor;
                    }
                    if hit_rotation_handle(&state.shapes[idx], wx, wy, state.camera.zoom) {
                        return CursorStyle::Grabbing;
                    }
                    if let Some(handle) = hit_handle(&state.shapes[idx], wx, wy, state.camera.zoom)
                    {
                        return handle_cursor(handle);
                    }
                }
            }
            if state.hit_test(wx, wy).is_some() {
                CursorStyle::Pointer
            } else {
                CursorStyle::Default
            }
        }
        Tool::Rect
        | Tool::Ellipse
        | Tool::Triangle
        | Tool::Diamond
        | Tool::Line
        | Tool::Arrow
        | Tool::Freehand
        | Tool::Highlighter => CursorStyle::Crosshair,
        Tool::Text | Tool::StickyNote => CursorStyle::Text,
        Tool::Eraser => CursorStyle::Default,
        Tool::Lasso => CursorStyle::Crosshair,
    }
}

fn handle_cursor(handle: usize) -> CursorStyle {
    match handle {
        0 | 3 => CursorStyle::NwseResize,
        1 | 2 => CursorStyle::NeswResize,
        4 | 5 => CursorStyle::NsResize,
        6 | 7 => CursorStyle::EwResize,
        _ => CursorStyle::Default,
    }
}

/// Auto-pan the camera when cursor is near viewport edge.
pub(crate) fn edge_scroll(state: &mut AppState, x: f64, y: f64) {
    const EDGE_MARGIN: f64 = 40.0;
    const MAX_SPEED: f64 = 8.0;

    let vw = state.viewport_w;
    let vh = state.viewport_h;
    let mut pan_x = 0.0;
    let mut pan_y = 0.0;

    if x < EDGE_MARGIN {
        pan_x = MAX_SPEED * (1.0 - x / EDGE_MARGIN);
    } else if x > vw - EDGE_MARGIN {
        pan_x = -MAX_SPEED * (1.0 - (vw - x) / EDGE_MARGIN);
    }
    if y < EDGE_MARGIN {
        pan_y = MAX_SPEED * (1.0 - y / EDGE_MARGIN);
    } else if y > vh - EDGE_MARGIN {
        pan_y = -MAX_SPEED * (1.0 - (vh - y) / EDGE_MARGIN);
    }

    if pan_x.abs() > 0.01 || pan_y.abs() > 0.01 {
        state.camera.pan(pan_x, pan_y);
    }
}

// ── Tooltip hover tracking ──

pub(crate) fn update_tooltip_hover(state: &mut AppState, x: f64, y: f64) {
    let hit = toolbar_hit_with_rect(state, x, y);
    match hit {
        Some((tool, btn_cx, btn_bottom)) => {
            let label = format!("{} ({})", tool.label(), tool.shortcut());
            if let Some(ref mut tt) = state.tooltip {
                if tt.text == label {
                    return;
                }
            }
            state.tooltip = Some(crate::state::TooltipState {
                sx: btn_cx,
                sy: btn_bottom + 4.0,
                text: label,
                hover_time: 0.0,
            });
        }
        None => {
            state.tooltip = None;
        }
    }
}

/// Like toolbar_hit but also returns button center-x and bottom-y.
fn toolbar_hit_with_rect(state: &AppState, sx: f64, sy: f64) -> Option<(Tool, f64, f64)> {
    let scale = ui::overlay_scale(state.viewport_w, state.viewport_h);
    let (bar_x, bar_y, _, _) = ui::toolbar_rect(state.viewport_w, state.viewport_h);
    let btn_size = 36.0 * scale;
    let btn_gap = 2.0 * scale;
    let pill_pad_x = 6.0 * scale;
    let pill_pad_y = 6.0 * scale;
    let sep_w = 8.0 * scale;
    let pill_h = btn_size + pill_pad_y * 2.0;

    if sy < bar_y || sy > bar_y + pill_h {
        return None;
    }

    let groups: &[&[Tool]] = &[
        &[Tool::Select, Tool::Lasso],
        &[Tool::Rect, Tool::Ellipse, Tool::Triangle, Tool::Diamond],
        &[Tool::Line, Tool::Arrow, Tool::Freehand, Tool::Highlighter],
        &[Tool::StickyNote, Tool::Text, Tool::Eraser],
    ];

    let mut cursor_x = bar_x + pill_pad_x;
    let btn_y = bar_y + pill_pad_y;

    for (gi, group) in groups.iter().enumerate() {
        for &tool in *group {
            if sx >= cursor_x && sx <= cursor_x + btn_size && sy >= btn_y && sy <= btn_y + btn_size
            {
                return Some((tool, cursor_x + btn_size / 2.0, btn_y + btn_size));
            }
            cursor_x += btn_size + btn_gap;
        }
        if gi < groups.len() - 1 {
            cursor_x -= btn_gap;
            cursor_x += sep_w + btn_gap;
        }
    }
    None
}

/// Toolbar hit test using grouped layout.
pub fn toolbar_hit(state: &AppState, sx: f64, sy: f64) -> Option<Tool> {
    toolbar_hit_with_rect(state, sx, sy).map(|(tool, _, _)| tool)
}

/// Color picker hit test with proper positioning.
pub fn color_picker_hit_with_viewport(sx: f64, sy: f64, viewport_h: f64) -> Option<u32> {
    color_picker_hit(sx, sy, viewport_h)
}

pub(crate) fn color_picker_hit(sx: f64, sy: f64, viewport_h: f64) -> Option<u32> {
    use crate::state::PALETTE;
    let swatch_d = 24.0;
    let gap = 8.0;
    let margin = 16.0;
    let swatch_r = swatch_d / 2.0;
    let pill_pad = 10.0;
    let status_h = 28.0;

    let num = PALETTE.len() as f64;
    let total_w = num * swatch_d + (num - 1.0) * gap;
    let pill_w = total_w + pill_pad * 2.0;
    let pill_h = swatch_d + pill_pad * 2.0;
    let pill_x = margin;
    let pill_y = viewport_h - margin - status_h - pill_h;

    if sx < pill_x || sx > pill_x + pill_w || sy < pill_y || sy > pill_y + pill_h {
        return None;
    }

    let start_x = pill_x + pill_pad + swatch_r;
    let center_y = pill_y + pill_pad + swatch_r;

    for (i, &color) in PALETTE.iter().enumerate() {
        let scx = start_x + i as f64 * (swatch_d + gap);
        let dx = sx - scx;
        let dy = sy - center_y;
        if dx * dx + dy * dy <= swatch_r * swatch_r {
            return Some(color);
        }
    }
    None
}
