#![allow(dead_code, unused_imports)]

pub use capy_canvas_core::input::{handle_mouse_button, handle_mouse_move};
pub use capy_canvas_core::state::{
    AppState, Camera, CanvasContentKind, DragMode, Shape, ShapeKind,
};
pub use winit::event::MouseButton;

pub fn click_at(state: &mut AppState, sx: f64, sy: f64, shift: bool) {
    state.cursor_x = sx;
    state.cursor_y = sy;
    handle_mouse_button(state, MouseButton::Left, true, shift);
    handle_mouse_button(state, MouseButton::Left, false, shift);
}

pub fn drag(state: &mut AppState, x1: f64, y1: f64, x2: f64, y2: f64, shift: bool) {
    state.alt_held = true;
    state.cursor_x = x1;
    state.cursor_y = y1;
    handle_mouse_button(state, MouseButton::Left, true, shift);
    handle_mouse_move(state, x2, y2);
    handle_mouse_button(state, MouseButton::Left, false, shift);
    state.alt_held = false;
}

pub fn add_rect(state: &mut AppState, x: f64, y: f64, w: f64, h: f64) -> usize {
    let mut shape = Shape::new(ShapeKind::Rect, x, y, 0x1e1e1e);
    shape.w = w;
    shape.h = h;
    state.add_shape(shape)
}

pub fn add_ellipse(state: &mut AppState, x: f64, y: f64, w: f64, h: f64) -> usize {
    let mut shape = Shape::new(ShapeKind::Ellipse, x, y, 0x1e1e1e);
    shape.w = w;
    shape.h = h;
    state.add_shape(shape)
}

pub fn add_line(state: &mut AppState, x1: f64, y1: f64, x2: f64, y2: f64) -> usize {
    let mut shape = Shape::new(ShapeKind::Line, x1, y1, 0x1e1e1e);
    shape.w = x2 - x1;
    shape.h = y2 - y1;
    state.add_shape(shape)
}
