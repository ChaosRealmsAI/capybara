//! Excalidraw-like viewport input behavior.

use winit::event::MouseScrollDelta;

use crate::state::{AppState, CursorStyle, DragMode};

const LINE_SCROLL_PX: f64 = 32.0;
const MAX_ZOOM_WHEEL_PX: f64 = 500.0;
const ZOOM_WHEEL_PX_PER_DOUBLING: f64 = 500.0;

pub(crate) fn handle_middle_button(state: &mut AppState, pressed: bool) -> bool {
    if pressed {
        begin_screen_pan(state);
        return true;
    }
    if state.drag_mode == DragMode::Panning {
        state.drag_mode = DragMode::None;
        state.cursor_style = CursorStyle::Default;
        return true;
    }
    false
}

pub(crate) fn handle_scroll(
    state: &mut AppState,
    delta: MouseScrollDelta,
    cmd_or_ctrl: bool,
    shift: bool,
) -> bool {
    let (mut dx, mut dy) = scroll_delta_to_screen_px(delta);
    if !dx.is_finite() || !dy.is_finite() || (dx == 0.0 && dy == 0.0) {
        return false;
    }

    if cmd_or_ctrl {
        let zoom_delta = if dy.abs() >= dx.abs() { dy } else { dx };
        zoom_about_cursor(state, zoom_delta);
        return true;
    }

    if shift {
        dx = if dy.abs() > 0.0 { dy } else { dx };
        dy = 0.0;
    }
    state.camera.pan(-dx, -dy);
    true
}

fn begin_screen_pan(state: &mut AppState) {
    state.drag_start_sx = state.cursor_x;
    state.drag_start_sy = state.cursor_y;
    state.drag_mode = DragMode::Panning;
    state.cursor_style = CursorStyle::Grabbing;
}

fn scroll_delta_to_screen_px(delta: MouseScrollDelta) -> (f64, f64) {
    match delta {
        MouseScrollDelta::LineDelta(x, y) => {
            (f64::from(x) * LINE_SCROLL_PX, f64::from(y) * LINE_SCROLL_PX)
        }
        MouseScrollDelta::PixelDelta(pos) => (pos.x, pos.y),
    }
}

fn zoom_about_cursor(state: &mut AppState, wheel_px: f64) {
    let clamped = wheel_px.clamp(-MAX_ZOOM_WHEEL_PX, MAX_ZOOM_WHEEL_PX);
    let factor = 2.0_f64.powf(clamped / ZOOM_WHEEL_PX_PER_DOUBLING);
    let sx = finite_or(state.cursor_x, state.viewport_w / 2.0);
    let sy = finite_or(state.cursor_y, state.viewport_h / 2.0);
    state.camera.zoom_at(sx, sy, factor);
    state.target_zoom = state.camera.zoom;
}

fn finite_or(value: f64, fallback: f64) -> f64 {
    if value.is_finite() { value } else { fallback }
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::dpi::PhysicalPosition;
    use winit::event::MouseButton;

    use crate::input_tools::resize_shape;
    use crate::mouse::{handle_mouse_button, handle_mouse_move};
    use crate::state::{Shape, ShapeKind, Tool};

    fn rect_at(x: f64, y: f64) -> Shape {
        let mut shape = Shape::new(ShapeKind::Rect, x, y, 0x5b8abf);
        shape.w = 120.0;
        shape.h = 80.0;
        shape
    }

    fn highlighter_at(x: f64, y: f64) -> Shape {
        let mut shape = Shape::new(ShapeKind::Highlighter, x, y, 0xfbbf24);
        shape.w = 200.0;
        shape.h = 100.0;
        shape.points = vec![
            (x + 10.0, y + 20.0),
            (x + 80.0, y + 45.0),
            (x + 190.0, y + 90.0),
        ];
        shape
    }

    #[test]
    fn viewport_interaction_cmd_wheel_zooms_around_cursor() {
        let mut state = AppState::new();
        state.camera.offset_x = -140.0;
        state.camera.offset_y = 60.0;
        state.camera.zoom = 1.5;
        state.target_zoom = state.camera.zoom;
        state.cursor_x = 420.0;
        state.cursor_y = 260.0;

        let before = state.camera.screen_to_world(state.cursor_x, state.cursor_y);
        assert!(handle_scroll(
            &mut state,
            MouseScrollDelta::PixelDelta(PhysicalPosition::new(0.0, 120.0)),
            true,
            false
        ));
        let after = state.camera.screen_to_world(state.cursor_x, state.cursor_y);

        assert!(state.camera.zoom > 1.5);
        assert_eq!(state.target_zoom, state.camera.zoom);
        assert!((before.0 - after.0).abs() < 1e-6);
        assert!((before.1 - after.1).abs() < 1e-6);
    }

    #[test]
    fn viewport_interaction_plain_wheel_pans_not_zooms() {
        let mut state = AppState::new();
        state.camera.zoom = 2.0;
        state.target_zoom = 2.0;

        assert!(handle_scroll(
            &mut state,
            MouseScrollDelta::PixelDelta(PhysicalPosition::new(24.0, -40.0)),
            false,
            false
        ));

        assert_eq!(state.camera.zoom, 2.0);
        assert_eq!(state.target_zoom, 2.0);
        assert_eq!(state.camera.offset_x, -24.0);
        assert_eq!(state.camera.offset_y, 40.0);
    }

    #[test]
    fn viewport_interaction_shift_wheel_pans_horizontally() {
        let mut state = AppState::new();

        assert!(handle_scroll(
            &mut state,
            MouseScrollDelta::PixelDelta(PhysicalPosition::new(0.0, 40.0)),
            false,
            true
        ));

        assert_eq!(state.camera.offset_x, -40.0);
        assert_eq!(state.camera.offset_y, 0.0);
    }

    #[test]
    fn viewport_interaction_middle_button_enters_hand_pan() {
        let mut state = AppState::new();
        state.cursor_x = 100.0;
        state.cursor_y = 100.0;

        assert!(handle_mouse_button(
            &mut state,
            MouseButton::Middle,
            true,
            false
        ));
        assert_eq!(state.drag_mode, DragMode::Panning);
        assert!(handle_mouse_move(&mut state, 130.0, 115.0));
        assert_eq!(state.camera.offset_x, 30.0);
        assert_eq!(state.camera.offset_y, 15.0);
        assert!(handle_mouse_button(
            &mut state,
            MouseButton::Middle,
            false,
            false
        ));
        assert_eq!(state.drag_mode, DragMode::None);
    }

    #[test]
    fn viewport_interaction_selected_move_uses_world_delta_after_zoom() {
        let mut state = AppState::new();
        state.camera.zoom = 2.0;
        state.camera.offset_x = 50.0;
        state.camera.offset_y = -30.0;
        let idx = state.add_shape(rect_at(100.0, 80.0));
        state.selected = vec![idx];
        state.tool = Tool::Select;
        let sx = state.shapes[idx].x * state.camera.zoom + state.camera.offset_x + 20.0;
        let sy = state.shapes[idx].y * state.camera.zoom + state.camera.offset_y + 20.0;

        handle_mouse_move(&mut state, sx, sy);
        assert!(handle_mouse_button(
            &mut state,
            MouseButton::Left,
            true,
            false
        ));
        assert!(handle_mouse_move(&mut state, sx + 40.0, sy + 20.0));

        assert!((state.shapes[idx].x - 120.0).abs() < 1e-6);
        assert!((state.shapes[idx].y - 90.0).abs() < 1e-6);
    }

    #[test]
    fn viewport_interaction_selected_path_move_keeps_points_with_frame() {
        let mut state = AppState::new();
        state.camera.zoom = 2.0;
        state.camera.offset_x = 50.0;
        state.camera.offset_y = -30.0;
        let idx = state.add_shape(highlighter_at(100.0, 80.0));
        state.selected = vec![idx];
        state.tool = Tool::Select;
        let before_points = state.shapes[idx].points.clone();
        let sx = state.shapes[idx].x * state.camera.zoom + state.camera.offset_x + 40.0;
        let sy = state.shapes[idx].y * state.camera.zoom + state.camera.offset_y + 40.0;

        handle_mouse_move(&mut state, sx, sy);
        assert!(handle_mouse_button(
            &mut state,
            MouseButton::Left,
            true,
            false
        ));
        assert!(handle_mouse_move(&mut state, sx + 80.0, sy + 40.0));

        assert!((state.shapes[idx].x - 140.0).abs() < 1e-6);
        assert!((state.shapes[idx].y - 100.0).abs() < 1e-6);
        for (before, after) in before_points.iter().zip(&state.shapes[idx].points) {
            assert!((after.0 - before.0 - 40.0).abs() < 1e-6);
            assert!((after.1 - before.1 - 20.0).abs() < 1e-6);
        }
    }

    #[test]
    fn viewport_interaction_path_resize_scales_points_with_frame() {
        let mut shape = highlighter_at(100.0, 80.0);

        resize_shape(&mut shape, 3, 350.0, 230.0);

        assert_eq!(shape.x, 100.0);
        assert_eq!(shape.y, 80.0);
        assert_eq!(shape.w, 250.0);
        assert_eq!(shape.h, 150.0);
        assert!((shape.points[0].0 - 112.5).abs() < 1e-6);
        assert!((shape.points[0].1 - 110.0).abs() < 1e-6);
        assert!((shape.points[2].0 - 337.5).abs() < 1e-6);
        assert!((shape.points[2].1 - 215.0).abs() < 1e-6);
    }
}
