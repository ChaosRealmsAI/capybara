//! Mouse interaction handlers and cursor-driven state updates.

use winit::event::{MouseButton, MouseScrollDelta};

use crate::input_tools::{
    self, compute_cursor_style, edge_scroll, handle_arrow_or_line_down, handle_draw_start,
    handle_eraser, handle_lasso_start, handle_select_down, handle_sticky_click, handle_text_click,
    resize_shape, update_tooltip_hover,
};
use crate::line_edit;
use crate::state::{
    AppState, ContextAction, ContextMenu, ContextMenuItem, CursorStyle, DragMode, Shape, ShapeKind,
    Tool,
};
use crate::ui;
use crate::viewport_interaction;

pub fn handle_mouse_button(
    state: &mut AppState,
    button: MouseButton,
    pressed: bool,
    shift: bool,
) -> bool {
    if button == MouseButton::Right {
        if pressed {
            return handle_right_click(state, state.cursor_x, state.cursor_y);
        }
        return false;
    }
    if button == MouseButton::Middle {
        return viewport_interaction::handle_middle_button(state, pressed);
    }
    if button != MouseButton::Left {
        return false;
    }

    if pressed {
        if let Some(ref menu) = state.context_menu {
            if let Some(action) = menu
                .hit_item(state.cursor_x, state.cursor_y)
                .and_then(|idx| menu.items.get(idx))
                .map(|item| item.action)
            {
                state.context_menu = None;
                return execute_context_action(state, action);
            }
            state.context_menu = None;
            return true;
        }
    }

    let (wx, wy) = state.camera.screen_to_world(state.cursor_x, state.cursor_y);
    if pressed {
        handle_mouse_down(state, state.cursor_x, state.cursor_y, wx, wy, shift)
    } else {
        handle_mouse_up(state)
    }
}

fn menu_item(label: &'static str, action: ContextAction) -> ContextMenuItem {
    ContextMenuItem { label, action }
}

fn handle_right_click(state: &mut AppState, sx: f64, sy: f64) -> bool {
    let (wx, wy) = state.camera.screen_to_world(sx, sy);
    if let Some(idx) = state.hit_test(wx, wy) {
        if !state.selected.contains(&idx) {
            state.selected = vec![idx];
        }
        let mut items = vec![
            menu_item("Bring to Front", ContextAction::BringToFront),
            menu_item("Send to Back", ContextAction::SendToBack),
            menu_item("Forward One", ContextAction::SendForward),
            menu_item("Backward One", ContextAction::SendBackward),
            menu_item("Duplicate", ContextAction::Duplicate),
            menu_item("Delete", ContextAction::Delete),
        ];
        if state.selected.len() >= 2 {
            items.extend([
                menu_item("Align Left", ContextAction::AlignLeft),
                menu_item("Align Center H", ContextAction::AlignCenterH),
                menu_item("Align Right", ContextAction::AlignRight),
                menu_item("Align Top", ContextAction::AlignTop),
                menu_item("Align Center V", ContextAction::AlignCenterV),
                menu_item("Align Bottom", ContextAction::AlignBottom),
            ]);
        }
        if state.selected.len() >= 3 {
            items.extend([
                menu_item("Distribute H", ContextAction::DistributeH),
                menu_item("Distribute V", ContextAction::DistributeV),
            ]);
        }
        state.context_menu = Some(ContextMenu {
            sx,
            sy,
            items,
            hovered: None,
        });
    } else {
        state.context_menu = Some(ContextMenu {
            sx,
            sy,
            hovered: None,
            items: vec![
                menu_item("Paste", ContextAction::Paste),
                menu_item("Select All", ContextAction::SelectAll),
                menu_item("Reset Zoom", ContextAction::ResetZoom),
            ],
        });
    }
    true
}

fn execute_context_action(state: &mut AppState, action: ContextAction) -> bool {
    match action {
        ContextAction::BringToFront => state.bring_to_front(),
        ContextAction::SendToBack => state.send_to_back(),
        ContextAction::Duplicate => state.duplicate_selected(20.0, 20.0),
        ContextAction::Delete => state.delete_selected(),
        ContextAction::Paste => {
            let (wx, wy) = state.camera.screen_to_world(state.cursor_x, state.cursor_y);
            state.paste_at(wx, wy);
        }
        ContextAction::SelectAll => state.selected = (0..state.shapes.len()).collect(),
        ContextAction::ResetZoom => {
            state.camera.zoom = 1.0;
            state.target_zoom = 1.0;
            state.camera.offset_x = 0.0;
            state.camera.offset_y = 0.0;
        }
        ContextAction::AlignLeft => state.align_left(),
        ContextAction::AlignCenterH => state.align_center_h(),
        ContextAction::AlignRight => state.align_right(),
        ContextAction::AlignTop => state.align_top(),
        ContextAction::AlignCenterV => state.align_center_v(),
        ContextAction::AlignBottom => state.align_bottom(),
        ContextAction::DistributeH => state.distribute_h(),
        ContextAction::DistributeV => state.distribute_v(),
        ContextAction::SendForward => state.send_forward(),
        ContextAction::SendBackward => state.send_backward(),
    }
    true
}

pub fn handle_double_click(state: &mut AppState) -> bool {
    line_edit::handle_double_click(state)
}

fn handle_mouse_down(
    state: &mut AppState,
    sx: f64,
    sy: f64,
    wx: f64,
    wy: f64,
    shift: bool,
) -> bool {
    state.drag_start_sx = sx;
    state.drag_start_sy = sy;

    if state.space_held {
        state.drag_mode = DragMode::Panning;
        return true;
    }
    if let Some((mwx, mwy)) = crate::minimap::hit_test(state, sx, sy) {
        state.camera.offset_x = state.viewport_w / 2.0 - mwx * state.camera.zoom;
        state.camera.offset_y = state.viewport_h / 2.0 - mwy * state.camera.zoom;
        return true;
    }
    if let Some(tool) = input_tools::toolbar_hit(state, sx, sy) {
        state.tool = tool;
        state.connector_from = None;
        return true;
    }
    // Color picker removed — colors are in the style panel
    if ui::handle_style_panel_mouse_down(state, sx, sy) {
        return true;
    }

    match state.tool {
        Tool::Select => handle_select_down(state, wx, wy, shift),
        Tool::Eraser => handle_eraser(state, wx, wy),
        Tool::Arrow | Tool::Line => handle_arrow_or_line_down(state, wx, wy),
        Tool::Text => handle_text_click(state, wx, wy),
        Tool::StickyNote => handle_sticky_click(state, wx, wy),
        Tool::Lasso => handle_lasso_start(state, wx, wy),
        _ => handle_draw_start(state, wx, wy),
    }
}

fn handle_mouse_up(state: &mut AppState) -> bool {
    let (was_creating, was_lasso, was_dragging) = (
        state.drag_mode == DragMode::Creating,
        state.drag_mode == DragMode::Lasso,
        state.drag_mode != DragMode::None,
    );
    if let DragMode::ConnectorDrag { from_id } = state.drag_mode {
        let (wx, wy) = state.camera.screen_to_world(state.cursor_x, state.cursor_y);
        let target_id = state.shapes.iter().find_map(|s| {
            let nx = wx >= s.x - 30.0 && wx <= s.x + s.w + 30.0;
            let ny = wy >= s.y - 30.0 && wy <= s.y + s.h + 30.0;
            (nx && ny && s.w > 1.0 && s.id != from_id).then_some(s.id)
        });
        if let Some(tid) = target_id {
            let fc = state.shape_by_id(from_id).map(|s| s.center());
            let tc = state.shape_by_id(tid).map(|s| s.center());
            if let (Some((fx, fy)), Some((tx, ty))) = (fc, tc) {
                state.push_undo();
                let kind = if state.tool == Tool::Line {
                    ShapeKind::Line
                } else {
                    ShapeKind::Arrow
                };
                let mut arrow = Shape::new(kind, fx, fy, Shape::default_color_for_kind(kind));
                arrow.w = tx - fx;
                arrow.h = ty - fy;
                arrow.binding_start = Some(from_id);
                arrow.binding_end = Some(tid);
                input_tools::apply_style_from_state(state, &mut arrow);
                let idx = state.add_shape(arrow);
                state.selected = vec![idx];
            }
        }
        state.connector_preview = None;
        state.connector_from = None;
        state.binding_indicator = None;
        state.drag_mode = DragMode::None;
        state.tool = Tool::Select;
        return true;
    }

    if was_lasso && state.lasso_points.len() >= 3 {
        state.selected.clear();
        let polygon = state.lasso_points.clone();
        for (i, shape) in state.shapes.iter().enumerate() {
            let (cx, cy) = shape.center();
            if crate::state::point_in_polygon(cx, cy, &polygon) {
                state.selected.push(i);
            }
        }
        state.lasso_points.clear();
        state.drag_mode = DragMode::None;
        state.tool = Tool::Select;
        return true;
    }
    state.lasso_points.clear();
    if was_creating {
        crate::input_finalize::finalize_created_shape(state);
    }
    state.drag_mode = DragMode::None;
    state.drag_shape_origins.clear();
    state.rubber_band = None;

    if was_creating && !matches!(state.tool, Tool::Freehand | Tool::Highlighter) {
        state.tool = Tool::Select;
    }
    was_dragging
}

pub fn handle_mouse_move(state: &mut AppState, x: f64, y: f64) -> bool {
    let prev_x = state.cursor_x;
    let prev_y = state.cursor_y;
    state.cursor_x = x;
    state.cursor_y = y;

    if let Some(ref mut menu) = state.context_menu {
        let new_hovered = menu.hit_item(x, y);
        if new_hovered != menu.hovered {
            menu.hovered = new_hovered;
            return true;
        }
    }

    update_tooltip_hover(state, x, y);

    match state.drag_mode {
        DragMode::None => handle_move_idle(state, x, y),
        DragMode::Panning => {
            state.camera.pan(x - prev_x, y - prev_y);
            state.cursor_style = CursorStyle::Grabbing;
            true
        }
        DragMode::Creating => handle_move_creating(state, x, y),
        DragMode::StylePanelDrag { .. } => ui::handle_style_panel_drag(state, x, y),
        DragMode::LineHandleDrag { index, handle } => {
            line_edit::handle_drag_move_screen(state, index, handle, x, y)
        }
        DragMode::Moving { start_wx, start_wy } => {
            let (wx, wy) = state.camera.screen_to_world(x, y);
            let dx = wx - start_wx;
            let dy = wy - start_wy;
            for (i, &idx) in state.selected.iter().enumerate() {
                if idx < state.shapes.len() && i < state.drag_shape_origins.len() {
                    let (ox, oy) = state.drag_shape_origins[i];
                    state.shapes[idx].move_to(ox + dx, oy + dy);
                }
            }
            edge_scroll(state, x, y);
            true
        }
        DragMode::Resizing { handle } => {
            let (wx, wy) = state.camera.screen_to_world(x, y);
            if let Some(&idx) = state.selected.first() {
                if idx < state.shapes.len() {
                    resize_shape(&mut state.shapes[idx], handle, wx, wy);
                }
            }
            true
        }
        DragMode::Rotating => {
            let (wx, wy) = state.camera.screen_to_world(x, y);
            if let Some(&idx) = state.selected.first() {
                if idx < state.shapes.len() {
                    let (cx, cy) = state.shapes[idx].center();
                    state.shapes[idx].rotation =
                        (wy - cy).atan2(wx - cx) + std::f64::consts::FRAC_PI_2;
                }
            }
            state.cursor_style = CursorStyle::Grabbing;
            true
        }
        DragMode::Erasing => {
            let (wx, wy) = state.camera.screen_to_world(x, y);
            if let Some(idx) = state.hit_test(wx, wy) {
                state.push_undo();
                let removed_id = state.shapes[idx].id;
                state.shapes.remove(idx);
                crate::connector::remove_connectors_for_shape(state, removed_id);
                crate::connector::clear_bindings_for_shape(state, removed_id);
                state.selected.clear();
            }
            true
        }
        DragMode::RubberBand { start_wx, start_wy } => {
            let (wx, wy) = state.camera.screen_to_world(x, y);
            let rx = start_wx.min(wx);
            let ry = start_wy.min(wy);
            let rw = (wx - start_wx).abs();
            let rh = (wy - start_wy).abs();
            state.selected.clear();
            for (i, shape) in state.shapes.iter().enumerate() {
                if shape.x < rx + rw
                    && shape.x + shape.w > rx
                    && shape.y < ry + rh
                    && shape.y + shape.h > ry
                {
                    state.selected.push(i);
                }
            }
            state.rubber_band = Some((rx, ry, rw, rh));
            true
        }
        DragMode::OpacityDrag => ui::handle_style_panel_drag(state, x, y),
        DragMode::Lasso => {
            let (wx, wy) = state.camera.screen_to_world(x, y);
            state.lasso_points.push((wx, wy));
            true
        }
        DragMode::ConnectorDrag { .. } => {
            let (wx, wy) = state.camera.screen_to_world(x, y);
            if let Some(ref mut preview) = state.connector_preview {
                preview.2 = wx;
                preview.3 = wy;
            }
            state.binding_indicator = None;
            let near_idx = state.shapes.iter().enumerate().find_map(|(i, s)| {
                let nx = wx >= s.x - 30.0 && wx <= s.x + s.w + 30.0;
                let ny = wy >= s.y - 30.0 && wy <= s.y + s.h + 30.0;
                (nx && ny && (s.w > 1.0 || s.h > 1.0)).then_some(i)
            });
            if let Some(h) = near_idx {
                let anchors = state.shapes[h].anchor_points();
                let best = anchors.iter().copied().min_by(|a, b| {
                    let da = (wx - a.0).powi(2) + (wy - a.1).powi(2);
                    let db = (wx - b.0).powi(2) + (wy - b.1).powi(2);
                    da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                });
                state.binding_indicator = best;
            }
            edge_scroll(state, x, y);
            true
        }
    }
}

fn handle_move_idle(state: &mut AppState, x: f64, y: f64) -> bool {
    let (wx, wy) = state.camera.screen_to_world(x, y);
    let old_hovered = state.hovered_shape;
    let old_binding = state.binding_indicator;
    if let Some(cursor) = ui::style_panel_cursor(state, x, y) {
        state.hovered_shape = None;
        state.binding_indicator = None;
        state.cursor_style = cursor;
        return true;
    }
    state.hovered_shape = state.hit_test(wx, wy);
    if let Some(h) = state.hovered_shape {
        if state.selected.contains(&h) {
            state.hovered_shape = None;
        }
    }
    state.cursor_style = compute_cursor_style(state, wx, wy);

    state.binding_indicator = None;
    if matches!(state.tool, Tool::Arrow | Tool::Line) {
        let near_idx = state
            .shapes
            .iter()
            .enumerate()
            .find_map(|(i, s)| {
                let nx = wx >= s.x - 30.0 && wx <= s.x + s.w + 30.0;
                let ny = wy >= s.y - 30.0 && wy <= s.y + s.h + 30.0;
                (nx && ny && (s.w > 1.0 || s.h > 1.0)).then_some(i)
            })
            .or_else(|| state.hovered_shape.or(state.hit_test(wx, wy)));
        if let Some(h) = near_idx {
            let anchors = state.shapes[h].anchor_points();
            let best = anchors.iter().copied().min_by(|a, b| {
                let da = (wx - a.0).powi(2) + (wy - a.1).powi(2);
                let db = (wx - b.0).powi(2) + (wy - b.1).powi(2);
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            });
            state.binding_indicator = best;
        }
    }

    let (bar_x, bar_y, bar_w, bar_h) = ui::toolbar_rect(state.viewport_w, state.viewport_h);
    let in_toolbar = x >= bar_x && x <= bar_x + bar_w && y >= bar_y && y <= bar_y + bar_h;

    old_hovered != state.hovered_shape || in_toolbar || old_binding != state.binding_indicator
}

fn handle_move_creating(state: &mut AppState, x: f64, y: f64) -> bool {
    let (wx, wy) = state.camera.screen_to_world(x, y);
    if let Some(&idx) = state.selected.first() {
        if idx < state.shapes.len() {
            let shape = &mut state.shapes[idx];
            if matches!(shape.kind, ShapeKind::Freehand | ShapeKind::Highlighter) {
                shape.points.push((wx, wy));
                let min_x = shape.points.iter().map(|p| p.0).fold(f64::MAX, f64::min);
                let min_y = shape.points.iter().map(|p| p.1).fold(f64::MAX, f64::min);
                let max_x = shape.points.iter().map(|p| p.0).fold(f64::MIN, f64::max);
                let max_y = shape.points.iter().map(|p| p.1).fold(f64::MIN, f64::max);
                shape.x = min_x;
                shape.y = min_y;
                shape.w = max_x - min_x;
                shape.h = max_y - min_y;
            } else {
                let (start_wx, start_wy) = state
                    .drag_shape_origins
                    .first()
                    .copied()
                    .unwrap_or_else(|| {
                        state
                            .camera
                            .screen_to_world(state.drag_start_sx, state.drag_start_sy)
                    });
                let snap = |v: f64| (v / 20.0).round() * 20.0;
                let (end_wx, end_wy) = if state.alt_held {
                    (wx, wy)
                } else {
                    (snap(wx), snap(wy))
                };
                if shape.kind == ShapeKind::Line || shape.kind == ShapeKind::Arrow {
                    shape.x = start_wx;
                    shape.y = start_wy;
                    shape.w = end_wx - start_wx;
                    shape.h = end_wy - start_wy;
                } else {
                    shape.x = start_wx.min(end_wx);
                    shape.y = start_wy.min(end_wy);
                    shape.w = (end_wx - start_wx).abs();
                    shape.h = (end_wy - start_wy).abs();
                }
            }
        }
    }
    true
}

pub fn handle_scroll(
    state: &mut AppState,
    delta: MouseScrollDelta,
    cmd_or_ctrl: bool,
    shift: bool,
) -> bool {
    viewport_interaction::handle_scroll(state, delta, cmd_or_ctrl, shift)
}
