mod common;
use common::*;

mod wave4_shapes {
    use super::*;
    use capy_canvas_core::state::{ShapeKind, Tool, point_in_polygon};

    // ── Triangle ──

    fn add_triangle(state: &mut AppState, x: f64, y: f64, w: f64, h: f64) -> usize {
        let mut shape = Shape::new(ShapeKind::Triangle, x, y, 0x1e1e1e);
        shape.w = w;
        shape.h = h;
        state.add_shape(shape)
    }

    #[test]
    fn triangle_hit_test_center() {
        let mut state = AppState::new();
        add_triangle(&mut state, 0.0, 0.0, 100.0, 100.0);
        // Center of bounding box (50, 50) should be inside triangle
        assert!(
            state.shapes[0].contains(50.0, 50.0),
            "center should be inside triangle"
        );
    }

    #[test]
    fn triangle_hit_test_outside_corners() {
        let mut state = AppState::new();
        add_triangle(&mut state, 0.0, 0.0, 100.0, 100.0);
        // Top-left corner of bbox is outside the triangle
        assert!(
            !state.shapes[0].contains(5.0, 5.0),
            "top-left bbox corner outside triangle"
        );
        // Top-right corner of bbox is outside the triangle
        assert!(
            !state.shapes[0].contains(95.0, 5.0),
            "top-right bbox corner outside triangle"
        );
    }

    #[test]
    fn triangle_hit_test_bottom_edge() {
        let mut state = AppState::new();
        add_triangle(&mut state, 0.0, 0.0, 100.0, 100.0);
        // Bottom-center should be inside (on the base of the triangle)
        assert!(
            state.shapes[0].contains(50.0, 100.0),
            "bottom-center on triangle base"
        );
    }

    #[test]
    fn triangle_create_and_select() {
        let mut state = AppState::new();
        let idx = add_triangle(&mut state, 10.0, 20.0, 80.0, 60.0);
        state.selected = vec![idx];
        assert_eq!(state.shapes[idx].kind, ShapeKind::Triangle);
        assert!((state.shapes[idx].w - 80.0).abs() < 1e-6);
    }

    // ── Diamond ──

    fn add_diamond(state: &mut AppState, x: f64, y: f64, w: f64, h: f64) -> usize {
        let mut shape = Shape::new(ShapeKind::Diamond, x, y, 0x1e1e1e);
        shape.w = w;
        shape.h = h;
        state.add_shape(shape)
    }

    #[test]
    fn diamond_hit_test_center() {
        let mut state = AppState::new();
        add_diamond(&mut state, 0.0, 0.0, 100.0, 100.0);
        assert!(
            state.shapes[0].contains(50.0, 50.0),
            "center should be inside diamond"
        );
    }

    #[test]
    fn diamond_hit_test_outside_corners() {
        let mut state = AppState::new();
        add_diamond(&mut state, 0.0, 0.0, 100.0, 100.0);
        // Corners of bbox are outside diamond
        assert!(
            !state.shapes[0].contains(2.0, 2.0),
            "top-left corner outside diamond"
        );
        assert!(
            !state.shapes[0].contains(98.0, 98.0),
            "bottom-right corner outside diamond"
        );
    }

    #[test]
    fn diamond_hit_test_midpoints() {
        let mut state = AppState::new();
        add_diamond(&mut state, 0.0, 0.0, 100.0, 100.0);
        // Points on edges (midpoints of sides) should be on the boundary
        assert!(state.shapes[0].contains(50.0, 0.0), "top vertex");
        assert!(state.shapes[0].contains(100.0, 50.0), "right vertex");
        assert!(state.shapes[0].contains(50.0, 100.0), "bottom vertex");
        assert!(state.shapes[0].contains(0.0, 50.0), "left vertex");
    }

    #[test]
    fn diamond_create_and_select() {
        let mut state = AppState::new();
        let idx = add_diamond(&mut state, 10.0, 20.0, 80.0, 60.0);
        state.selected = vec![idx];
        assert_eq!(state.shapes[idx].kind, ShapeKind::Diamond);
    }

    // ── Sticky Note ──

    #[test]
    fn sticky_note_default_size() {
        let mut state = AppState::new();
        let mut shape = Shape::new(ShapeKind::StickyNote, 100.0, 100.0, 0xfef3c7);
        shape.w = 200.0;
        shape.h = 200.0;
        let idx = state.add_shape(shape);
        assert_eq!(state.shapes[idx].kind, ShapeKind::StickyNote);
        assert!((state.shapes[idx].w - 200.0).abs() < 1e-6);
        assert!((state.shapes[idx].h - 200.0).abs() < 1e-6);
    }

    #[test]
    fn sticky_note_hit_test() {
        let mut state = AppState::new();
        let mut shape = Shape::new(ShapeKind::StickyNote, 0.0, 0.0, 0xfef3c7);
        shape.w = 200.0;
        shape.h = 200.0;
        state.add_shape(shape);
        assert!(
            state.shapes[0].contains(100.0, 100.0),
            "center inside sticky note"
        );
        assert!(
            !state.shapes[0].contains(250.0, 250.0),
            "outside sticky note"
        );
    }

    #[test]
    fn sticky_note_stores_text() {
        let mut state = AppState::new();
        let mut shape = Shape::new(ShapeKind::StickyNote, 0.0, 0.0, 0xfef3c7);
        shape.w = 200.0;
        shape.h = 200.0;
        shape.text = "Hello from sticky".to_string();
        let idx = state.add_shape(shape);
        assert_eq!(state.shapes[idx].text, "Hello from sticky");
    }

    // ── Highlighter ──

    #[test]
    fn highlighter_uses_points() {
        let mut state = AppState::new();
        let mut shape = Shape::new(ShapeKind::Highlighter, 0.0, 0.0, 0xf08c00);
        shape.points = vec![(0.0, 0.0), (10.0, 10.0), (20.0, 5.0)];
        shape.w = 20.0;
        shape.h = 10.0;
        let idx = state.add_shape(shape);
        assert_eq!(state.shapes[idx].kind, ShapeKind::Highlighter);
        assert_eq!(state.shapes[idx].points.len(), 3);
    }

    #[test]
    fn highlighter_hit_test_bbox() {
        let mut state = AppState::new();
        let mut shape = Shape::new(ShapeKind::Highlighter, 10.0, 10.0, 0xf08c00);
        shape.points = vec![(10.0, 10.0), (50.0, 50.0)];
        shape.w = 40.0;
        shape.h = 40.0;
        state.add_shape(shape);
        assert!(
            state.shapes[0].contains(30.0, 30.0),
            "inside highlighter bbox"
        );
        assert!(
            !state.shapes[0].contains(5.0, 5.0),
            "outside highlighter bbox"
        );
    }

    // ── Lasso / point-in-polygon ──

    #[test]
    fn point_in_polygon_square() {
        let polygon = vec![(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)];
        assert!(
            point_in_polygon(50.0, 50.0, &polygon),
            "center inside square"
        );
        assert!(!point_in_polygon(150.0, 50.0, &polygon), "outside square");
    }

    #[test]
    fn point_in_polygon_triangle() {
        let polygon = vec![(50.0, 0.0), (100.0, 100.0), (0.0, 100.0)];
        assert!(
            point_in_polygon(50.0, 50.0, &polygon),
            "inside triangle polygon"
        );
        assert!(
            !point_in_polygon(10.0, 10.0, &polygon),
            "outside triangle polygon"
        );
    }

    #[test]
    fn point_in_polygon_degenerate() {
        let polygon = vec![(0.0, 0.0), (1.0, 1.0)]; // only 2 points
        assert!(!point_in_polygon(0.5, 0.5, &polygon), "degenerate polygon");
    }

    // ── Tool variants ──

    #[test]
    fn new_tools_have_labels() {
        assert!(!Tool::Triangle.label().is_empty());
        assert!(!Tool::Diamond.label().is_empty());
        assert!(!Tool::StickyNote.label().is_empty());
        assert!(!Tool::Highlighter.label().is_empty());
        assert!(!Tool::Lasso.label().is_empty());
    }

    #[test]
    fn new_tools_have_shortcuts() {
        assert!(!Tool::Triangle.shortcut().is_empty());
        assert!(!Tool::Diamond.shortcut().is_empty());
        assert!(!Tool::StickyNote.shortcut().is_empty());
        assert!(!Tool::Highlighter.shortcut().is_empty());
        assert!(!Tool::Lasso.shortcut().is_empty());
    }

    #[test]
    fn all_toolbar_includes_new_tools() {
        let tools = Tool::all_toolbar();
        assert!(tools.contains(&Tool::Triangle), "Triangle in toolbar");
        assert!(tools.contains(&Tool::Diamond), "Diamond in toolbar");
        assert!(tools.contains(&Tool::StickyNote), "StickyNote in toolbar");
        assert!(tools.contains(&Tool::Highlighter), "Highlighter in toolbar");
        assert!(tools.contains(&Tool::Lasso), "Lasso in toolbar");
    }

    // ── Delete / Undo with new shapes ──

    #[test]
    fn delete_triangle() {
        let mut state = AppState::new();
        add_triangle(&mut state, 0.0, 0.0, 100.0, 100.0);
        state.selected = vec![0];
        state.delete_selected();
        assert!(state.shapes.is_empty());
    }

    #[test]
    fn undo_diamond_creation() {
        let mut state = AppState::new();
        state.push_undo();
        add_diamond(&mut state, 50.0, 50.0, 80.0, 80.0);
        assert_eq!(state.shapes.len(), 1);
        state.undo();
        assert!(state.shapes.is_empty());
    }

    #[test]
    fn copy_paste_sticky_note() {
        let mut state = AppState::new();
        let mut shape = Shape::new(ShapeKind::StickyNote, 0.0, 0.0, 0xfef3c7);
        shape.w = 200.0;
        shape.h = 200.0;
        shape.text = "Copy me".to_string();
        let idx = state.add_shape(shape);
        state.selected = vec![idx];
        state.copy_selected();
        state.paste_at(300.0, 300.0);
        assert_eq!(state.shapes.len(), 2);
        assert_eq!(state.shapes[1].kind, ShapeKind::StickyNote);
        assert_eq!(state.shapes[1].text, "Copy me");
    }

    #[test]
    fn copy_paste_highlighter_keeps_points_attached_to_frame() {
        let mut state = AppState::new();
        let mut shape = Shape::new(ShapeKind::Highlighter, 100.0, 120.0, 0xfbbf24);
        shape.w = 140.0;
        shape.h = 60.0;
        shape.points = vec![(110.0, 130.0), (180.0, 155.0), (238.0, 176.0)];
        let idx = state.add_shape(shape);
        state.selected = vec![idx];
        state.copy_selected();

        state.paste_at(360.0, 320.0);

        assert_eq!(state.shapes.len(), 2);
        let pasted = &state.shapes[1];
        assert_eq!(pasted.kind, ShapeKind::Highlighter);
        assert!((pasted.x - 290.0).abs() < 1e-6);
        assert!((pasted.y - 290.0).abs() < 1e-6);
        assert!((pasted.points[0].0 - 300.0).abs() < 1e-6);
        assert!((pasted.points[0].1 - 300.0).abs() < 1e-6);
        assert!((pasted.points[2].0 - 428.0).abs() < 1e-6);
        assert!((pasted.points[2].1 - 346.0).abs() < 1e-6);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Wave 5: Arrow & Connector Polish
// ═══════════════════════════════════════════════════════════════════════
