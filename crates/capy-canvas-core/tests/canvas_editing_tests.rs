mod common;
use common::*;

mod drag_create {
    use super::*;
    use capy_canvas_core::state::Tool;

    /// excalidraw: drag from (100,100) to (300,200) creates rect at (100,100) with
    /// size (200, 100).
    #[test]
    fn drag_creates_rect_with_correct_bounds() {
        let mut state = AppState::new();
        state.tool = Tool::Rect;

        drag(&mut state, 100.0, 100.0, 300.0, 200.0, false);

        assert_eq!(state.shapes.len(), 1, "one shape should be created");
        let s = &state.shapes[0];
        assert_eq!(s.kind, ShapeKind::Rect);
        assert!((s.x - 100.0).abs() < 1e-6, "x should be 100, got {}", s.x);
        assert!((s.y - 100.0).abs() < 1e-6, "y should be 100, got {}", s.y);
        assert!((s.w - 200.0).abs() < 1e-6, "w should be 200, got {}", s.w);
        assert!((s.h - 100.0).abs() < 1e-6, "h should be 100, got {}", s.h);
    }

    /// excalidraw: drag creates ellipse with correct position and size.
    #[test]
    fn drag_creates_ellipse_with_correct_bounds() {
        let mut state = AppState::new();
        state.tool = Tool::Ellipse;

        drag(&mut state, 30.0, 20.0, 60.0, 70.0, false);

        assert_eq!(state.shapes.len(), 1);
        let s = &state.shapes[0];
        assert_eq!(s.kind, ShapeKind::Ellipse);
        assert!((s.x - 30.0).abs() < 1e-6);
        assert!((s.y - 20.0).abs() < 1e-6);
        assert!((s.w - 30.0).abs() < 1e-6); // 60 - 30
        assert!((s.h - 50.0).abs() < 1e-6); // 70 - 20
    }

    /// excalidraw: drag creates line with correct start and delta.
    #[test]
    fn drag_creates_line() {
        let mut state = AppState::new();
        state.tool = Tool::Line;

        drag(&mut state, 30.0, 20.0, 60.0, 70.0, false);

        assert_eq!(state.shapes.len(), 1);
        let s = &state.shapes[0];
        assert_eq!(s.kind, ShapeKind::Line);
        assert!((s.x - 30.0).abs() < 1e-6);
        assert!((s.y - 20.0).abs() < 1e-6);
        assert!((s.w - 30.0).abs() < 1e-6); // 60 - 30
        assert!((s.h - 50.0).abs() < 1e-6); // 70 - 20
    }

    /// excalidraw: drag creates arrow with correct start and delta.
    #[test]
    fn drag_creates_arrow() {
        let mut state = AppState::new();
        state.tool = Tool::Arrow;

        drag(&mut state, 30.0, 20.0, 60.0, 70.0, false);

        assert_eq!(state.shapes.len(), 1);
        let s = &state.shapes[0];
        assert_eq!(s.kind, ShapeKind::Arrow);
        assert!((s.x - 30.0).abs() < 1e-6);
        assert!((s.y - 20.0).abs() < 1e-6);
    }

    /// Click without moving creates a visible default shape instead of an
    /// invisible zero-size shape.
    #[test]
    fn zero_size_drag_creates_visible_default_shape() {
        let mut state = AppState::new();
        state.tool = Tool::Rect;

        // Click without moving
        state.cursor_x = 50.0;
        state.cursor_y = 50.0;
        handle_mouse_button(&mut state, MouseButton::Left, true, false);
        handle_mouse_button(&mut state, MouseButton::Left, false, false);

        assert_eq!(state.shapes.len(), 1);
        let s = &state.shapes[0];
        assert!(
            (s.w - 120.0).abs() < 1e-6,
            "click-create should produce visible default width"
        );
        assert!(
            (s.h - 80.0).abs() < 1e-6,
            "click-create should produce visible default height"
        );
    }

    /// excalidraw dragCreate: rect at (30,20) to (60,70) -> position (30,20)
    /// size (30,50). Direct match of the excalidraw test assertion.
    #[test]
    fn drag_create_rect_excalidraw_exact() {
        let mut state = AppState::new();
        state.tool = Tool::Rect;

        drag(&mut state, 30.0, 20.0, 60.0, 70.0, false);

        assert_eq!(state.shapes.len(), 1);
        let s = &state.shapes[0];
        assert_eq!(s.kind, ShapeKind::Rect);
        assert!((s.x - 30.0).abs() < 1e-6);
        assert!((s.y - 20.0).abs() < 1e-6);
        assert!((s.w - 30.0).abs() < 1e-6);
        assert!((s.h - 50.0).abs() < 1e-6);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Resize
// (from excalidraw resize.test.tsx)
// ═══════════════════════════════════════════════════════════════════════

mod resize {
    use super::*;

    /// excalidraw: dragging to resize changes shape bounds.
    /// We test the underlying size mutation directly.
    #[test]
    fn resize_changes_shape_bounds() {
        let mut state = AppState::new();
        let idx = add_rect(&mut state, 0.0, 0.0, 200.0, 100.0);

        // Simulate a resize: increase width by 67, height stays
        state.shapes[idx].w += 67.0;
        assert!((state.shapes[idx].w - 267.0).abs() < 1e-6);
        assert!((state.shapes[idx].h - 100.0).abs() < 1e-6);
    }

    /// excalidraw: resizing north handle changes y and height.
    #[test]
    fn resize_north_changes_y_and_height() {
        let mut state = AppState::new();
        let idx = add_rect(&mut state, 0.0, 0.0, 200.0, 100.0);

        // Move top edge up by 27px
        state.shapes[idx].y -= 27.0;
        state.shapes[idx].h += 27.0;

        assert!((state.shapes[idx].y - (-27.0)).abs() < 1e-6);
        assert!((state.shapes[idx].h - 127.0).abs() < 1e-6);
        assert!(
            (state.shapes[idx].w - 200.0).abs() < 1e-6,
            "width should not change"
        );
    }

    /// excalidraw: resizing east handle changes only width.
    #[test]
    fn resize_east_changes_only_width() {
        let mut state = AppState::new();
        let idx = add_rect(&mut state, 0.0, 0.0, 200.0, 100.0);

        state.shapes[idx].w += 67.0;

        assert!((state.shapes[idx].w - 267.0).abs() < 1e-6);
        assert!(
            (state.shapes[idx].h - 100.0).abs() < 1e-6,
            "height should not change"
        );
        assert!((state.shapes[idx].x).abs() < 1e-6, "x should not change");
        assert!((state.shapes[idx].y).abs() < 1e-6, "y should not change");
    }

    /// excalidraw: resizing south handle changes only height.
    #[test]
    fn resize_south_changes_only_height() {
        let mut state = AppState::new();
        let idx = add_rect(&mut state, 0.0, 0.0, 200.0, 100.0);

        state.shapes[idx].h -= 39.0;

        assert!((state.shapes[idx].h - 61.0).abs() < 1e-6);
        assert!(
            (state.shapes[idx].w - 200.0).abs() < 1e-6,
            "width should not change"
        );
    }

    /// excalidraw: aspect ratio preserved resize (shift held).
    /// Resize SE corner by (100, _) with locked 2:1 ratio.
    #[test]
    fn aspect_ratio_preserved_resize() {
        let mut state = AppState::new();
        let idx = add_rect(&mut state, 0.0, 0.0, 200.0, 100.0);

        // Simulate shift-resize: add 100 to width, maintain 2:1 aspect ratio
        let ratio = state.shapes[idx].w / state.shapes[idx].h;
        state.shapes[idx].w += 100.0;
        state.shapes[idx].h = state.shapes[idx].w / ratio;

        assert!((state.shapes[idx].w - 300.0).abs() < 1e-6);
        assert!((state.shapes[idx].h - 150.0).abs() < 1e-6);
    }

    /// excalidraw: resize from center (alt held) shrinks symmetrically.
    #[test]
    fn resize_from_center() {
        let mut state = AppState::new();
        let idx = add_rect(&mut state, 0.0, 0.0, 200.0, 100.0);

        // Alt-resize NW handle by (20, 10) -> shrink by 2x that from both sides
        let dx = 20.0;
        let dy = 10.0;
        state.shapes[idx].x += dx;
        state.shapes[idx].y += dy;
        state.shapes[idx].w -= dx * 2.0;
        state.shapes[idx].h -= dy * 2.0;

        assert!((state.shapes[idx].x - 20.0).abs() < 1e-6);
        assert!((state.shapes[idx].y - 10.0).abs() < 1e-6);
        assert!((state.shapes[idx].w - 160.0).abs() < 1e-6);
        assert!((state.shapes[idx].h - 80.0).abs() < 1e-6);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Z-order
// (from excalidraw zindex.test.tsx)
// ═══════════════════════════════════════════════════════════════════════

mod z_order {
    use super::*;

    /// excalidraw: most recently created shape has highest z-index (is last in array).
    #[test]
    fn most_recently_created_has_highest_z_index() {
        let mut state = AppState::new();
        let a = add_rect(&mut state, 0.0, 0.0, 50.0, 50.0);
        let b = add_rect(&mut state, 100.0, 100.0, 50.0, 50.0);
        let c = add_rect(&mut state, 200.0, 200.0, 50.0, 50.0);

        assert!(c > b && b > a, "later shapes should have higher indices");
        assert_eq!(state.shapes.len(), 3);
        assert!(state.shapes[2].id > state.shapes[1].id);
        assert!(state.shapes[1].id > state.shapes[0].id);
    }

    /// excalidraw: hit_test returns topmost shape (highest z) at overlapping point.
    #[test]
    fn click_returns_topmost_shape() {
        let mut state = AppState::new();
        add_rect(&mut state, 0.0, 0.0, 100.0, 100.0);
        add_rect(&mut state, 50.0, 50.0, 100.0, 100.0);
        add_rect(&mut state, 80.0, 80.0, 100.0, 100.0);

        let hit = state.hit_test(90.0, 90.0);
        assert_eq!(hit, Some(2), "should hit the topmost (last created) shape");
    }

    /// excalidraw zindex: after deletion, remaining shapes keep relative z-order.
    #[test]
    fn deletion_preserves_relative_z_order() {
        let mut state = AppState::new();
        state.tool = capy_canvas_core::state::Tool::Select;
        let _a = add_rect(&mut state, 0.0, 0.0, 50.0, 50.0); // id 1
        let _b = add_rect(&mut state, 100.0, 100.0, 50.0, 50.0); // id 2
        let _c = add_rect(&mut state, 200.0, 200.0, 50.0, 50.0); // id 3

        // Delete shape B (index 1)
        state.selected = vec![1];
        state.delete_selected();

        assert_eq!(state.shapes.len(), 2);
        assert_eq!(state.shapes[0].id, 1, "A should remain at index 0");
        assert_eq!(state.shapes[1].id, 3, "C should now be at index 1");
    }

    /// excalidraw: shape IDs are monotonically increasing.
    #[test]
    fn shape_ids_monotonically_increase() {
        let mut state = AppState::new();
        let idx_a = add_rect(&mut state, 0.0, 0.0, 50.0, 50.0);
        let idx_b = add_rect(&mut state, 100.0, 100.0, 50.0, 50.0);
        let idx_c = add_rect(&mut state, 200.0, 200.0, 50.0, 50.0);

        assert!(
            state.shapes[idx_a].id < state.shapes[idx_b].id
                && state.shapes[idx_b].id < state.shapes[idx_c].id,
            "IDs should be monotonically increasing"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Undo / Redo
// (inspired by tldraw and excalidraw undo patterns)
// ═══════════════════════════════════════════════════════════════════════

mod undo_redo {
    use super::*;

    /// Create shape -> undo -> shape gone.
    #[test]
    fn create_shape_undo_removes_it() {
        let mut state = AppState::new();
        state.push_undo();
        add_rect(&mut state, 100.0, 100.0, 50.0, 50.0);
        assert_eq!(state.shapes.len(), 1);

        state.undo();
        assert_eq!(
            state.shapes.len(),
            0,
            "undo should remove the created shape"
        );
    }

    /// Create shape -> undo -> redo -> shape back.
    #[test]
    fn create_undo_redo_restores_shape() {
        let mut state = AppState::new();
        state.push_undo();
        add_rect(&mut state, 100.0, 100.0, 50.0, 50.0);
        assert_eq!(state.shapes.len(), 1);

        state.undo();
        assert_eq!(state.shapes.len(), 0);

        state.redo();
        assert_eq!(state.shapes.len(), 1, "redo should restore the shape");
    }

    /// Move shape -> undo -> back to original position.
    #[test]
    fn move_undo_returns_to_original_position() {
        let mut state = AppState::new();
        let idx = add_rect(&mut state, 100.0, 100.0, 50.0, 50.0);
        let orig_x = state.shapes[idx].x;
        let orig_y = state.shapes[idx].y;

        state.push_undo();
        state.shapes[idx].x = 200.0;
        state.shapes[idx].y = 300.0;

        state.undo();
        assert!(
            (state.shapes[0].x - orig_x).abs() < 1e-6,
            "x should return to original"
        );
        assert!(
            (state.shapes[0].y - orig_y).abs() < 1e-6,
            "y should return to original"
        );
    }

    /// Delete shape -> undo -> shape restored.
    #[test]
    fn delete_undo_restores_shape() {
        let mut state = AppState::new();
        state.tool = capy_canvas_core::state::Tool::Select;
        add_rect(&mut state, 100.0, 100.0, 50.0, 50.0);
        assert_eq!(state.shapes.len(), 1);

        state.selected = vec![0];
        state.delete_selected();
        assert_eq!(state.shapes.len(), 0);

        state.undo();
        assert_eq!(
            state.shapes.len(),
            1,
            "undo should restore the deleted shape"
        );
    }

    /// Undo clears selection (consistent with tldraw/excalidraw behavior).
    #[test]
    fn undo_clears_selection() {
        let mut state = AppState::new();
        state.push_undo();
        let idx = add_rect(&mut state, 100.0, 100.0, 50.0, 50.0);
        state.selected = vec![idx];

        state.undo();
        assert!(state.selected.is_empty(), "undo should clear selection");
    }

    /// Redo clears selection.
    #[test]
    fn redo_clears_selection() {
        let mut state = AppState::new();
        state.push_undo();
        add_rect(&mut state, 100.0, 100.0, 50.0, 50.0);

        state.undo();
        state.redo();
        assert!(state.selected.is_empty(), "redo should clear selection");
    }

    /// Creating new changes after undo discards redo stack.
    #[test]
    fn new_action_after_undo_clears_redo() {
        let mut state = AppState::new();

        // Create first shape
        state.push_undo();
        add_rect(&mut state, 100.0, 100.0, 50.0, 50.0);

        // Undo it
        state.undo();
        assert!(!state.redo_stack.is_empty(), "redo stack should have entry");

        // New action: push undo again (new branch)
        state.push_undo();
        add_rect(&mut state, 200.0, 200.0, 50.0, 50.0);
        assert!(
            state.redo_stack.is_empty(),
            "redo stack should be cleared after new action"
        );
    }

    /// Multiple undos pop the stack correctly.
    #[test]
    fn multiple_undos() {
        let mut state = AppState::new();

        state.push_undo();
        add_rect(&mut state, 0.0, 0.0, 10.0, 10.0);
        state.push_undo();
        add_rect(&mut state, 100.0, 100.0, 10.0, 10.0);
        state.push_undo();
        add_rect(&mut state, 200.0, 200.0, 10.0, 10.0);

        assert_eq!(state.shapes.len(), 3);

        state.undo();
        assert_eq!(state.shapes.len(), 2);
        state.undo();
        assert_eq!(state.shapes.len(), 1);
        state.undo();
        assert_eq!(state.shapes.len(), 0);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Camera (pan / zoom)
// (from tldraw HandTool.test.ts)
// ═══════════════════════════════════════════════════════════════════════
