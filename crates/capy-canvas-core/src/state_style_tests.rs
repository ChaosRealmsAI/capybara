//! Unit tests for canvas vector style defaults and contracts.

use super::*;

#[test]
fn default_canvas_style_is_warm_sketch() {
    let state = AppState::new();
    assert_eq!(state.color, 0x8a6fae);
    assert_eq!(state.fill_color, 0xfef3c7);
    assert_eq!(state.fill_style, FillStyle::Hachure);
    assert!(
        state.stroke_width >= 2.0,
        "sketch strokes must stay visibly weighted"
    );
}

#[test]
fn ai_snapshot_exposes_vector_style() {
    let mut state = AppState::new();
    let mut shape = Shape::new(ShapeKind::Rect, 0.0, 0.0, 0x8a6fae);
    shape.w = 120.0;
    shape.h = 80.0;
    shape.stroke_color = 0x8a6fae;
    shape.color = 0xfef3c7;
    shape.fill_style = FillStyle::Hachure;
    state.add_shape(shape);

    let snapshot = state.ai_snapshot();
    assert_eq!(snapshot.nodes[0].stroke_color, "#8a6fae");
    assert_eq!(snapshot.nodes[0].fill_color, "#fef3c7");
    assert_eq!(snapshot.nodes[0].fill_style, "hachure");
}

#[test]
fn bound_arrow_creation_uses_current_vector_style() {
    let mut state = AppState::new();
    let mut from = Shape::new(ShapeKind::Rect, 0.0, 0.0, 0x8a6fae);
    from.w = 100.0;
    from.h = 80.0;
    let mut to = Shape::new(ShapeKind::Rect, 300.0, 0.0, 0x8a6fae);
    to.w = 100.0;
    to.h = 80.0;
    state.add_shape(from);
    state.add_shape(to);
    state.tool = Tool::Arrow;
    state.color = 0xd94f5c;
    state.fill_color = 0xfde2e7;
    state.fill_style = FillStyle::Hachure;

    state.cursor_x = 100.0;
    state.cursor_y = 40.0;
    crate::mouse::handle_mouse_button(&mut state, winit::event::MouseButton::Left, true, false);
    crate::mouse::handle_mouse_move(&mut state, 300.0, 40.0);
    crate::mouse::handle_mouse_button(&mut state, winit::event::MouseButton::Left, false, false);

    let arrow = state
        .shapes
        .iter()
        .find(|shape| shape.kind == ShapeKind::Arrow)
        .expect("bound arrow should be created");
    assert_eq!(arrow.stroke_color, 0xd94f5c);
    assert_eq!(arrow.color, 0xfde2e7);
    assert_eq!(arrow.fill_style, FillStyle::Hachure);
}
