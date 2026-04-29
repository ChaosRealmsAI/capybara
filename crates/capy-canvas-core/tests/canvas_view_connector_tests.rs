mod common;
use common::*;

mod camera {
    use super::*;

    /// tldraw HandTool: initial camera at (0, 0, zoom=1).
    #[test]
    fn initial_camera_state() {
        let cam = Camera::default();
        assert!((cam.offset_x).abs() < 1e-6);
        assert!((cam.offset_y).abs() < 1e-6);
        assert!((cam.zoom - 1.0).abs() < 1e-6);
    }

    /// tldraw HandTool: "Moves the camera" — pan changes offset.
    #[test]
    fn pan_changes_offset() {
        let mut cam = Camera::default();
        cam.pan(25.0, 25.0);
        assert!((cam.offset_x - 25.0).abs() < 1e-6);
        assert!((cam.offset_y - 25.0).abs() < 1e-6);

        cam.pan(25.0, 25.0);
        assert!((cam.offset_x - 50.0).abs() < 1e-6);
        assert!((cam.offset_y - 50.0).abs() < 1e-6);
    }

    /// tldraw HandTool: zoom in increases zoom factor.
    #[test]
    fn zoom_in_increases_factor() {
        let mut cam = Camera::default();
        let original_zoom = cam.zoom;
        cam.zoom_at(500.0, 400.0, 1.5);
        assert!(cam.zoom > original_zoom, "zoom should increase");
    }

    /// tldraw HandTool: zoom out decreases zoom factor.
    #[test]
    fn zoom_out_decreases_factor() {
        let mut cam = Camera::default();
        let original_zoom = cam.zoom;
        cam.zoom_at(500.0, 400.0, 0.5);
        assert!(cam.zoom < original_zoom, "zoom should decrease");
    }

    /// tldraw: zoom at cursor position keeps cursor world position fixed.
    #[test]
    fn zoom_at_cursor_preserves_world_position() {
        let mut cam = Camera::default();
        let cursor_sx = 300.0;
        let cursor_sy = 200.0;

        // World position of cursor before zoom
        let (wx_before, wy_before) = cam.screen_to_world(cursor_sx, cursor_sy);

        cam.zoom_at(cursor_sx, cursor_sy, 2.0);

        // World position of cursor after zoom should be the same
        let (wx_after, wy_after) = cam.screen_to_world(cursor_sx, cursor_sy);

        assert!(
            (wx_before - wx_after).abs() < 1e-6,
            "world x under cursor should be stable: before={wx_before}, after={wx_after}"
        );
        assert!(
            (wy_before - wy_after).abs() < 1e-6,
            "world y under cursor should be stable: before={wy_before}, after={wy_after}"
        );
    }

    /// Camera zoom is clamped between 0.1 and 10.0.
    #[test]
    fn zoom_is_clamped() {
        let mut cam = Camera::default();

        // Zoom way in
        for _ in 0..50 {
            cam.zoom_at(0.0, 0.0, 1.5);
        }
        assert!(cam.zoom <= 10.0, "zoom should be clamped at 10.0");

        // Zoom way out
        for _ in 0..100 {
            cam.zoom_at(0.0, 0.0, 0.5);
        }
        assert!(cam.zoom >= 0.1, "zoom should be clamped at 0.1");
    }

    /// Screen-to-world conversion with non-default camera.
    #[test]
    fn screen_to_world_with_offset_and_zoom() {
        let cam = Camera {
            offset_x: 100.0,
            offset_y: 50.0,
            zoom: 2.0,
        };

        let (wx, wy) = cam.screen_to_world(200.0, 150.0);
        // wx = (200 - 100) / 2 = 50
        // wy = (150 - 50) / 2 = 50
        assert!((wx - 50.0).abs() < 1e-6, "wx should be 50, got {wx}");
        assert!((wy - 50.0).abs() < 1e-6, "wy should be 50, got {wy}");
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Translating (drag to move)
// (from tldraw SelectTool.test.ts — TLSelectTool.Translating)
// ═══════════════════════════════════════════════════════════════════════

mod translating {
    use super::*;
    use capy_canvas_core::state::Tool;

    /// tldraw: "Drags a shape" — drag from (150,150) with shape at (100,100) 100x100
    /// moves shape by delta.
    #[test]
    fn drag_moves_shape() {
        let mut state = AppState::new();
        state.tool = Tool::Select;
        add_rect(&mut state, 100.0, 100.0, 100.0, 100.0);

        // Click inside shape to select + start moving
        state.cursor_x = 150.0;
        state.cursor_y = 150.0;
        handle_mouse_button(&mut state, MouseButton::Left, true, false);

        // Drag to (200, 200) — delta is (50, 50)
        handle_mouse_move(&mut state, 200.0, 200.0);
        handle_mouse_button(&mut state, MouseButton::Left, false, false);

        assert!(
            (state.shapes[0].x - 150.0).abs() < 1e-6,
            "shape x should be 150"
        );
        assert!(
            (state.shapes[0].y - 150.0).abs() < 1e-6,
            "shape y should be 150"
        );
    }

    /// tldraw: "Enters from pointing and exits to idle" — after drag, drag_mode
    /// returns to None.
    #[test]
    fn drag_mode_returns_to_none_after_release() {
        let mut state = AppState::new();
        state.tool = Tool::Select;
        add_rect(&mut state, 100.0, 100.0, 100.0, 100.0);

        state.cursor_x = 150.0;
        state.cursor_y = 150.0;
        handle_mouse_button(&mut state, MouseButton::Left, true, false);
        assert!(matches!(state.drag_mode, DragMode::Moving { .. }));

        handle_mouse_move(&mut state, 200.0, 200.0);
        handle_mouse_button(&mut state, MouseButton::Left, false, false);
        assert_eq!(
            state.drag_mode,
            DragMode::None,
            "drag mode should be None after release"
        );
    }

    /// tldraw: moving multiple selected shapes preserves relative positions.
    #[test]
    fn drag_multiple_shapes_preserves_relative_positions() {
        let mut state = AppState::new();
        state.tool = Tool::Select;
        add_rect(&mut state, 0.0, 0.0, 50.0, 50.0);
        add_rect(&mut state, 100.0, 100.0, 50.0, 50.0);

        // Select both
        state.selected = vec![0, 1];

        // Start drag from inside shape 0
        state.cursor_x = 25.0;
        state.cursor_y = 25.0;
        state.drag_start_sx = 25.0;
        state.drag_start_sy = 25.0;
        state.drag_shape_origins = vec![(0.0, 0.0), (100.0, 100.0)];
        state.drag_mode = DragMode::Moving {
            start_wx: 25.0,
            start_wy: 25.0,
        };
        state.push_undo();

        // Move by (50, 50)
        handle_mouse_move(&mut state, 75.0, 75.0);
        handle_mouse_button(&mut state, MouseButton::Left, false, false);

        // Both should have moved by same delta
        let dx_a = state.shapes[0].x;
        let dy_a = state.shapes[0].y;
        let dx_b = state.shapes[1].x - 100.0;
        let dy_b = state.shapes[1].y - 100.0;

        assert!(
            (dx_a - dx_b).abs() < 1e-6 && (dy_a - dy_b).abs() < 1e-6,
            "both shapes should move by same delta"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Connector binding
// (from excalidraw binding.test.tsx)
// ═══════════════════════════════════════════════════════════════════════

mod connector {
    use super::*;
    use capy_canvas_core::state::Tool;

    /// Excalidraw-style: Arrow tool drag near shapes creates a bound arrow shape.
    #[test]
    fn arrow_tool_binds_two_shapes() {
        let mut state = AppState::new();
        state.tool = Tool::Arrow;
        // Place shapes below toolbar area (toolbar occupies y=10..58)
        let _a = add_rect(&mut state, 0.0, 100.0, 100.0, 100.0);
        let _b = add_rect(&mut state, 300.0, 100.0, 100.0, 100.0);

        // Drag from shape A (center at 50, 150) to shape B (center at 350, 150)
        drag(&mut state, 50.0, 150.0, 350.0, 150.0, false);
        // A bound arrow shape should be created (the last shape)
        let arrow_idx = state.shapes.len() - 1;
        let arrow = &state.shapes[arrow_idx];
        assert_eq!(arrow.kind, ShapeKind::Arrow);
        assert_eq!(arrow.binding_start, Some(state.shapes[0].id));
        assert_eq!(arrow.binding_end, Some(state.shapes[1].id));
        assert!(
            state.connector_from.is_none(),
            "connector_from should be cleared after drag"
        );
        assert!(
            state.connector_preview.is_none(),
            "connector_preview should be cleared after drag"
        );
    }

    /// excalidraw: connector is removed when a bound shape is deleted.
    #[test]
    fn deleting_shape_removes_connected_connectors() {
        let mut state = AppState::new();
        state.tool = Tool::Select;
        let _a = add_rect(&mut state, 0.0, 0.0, 100.0, 100.0);
        let _b = add_rect(&mut state, 300.0, 0.0, 100.0, 100.0);
        state.connectors.push(capy_canvas_core::state::Connector {
            from_id: state.shapes[0].id,
            to_id: state.shapes[1].id,
            color: 0x1e1e1e,
            style: capy_canvas_core::state::ConnectorStyle::default(),
            label: None,
        });

        // Delete shape A
        state.selected = vec![0];
        state.delete_selected();

        assert!(
            state.connectors.is_empty(),
            "connector should be removed when bound shape is deleted"
        );
    }

    /// Shape.edge_point returns a point on the boundary heading toward target.
    #[test]
    fn edge_point_returns_boundary_point() {
        let shape = Shape::new(ShapeKind::Rect, 0.0, 0.0, 0x1e1e1e);
        let mut s = shape;
        s.w = 100.0;
        s.h = 100.0;

        // Center is (50, 50). Target to the right at (200, 50) -> edge should be
        // at (100, 50).
        let (ex, ey) = s.edge_point(200.0, 50.0);
        assert!((ex - 100.0).abs() < 1e-6, "edge x should be 100, got {ex}");
        assert!((ey - 50.0).abs() < 1e-6, "edge y should be 50, got {ey}");
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Alignment guides
// (inspired by tldraw grid-align-on-create.test.ts)
// ═══════════════════════════════════════════════════════════════════════

mod alignment {
    use super::*;

    /// Alignment guides detect vertical alignment within threshold.
    #[test]
    fn alignment_guides_detect_vertical_alignment() {
        let mut state = AppState::new();
        // Static shape at x=100
        add_rect(&mut state, 100.0, 0.0, 50.0, 50.0);
        // Dragged shape at x=103 (within 5px threshold of 100)
        add_rect(&mut state, 103.0, 100.0, 50.0, 50.0);

        let guides = state.alignment_guides(&[1]);
        let has_vertical = guides.iter().any(|g| matches!(g, capy_canvas_core::state::AlignGuide::Vertical(x) if (*x - 100.0).abs() < 1e-6));
        assert!(has_vertical, "should detect vertical alignment at x=100");
    }

    /// Alignment guides detect horizontal alignment within threshold.
    #[test]
    fn alignment_guides_detect_horizontal_alignment() {
        let mut state = AppState::new();
        // Static shape at y=200
        add_rect(&mut state, 0.0, 200.0, 50.0, 50.0);
        // Dragged shape at y=202 (within 5px threshold of 200)
        add_rect(&mut state, 100.0, 202.0, 50.0, 50.0);

        let guides = state.alignment_guides(&[1]);
        let has_horizontal = guides.iter().any(|g| matches!(g, capy_canvas_core::state::AlignGuide::Horizontal(y) if (*y - 200.0).abs() < 1e-6));
        assert!(
            has_horizontal,
            "should detect horizontal alignment at y=200"
        );
    }

    /// No guides when shapes are far apart.
    #[test]
    fn no_alignment_guides_when_far_apart() {
        let mut state = AppState::new();
        add_rect(&mut state, 0.0, 0.0, 50.0, 50.0);
        add_rect(&mut state, 500.0, 500.0, 50.0, 50.0);

        let guides = state.alignment_guides(&[1]);
        assert!(guides.is_empty(), "no guides when shapes are far apart");
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Alignment & Distribution tests
// ═══════════════════════════════════════════════════════════════════════
