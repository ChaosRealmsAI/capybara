mod common;
use common::*;

mod selection {
    use super::*;
    use capy_canvas_core::state::Tool;

    /// tldraw: "Transitions to pointing_shape on shape pointer down" — clicking
    /// on a shape should select it.
    #[test]
    fn click_on_shape_selects_it() {
        let mut state = AppState::new();
        state.tool = Tool::Select;
        let _idx = add_rect(&mut state, 100.0, 100.0, 100.0, 100.0);

        // Click inside the shape (world coords = screen coords at zoom 1, offset 0)
        click_at(&mut state, 150.0, 150.0, false);
        assert_eq!(
            state.selected,
            vec![0],
            "clicking on shape should select it"
        );
    }

    /// tldraw: "Transitions to pointing_canvas on canvas pointer down" — clicking
    /// on empty canvas should deselect all.
    #[test]
    fn click_on_empty_deselects_all() {
        let mut state = AppState::new();
        state.tool = Tool::Select;
        let idx = add_rect(&mut state, 100.0, 100.0, 100.0, 100.0);
        state.selected = vec![idx];

        // Click outside the shape
        click_at(&mut state, 10.0, 10.0, false);
        assert!(
            state.selected.is_empty(),
            "clicking empty canvas should deselect all"
        );
    }

    /// tldraw: "Selects on shift+pointer up" — shift+click adds to selection.
    #[test]
    fn shift_click_adds_to_selection() {
        let mut state = AppState::new();
        state.tool = Tool::Select;
        let _a = add_rect(&mut state, 0.0, 0.0, 50.0, 50.0);
        let _b = add_rect(&mut state, 100.0, 100.0, 50.0, 50.0);

        // Select first shape
        click_at(&mut state, 25.0, 25.0, false);
        assert_eq!(state.selected, vec![0]);

        // Shift+click second shape
        click_at(&mut state, 125.0, 125.0, true);
        assert_eq!(
            state.selected.len(),
            2,
            "shift+click should add to selection"
        );
        assert!(state.selected.contains(&0));
        assert!(state.selected.contains(&1));
    }

    /// tldraw: clicking already-selected shape keeps it selected.
    #[test]
    fn click_selected_shape_stays_selected() {
        let mut state = AppState::new();
        state.tool = Tool::Select;
        let idx = add_rect(&mut state, 100.0, 100.0, 100.0, 100.0);
        state.selected = vec![idx];

        click_at(&mut state, 150.0, 150.0, false);
        assert_eq!(
            state.selected,
            vec![idx],
            "clicking selected shape should keep it selected"
        );
    }

    /// tldraw: clicking different shape deselects previous, selects new.
    #[test]
    fn click_different_shape_switches_selection() {
        let mut state = AppState::new();
        state.tool = Tool::Select;
        let _a = add_rect(&mut state, 0.0, 0.0, 50.0, 50.0);
        let _b = add_rect(&mut state, 200.0, 200.0, 50.0, 50.0);
        state.selected = vec![0];

        click_at(&mut state, 225.0, 225.0, false);
        assert_eq!(
            state.selected,
            vec![1],
            "clicking different shape should switch selection"
        );
    }

    /// tldraw: shift+click on selected shape removes it from selection.
    #[test]
    fn shift_click_deselects_from_multi_selection() {
        let mut state = AppState::new();
        state.tool = Tool::Select;
        let _a = add_rect(&mut state, 0.0, 0.0, 50.0, 50.0);
        let _b = add_rect(&mut state, 100.0, 100.0, 50.0, 50.0);
        state.selected = vec![0, 1];

        // Shift+click shape B to deselect it
        click_at(&mut state, 125.0, 125.0, true);
        assert_eq!(
            state.selected,
            vec![0],
            "shift+click should remove from selection"
        );
    }

    /// excalidraw selection.test.ts: no change returns same selection reference.
    /// We test that selecting the same set is stable.
    #[test]
    fn empty_selection_stays_empty() {
        let mut state = AppState::new();
        state.tool = Tool::Select;
        assert!(state.selected.is_empty());

        // Click on empty
        click_at(&mut state, 500.0, 500.0, false);
        assert!(state.selected.is_empty());
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Bounds / Hit test
// (from excalidraw bounds.test.ts, tldraw SelectTool.test.ts pointer move)
// ═══════════════════════════════════════════════════════════════════════

mod bounds_hit_test {
    use super::*;

    /// excalidraw: point inside rect -> hit.
    #[test]
    fn point_inside_rect_is_hit() {
        let mut state = AppState::new();
        add_rect(&mut state, 10.0, 20.0, 100.0, 80.0);
        let hit = state.hit_test(50.0, 50.0);
        assert_eq!(hit, Some(0), "point inside rect should be a hit");
    }

    /// excalidraw: point outside rect -> miss.
    #[test]
    fn point_outside_rect_is_miss() {
        let mut state = AppState::new();
        add_rect(&mut state, 10.0, 20.0, 100.0, 80.0);
        let hit = state.hit_test(200.0, 200.0);
        assert_eq!(hit, None, "point outside rect should be a miss");
    }

    /// excalidraw: point on rect edge -> hit.
    #[test]
    fn point_on_rect_edge_is_hit() {
        let mut state = AppState::new();
        add_rect(&mut state, 10.0, 20.0, 100.0, 80.0);
        // Exactly on left edge
        let hit = state.hit_test(10.0, 50.0);
        assert_eq!(hit, Some(0), "point on rect edge should be a hit");
        // Exactly on right edge
        let hit = state.hit_test(110.0, 50.0);
        assert_eq!(hit, Some(0), "point on right edge should be a hit");
        // Exactly on top edge
        let hit = state.hit_test(50.0, 20.0);
        assert_eq!(hit, Some(0), "point on top edge should be a hit");
    }

    /// excalidraw bounds.test.ts: x1 coordinate of element (absolute coords).
    #[test]
    fn rect_bounds_x1() {
        let mut state = AppState::new();
        add_rect(&mut state, 10.0, 20.0, 100.0, 0.0);
        // x1 should equal shape.x
        assert!((state.shapes[0].x - 10.0).abs() < 1e-6);
    }

    /// excalidraw bounds.test.ts: x2 coordinate equals x + w.
    #[test]
    fn rect_bounds_x2() {
        let mut state = AppState::new();
        add_rect(&mut state, 10.0, 20.0, 100.0, 50.0);
        let x2 = state.shapes[0].x + state.shapes[0].w;
        assert!((x2 - 110.0).abs() < 1e-6);
    }

    /// excalidraw bounds.test.ts: y1 coordinate.
    #[test]
    fn rect_bounds_y1() {
        let mut state = AppState::new();
        add_rect(&mut state, 0.0, 10.0, 0.0, 100.0);
        assert!((state.shapes[0].y - 10.0).abs() < 1e-6);
    }

    /// excalidraw bounds.test.ts: y2 coordinate equals y + h.
    #[test]
    fn rect_bounds_y2() {
        let mut state = AppState::new();
        add_rect(&mut state, 0.0, 10.0, 0.0, 100.0);
        let y2 = state.shapes[0].y + state.shapes[0].h;
        assert!((y2 - 110.0).abs() < 1e-6);
    }

    /// excalidraw: point inside ellipse -> hit.
    #[test]
    fn point_inside_ellipse_is_hit() {
        let mut state = AppState::new();
        add_ellipse(&mut state, 0.0, 0.0, 100.0, 80.0);
        // Center of ellipse
        let hit = state.hit_test(50.0, 40.0);
        assert_eq!(hit, Some(0), "center of ellipse should be a hit");
    }

    /// excalidraw: point outside ellipse -> miss.
    #[test]
    fn point_outside_ellipse_is_miss() {
        let mut state = AppState::new();
        add_ellipse(&mut state, 0.0, 0.0, 100.0, 80.0);
        // Corner of bounding box (outside ellipse curve)
        let hit = state.hit_test(2.0, 2.0);
        assert_eq!(hit, None, "corner of ellipse bbox should be a miss");
    }

    /// excalidraw: point near line -> hit (within 5px threshold).
    #[test]
    fn point_near_line_is_hit() {
        let mut state = AppState::new();
        add_line(&mut state, 0.0, 0.0, 100.0, 100.0);
        // Point slightly off the diagonal line (within 5px)
        let hit = state.hit_test(50.0, 53.0);
        assert_eq!(hit, Some(0), "point within 5px of line should be a hit");
    }

    /// excalidraw: point far from line -> miss.
    #[test]
    fn point_far_from_line_is_miss() {
        let mut state = AppState::new();
        add_line(&mut state, 0.0, 0.0, 100.0, 100.0);
        // Point 20px off the diagonal
        let hit = state.hit_test(50.0, 70.0);
        assert_eq!(hit, None, "point >5px from line should be a miss");
    }

    /// Hit test returns topmost shape (highest index) when shapes overlap,
    /// mimicking excalidraw z-index behavior.
    #[test]
    fn hit_test_returns_topmost_shape() {
        let mut state = AppState::new();
        add_rect(&mut state, 0.0, 0.0, 100.0, 100.0);
        add_rect(&mut state, 50.0, 50.0, 100.0, 100.0);
        let hit = state.hit_test(75.0, 75.0);
        assert_eq!(
            hit,
            Some(1),
            "hit_test should return topmost (last) overlapping shape"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Drag create
// (from excalidraw dragCreate.test.tsx)
// ═══════════════════════════════════════════════════════════════════════
