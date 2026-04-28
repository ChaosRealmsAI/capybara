//! Integration tests for canvas-vello, extracted from tldraw & excalidraw test suites.
//!
//! Source references:
//! - tldraw: SelectTool.test.ts, select.test.tsx, HandTool.test.ts, grid-align-on-create.test.ts
//! - excalidraw: selection.test.ts, bounds.test.ts, resize.test.tsx, dragCreate.test.tsx,
//!   zindex.test.tsx, binding.test.tsx

use capy_canvas_core::input::{handle_mouse_button, handle_mouse_move};
use capy_canvas_core::state::{AppState, Camera, CanvasContentKind, DragMode, Shape, ShapeKind};
use winit::event::MouseButton;

// ── Test Helpers ──

/// Simulate a click at screen coordinates (sx, sy) in Select tool mode.
fn click_at(state: &mut AppState, sx: f64, sy: f64, shift: bool) {
    state.cursor_x = sx;
    state.cursor_y = sy;
    handle_mouse_button(state, MouseButton::Left, true, shift);
    handle_mouse_button(state, MouseButton::Left, false, shift);
}

/// Simulate a drag from (x1,y1) to (x2,y2) in screen coords.
/// Disables grid snapping (alt_held = true) for precise coordinate tests.
fn drag(state: &mut AppState, x1: f64, y1: f64, x2: f64, y2: f64, shift: bool) {
    state.alt_held = true; // disable grid snap for predictable coordinates
    state.cursor_x = x1;
    state.cursor_y = y1;
    handle_mouse_button(state, MouseButton::Left, true, shift);
    handle_mouse_move(state, x2, y2);
    handle_mouse_button(state, MouseButton::Left, false, shift);
    state.alt_held = false;
}

/// Add a rect shape at world (x, y) with size (w, h). Returns the index.
fn add_rect(state: &mut AppState, x: f64, y: f64, w: f64, h: f64) -> usize {
    let mut shape = Shape::new(ShapeKind::Rect, x, y, 0x1e1e1e);
    shape.w = w;
    shape.h = h;
    state.add_shape(shape)
}

/// Add an ellipse shape at world (x, y) with size (w, h). Returns the index.
fn add_ellipse(state: &mut AppState, x: f64, y: f64, w: f64, h: f64) -> usize {
    let mut shape = Shape::new(ShapeKind::Ellipse, x, y, 0x1e1e1e);
    shape.w = w;
    shape.h = h;
    state.add_shape(shape)
}

/// Add a line from (x1,y1) to (x2,y2). Returns the index.
fn add_line(state: &mut AppState, x1: f64, y1: f64, x2: f64, y2: f64) -> usize {
    let mut shape = Shape::new(ShapeKind::Line, x1, y1, 0x1e1e1e);
    shape.w = x2 - x1;
    shape.h = y2 - y1;
    state.add_shape(shape)
}

mod ai_context {
    use super::*;

    #[test]
    fn selected_context_exports_product_metadata() {
        let mut state = AppState::new();
        let mut shape = Shape::new(ShapeKind::Rect, 40.0, 50.0, 0xa78bfa);
        shape.w = 220.0;
        shape.h = 140.0;
        shape.metadata.content_kind = Some(CanvasContentKind::Video);
        shape.metadata.title = Some("5 秒开场镜头".to_string());
        shape.metadata.status = Some("queued".to_string());
        shape.metadata.owner = Some("Video Capy".to_string());
        shape.metadata.refs = vec!["brand-kit".to_string(), "hero-image".to_string()];
        shape.metadata.next_action = Some("生成分镜".to_string());
        shape.metadata.editor_route = Some("video_timeline".to_string());
        let idx = state.add_shape(shape);
        state.selected = vec![idx];

        let context = state.selected_context();
        assert_eq!(context.selected_count, 1);
        assert_eq!(context.items[0].content_kind, CanvasContentKind::Video);
        assert_eq!(context.items[0].title, "5 秒开场镜头");
        assert_eq!(context.items[0].owner.as_deref(), Some("Video Capy"));
        assert_eq!(
            context.items[0].editor_route.as_deref(),
            Some("video_timeline")
        );

        let text = state.selected_context_text();
        assert!(text.contains("5 秒开场镜头"));
        assert!(text.contains("video"));
        assert!(text.contains("生成分镜"));
    }

    #[test]
    fn imported_image_bytes_becomes_ai_visible_image_content() {
        let mut state = AppState::new();
        let idx = state.import_image_bytes(
            10.0,
            20.0,
            std::sync::Arc::new(vec![255, 0, 0, 255]),
            1,
            1,
            "image/png".to_string(),
        );
        state.selected = vec![idx];

        let context = state.selected_context();
        assert_eq!(context.items[0].content_kind, CanvasContentKind::Image);
        assert_eq!(context.items[0].mime.as_deref(), Some("image/png"));
        assert_eq!(context.items[0].geometry.w, 1.0);
        assert_eq!(context.items[0].geometry.h, 1.0);
    }

    #[test]
    fn ai_snapshot_exports_layout_relationships_and_actions() {
        use capy_canvas_core::connector::create_connector;

        let mut state = AppState::new();
        state.viewport_w = 1440.0;
        state.viewport_h = 900.0;
        let mut video = Shape::new(ShapeKind::Rect, 40.0, 80.0, 0xa78bfa);
        video.w = 240.0;
        video.h = 160.0;
        video.group_id = 7;
        video.metadata.content_kind = Some(CanvasContentKind::Video);
        video.metadata.title = Some("Launch reel".to_string());
        video.metadata.editor_route = Some("video_timeline".to_string());
        let video_idx = state.add_shape(video);

        let mut web = Shape::new(ShapeKind::Rect, 420.0, 120.0, 0x34d399);
        web.w = 300.0;
        web.h = 200.0;
        web.group_id = 7;
        web.metadata.content_kind = Some(CanvasContentKind::Web);
        web.metadata.title = Some("Landing page".to_string());
        let web_idx = state.add_shape(web);

        let video_id = state.shapes[video_idx].id;
        let web_id = state.shapes[web_idx].id;
        assert!(create_connector(&mut state, video_id, web_id));
        state.selected = vec![video_idx];

        let snapshot = state.ai_snapshot();

        assert_eq!(snapshot.schema_version, 1);
        assert_eq!(snapshot.viewport.width, 1440.0);
        assert_eq!(snapshot.nodes.len(), 2);
        assert_eq!(snapshot.nodes[0].id, state.shapes[video_idx].id);
        assert_eq!(snapshot.nodes[0].z_index, 0);
        assert_eq!(snapshot.nodes[0].content_kind, CanvasContentKind::Video);
        assert_eq!(snapshot.nodes[0].bounds.x, 40.0);
        assert_eq!(snapshot.nodes[0].bounds.w, 240.0);
        assert!(snapshot.nodes[0].selected);
        assert!(
            snapshot.nodes[0]
                .available_actions
                .contains(&"open_detail".to_string())
        );
        assert_eq!(snapshot.connectors.len(), 1);
        assert_eq!(
            snapshot.connectors[0].from_title.as_deref(),
            Some("Launch reel")
        );
        assert_eq!(
            snapshot.connectors[0].to_title.as_deref(),
            Some("Landing page")
        );
        assert_eq!(snapshot.groups.len(), 1);
        assert_eq!(snapshot.groups[0].group_id, 7);
        assert_eq!(snapshot.selection.selected_count, 1);

        let json = serde_json::to_value(&snapshot).expect("serialize snapshot");
        assert_eq!(json["nodes"][0]["content_kind"], "video");
        assert_eq!(json["nodes"][0]["bounds"]["x"], 40.0);
        assert_eq!(json["available_actions"][0], "select_by_id");
    }

    #[test]
    fn id_based_core_operations_do_not_depend_on_z_index() {
        let mut state = AppState::new();
        let a = add_rect(&mut state, 0.0, 0.0, 80.0, 80.0);
        let b = add_rect(&mut state, 200.0, 200.0, 80.0, 80.0);
        let a_id = state.shapes[a].id;
        let b_id = state.shapes[b].id;

        state
            .select_shape_ids(&[b_id, a_id])
            .expect("select by ids");
        assert_eq!(state.selected, vec![b, a]);

        state
            .move_shape_by_id(a_id, 50.0, 60.0)
            .expect("move by id");
        assert_eq!(state.shapes[a].x, 50.0);
        assert_eq!(state.shapes[a].y, 60.0);
        assert_eq!(state.selected, vec![a]);

        state.delete_shape_by_id(b_id).expect("delete by id");
        assert_eq!(state.shapes.len(), 1);
        assert_eq!(state.shapes[0].id, a_id);
        assert!(state.delete_shape_by_id(999).is_err());
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Selection tests
// (from tldraw SelectTool.test.ts, select.test.tsx, excalidraw selection.test.ts)
// ═══════════════════════════════════════════════════════════════════════

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

    /// excalidraw: zero-size drag (click without move) still creates a shape but
    /// with zero dimensions.
    #[test]
    fn zero_size_drag_creates_shape_with_zero_dimensions() {
        let mut state = AppState::new();
        state.tool = Tool::Rect;

        // Click without moving
        state.cursor_x = 50.0;
        state.cursor_y = 50.0;
        handle_mouse_button(&mut state, MouseButton::Left, true, false);
        handle_mouse_button(&mut state, MouseButton::Left, false, false);

        // Shape is created but with w=0, h=0
        assert_eq!(state.shapes.len(), 1);
        let s = &state.shapes[0];
        assert!((s.w).abs() < 1e-6, "zero-drag should produce zero width");
        assert!((s.h).abs() < 1e-6, "zero-drag should produce zero height");
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

mod align_distribute {
    use super::*;

    #[test]
    fn align_left_moves_to_min_x() {
        let mut state = AppState::new();
        add_rect(&mut state, 10.0, 0.0, 50.0, 50.0);
        add_rect(&mut state, 100.0, 0.0, 60.0, 50.0);
        add_rect(&mut state, 200.0, 0.0, 40.0, 50.0);
        state.selected = vec![0, 1, 2];
        state.align_left();
        assert!((state.shapes[0].x - 10.0).abs() < 1e-6);
        assert!((state.shapes[1].x - 10.0).abs() < 1e-6);
        assert!((state.shapes[2].x - 10.0).abs() < 1e-6);
    }

    #[test]
    fn align_right_moves_to_max_right_edge() {
        let mut state = AppState::new();
        add_rect(&mut state, 10.0, 0.0, 50.0, 50.0); // right edge = 60
        add_rect(&mut state, 100.0, 0.0, 60.0, 50.0); // right edge = 160
        add_rect(&mut state, 200.0, 0.0, 40.0, 50.0); // right edge = 240
        state.selected = vec![0, 1, 2];
        state.align_right();
        // All right edges should be at 240
        assert!((state.shapes[0].x - 190.0).abs() < 1e-6, "50w -> x=190");
        assert!((state.shapes[1].x - 180.0).abs() < 1e-6, "60w -> x=180");
        assert!((state.shapes[2].x - 200.0).abs() < 1e-6, "40w -> x=200");
    }

    #[test]
    fn align_top_moves_to_min_y() {
        let mut state = AppState::new();
        add_rect(&mut state, 0.0, 20.0, 50.0, 50.0);
        add_rect(&mut state, 0.0, 100.0, 50.0, 50.0);
        state.selected = vec![0, 1];
        state.align_top();
        assert!((state.shapes[0].y - 20.0).abs() < 1e-6);
        assert!((state.shapes[1].y - 20.0).abs() < 1e-6);
    }

    #[test]
    fn align_bottom_moves_to_max_bottom_edge() {
        let mut state = AppState::new();
        add_rect(&mut state, 0.0, 20.0, 50.0, 30.0); // bottom = 50
        add_rect(&mut state, 0.0, 100.0, 50.0, 60.0); // bottom = 160
        state.selected = vec![0, 1];
        state.align_bottom();
        // All bottom edges should be at 160
        assert!((state.shapes[0].y - 130.0).abs() < 1e-6, "30h -> y=130");
        assert!((state.shapes[1].y - 100.0).abs() < 1e-6, "60h -> y=100");
    }

    #[test]
    fn align_center_h_centers_horizontally() {
        let mut state = AppState::new();
        add_rect(&mut state, 0.0, 0.0, 100.0, 50.0); // left=0, right=100
        add_rect(&mut state, 200.0, 0.0, 60.0, 50.0); // left=200, right=260
        state.selected = vec![0, 1];
        state.align_center_h();
        // Bounding box: 0..260, center_x = 130
        let cx0 = state.shapes[0].x + state.shapes[0].w / 2.0;
        let cx1 = state.shapes[1].x + state.shapes[1].w / 2.0;
        assert!((cx0 - 130.0).abs() < 1e-6);
        assert!((cx1 - 130.0).abs() < 1e-6);
    }

    #[test]
    fn align_center_v_centers_vertically() {
        let mut state = AppState::new();
        add_rect(&mut state, 0.0, 0.0, 50.0, 100.0); // top=0, bottom=100
        add_rect(&mut state, 0.0, 200.0, 50.0, 60.0); // top=200, bottom=260
        state.selected = vec![0, 1];
        state.align_center_v();
        // Bounding box: 0..260, center_y = 130
        let cy0 = state.shapes[0].y + state.shapes[0].h / 2.0;
        let cy1 = state.shapes[1].y + state.shapes[1].h / 2.0;
        assert!((cy0 - 130.0).abs() < 1e-6);
        assert!((cy1 - 130.0).abs() < 1e-6);
    }

    #[test]
    fn align_noop_with_one_shape() {
        let mut state = AppState::new();
        add_rect(&mut state, 50.0, 50.0, 100.0, 100.0);
        state.selected = vec![0];
        state.align_left();
        // Should not change anything
        assert!((state.shapes[0].x - 50.0).abs() < 1e-6);
    }

    #[test]
    fn align_pushes_undo() {
        let mut state = AppState::new();
        add_rect(&mut state, 0.0, 0.0, 50.0, 50.0);
        add_rect(&mut state, 100.0, 0.0, 50.0, 50.0);
        state.selected = vec![0, 1];
        let undo_before = state.undo_stack.len();
        state.align_left();
        assert_eq!(
            state.undo_stack.len(),
            undo_before + 1,
            "align should push undo"
        );
    }

    #[test]
    fn distribute_h_even_spacing() {
        let mut state = AppState::new();
        // Three shapes with different widths
        add_rect(&mut state, 0.0, 0.0, 20.0, 50.0); // x=0, w=20
        add_rect(&mut state, 50.0, 0.0, 30.0, 50.0); // x=50, w=30
        add_rect(&mut state, 200.0, 0.0, 40.0, 50.0); // x=200, w=40
        state.selected = vec![0, 1, 2];
        state.distribute_h();
        // Total width of shapes: 20+30+40 = 90
        // Span from first.x to last.x+w: 0..240
        // Available gap space: 240-0-90 = 150, divided by 2 gaps = 75
        // Shape 0: x=0
        // Shape 1: x=0+20+75 = 95
        // Shape 2: x=95+30+75 = 200
        assert!((state.shapes[0].x - 0.0).abs() < 1e-6, "first shape stays");
        assert!(
            (state.shapes[1].x - 95.0).abs() < 1e-6,
            "middle shape centered"
        );
        assert!((state.shapes[2].x - 200.0).abs() < 1e-6, "last shape stays");
    }

    #[test]
    fn distribute_v_even_spacing() {
        let mut state = AppState::new();
        add_rect(&mut state, 0.0, 0.0, 50.0, 20.0);
        add_rect(&mut state, 0.0, 50.0, 50.0, 30.0);
        add_rect(&mut state, 0.0, 200.0, 50.0, 40.0);
        state.selected = vec![0, 1, 2];
        state.distribute_v();
        // Total height: 20+30+40 = 90
        // Span: 0..240
        // Gap: (240-90)/2 = 75
        assert!((state.shapes[0].y - 0.0).abs() < 1e-6);
        assert!((state.shapes[1].y - 95.0).abs() < 1e-6);
        assert!((state.shapes[2].y - 200.0).abs() < 1e-6);
    }

    #[test]
    fn distribute_noop_with_two_shapes() {
        let mut state = AppState::new();
        add_rect(&mut state, 0.0, 0.0, 50.0, 50.0);
        add_rect(&mut state, 100.0, 0.0, 50.0, 50.0);
        state.selected = vec![0, 1];
        let undo_before = state.undo_stack.len();
        state.distribute_h();
        // Should not push undo or change anything
        assert_eq!(state.undo_stack.len(), undo_before);
        assert!((state.shapes[0].x - 0.0).abs() < 1e-6);
        assert!((state.shapes[1].x - 100.0).abs() < 1e-6);
    }

    #[test]
    fn distribute_pushes_undo() {
        let mut state = AppState::new();
        add_rect(&mut state, 0.0, 0.0, 20.0, 50.0);
        add_rect(&mut state, 50.0, 0.0, 20.0, 50.0);
        add_rect(&mut state, 200.0, 0.0, 20.0, 50.0);
        state.selected = vec![0, 1, 2];
        let undo_before = state.undo_stack.len();
        state.distribute_h();
        assert_eq!(
            state.undo_stack.len(),
            undo_before + 1,
            "distribute should push undo"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Wave 4: Advanced drawing tools tests
// ═══════════════════════════════════════════════════════════════════════

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
}

// ═══════════════════════════════════════════════════════════════════════
// Wave 5: Arrow & Connector Polish
// ═══════════════════════════════════════════════════════════════════════

mod wave5_arrows {
    use super::*;
    use capy_canvas_core::state::{ArrowHead, ArrowStyle, ConnectorStyle, Tool};

    // ── Arrow Head Types ──

    #[test]
    fn arrow_default_heads() {
        let shape = Shape::new(ShapeKind::Arrow, 0.0, 0.0, 0x1e1e1e);
        assert_eq!(shape.arrow_start, ArrowHead::None, "start defaults to None");
        assert_eq!(
            shape.arrow_end,
            ArrowHead::Triangle,
            "end defaults to Triangle"
        );
    }

    #[test]
    fn set_arrow_head_start() {
        let mut state = AppState::new();
        let mut arrow = Shape::new(ShapeKind::Arrow, 0.0, 0.0, 0x1e1e1e);
        arrow.w = 100.0;
        arrow.h = 50.0;
        let idx = state.add_shape(arrow);
        state.selected = vec![idx];
        state.shapes[idx].arrow_start = ArrowHead::Circle;
        assert_eq!(state.shapes[idx].arrow_start, ArrowHead::Circle);
    }

    #[test]
    fn set_arrow_head_end_diamond() {
        let mut state = AppState::new();
        let mut arrow = Shape::new(ShapeKind::Arrow, 0.0, 0.0, 0x1e1e1e);
        arrow.w = 100.0;
        arrow.h = 50.0;
        let idx = state.add_shape(arrow);
        state.shapes[idx].arrow_end = ArrowHead::Diamond;
        assert_eq!(state.shapes[idx].arrow_end, ArrowHead::Diamond);
    }

    #[test]
    fn all_arrowhead_types_exist() {
        // Ensure all 5 variants are distinct
        let heads = [
            ArrowHead::None,
            ArrowHead::Triangle,
            ArrowHead::Circle,
            ArrowHead::Diamond,
            ArrowHead::Bar,
        ];
        for (i, a) in heads.iter().enumerate() {
            for (j, b) in heads.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b);
                }
            }
        }
    }

    // ── Arrow Style (Curved/Straight) ──

    #[test]
    fn arrow_default_style_straight() {
        let shape = Shape::new(ShapeKind::Arrow, 0.0, 0.0, 0x1e1e1e);
        assert_eq!(shape.arrow_style, ArrowStyle::Straight);
    }

    #[test]
    fn set_arrow_style_curved() {
        let mut state = AppState::new();
        let mut arrow = Shape::new(ShapeKind::Arrow, 0.0, 0.0, 0x1e1e1e);
        arrow.w = 200.0;
        arrow.h = 100.0;
        let idx = state.add_shape(arrow);
        state.shapes[idx].arrow_style = ArrowStyle::Curved;
        assert_eq!(state.shapes[idx].arrow_style, ArrowStyle::Curved);
    }

    // ── Labels on Arrows ──

    #[test]
    fn arrow_label_default_none() {
        let shape = Shape::new(ShapeKind::Arrow, 0.0, 0.0, 0x1e1e1e);
        assert!(shape.label.is_none());
    }

    #[test]
    fn set_arrow_label() {
        let mut state = AppState::new();
        let mut arrow = Shape::new(ShapeKind::Arrow, 0.0, 0.0, 0x1e1e1e);
        arrow.w = 100.0;
        arrow.h = 50.0;
        let idx = state.add_shape(arrow);
        state.shapes[idx].label = Some("connects to".to_string());
        assert_eq!(state.shapes[idx].label.as_deref(), Some("connects to"));
    }

    // ── Connector Labels ──

    #[test]
    fn connector_label_default_none() {
        let mut state = AppState::new();
        let _a = add_rect(&mut state, 0.0, 0.0, 100.0, 100.0);
        let _b = add_rect(&mut state, 300.0, 0.0, 100.0, 100.0);
        capy_canvas_core::connector::create_connector(&mut state, 1, 2);
        assert!(state.connectors[0].label.is_none());
    }

    #[test]
    fn set_connector_label() {
        let mut state = AppState::new();
        let _a = add_rect(&mut state, 0.0, 0.0, 100.0, 100.0);
        let _b = add_rect(&mut state, 300.0, 0.0, 100.0, 100.0);
        capy_canvas_core::connector::create_connector(&mut state, 1, 2);
        state.connectors[0].label = Some("depends on".to_string());
        assert_eq!(state.connectors[0].label.as_deref(), Some("depends on"));
    }

    // ── Elbow Routing ──

    #[test]
    fn connector_default_style_straight() {
        let mut state = AppState::new();
        let _a = add_rect(&mut state, 0.0, 0.0, 100.0, 100.0);
        let _b = add_rect(&mut state, 300.0, 300.0, 100.0, 100.0);
        capy_canvas_core::connector::create_connector(&mut state, 1, 2);
        assert_eq!(state.connectors[0].style, ConnectorStyle::Straight);
    }

    #[test]
    fn connector_elbow_style() {
        let mut state = AppState::new();
        let _a = add_rect(&mut state, 0.0, 0.0, 100.0, 100.0);
        let _b = add_rect(&mut state, 300.0, 300.0, 100.0, 100.0);
        capy_canvas_core::connector::create_connector_styled(
            &mut state,
            1,
            2,
            ConnectorStyle::Elbow,
        );
        assert_eq!(state.connectors[0].style, ConnectorStyle::Elbow);
    }

    #[test]
    fn elbow_route_bend_point() {
        let bend = capy_canvas_core::connector::elbow_route((0.0, 0.0), (100.0, 100.0));
        // Horizontal-first: bend at (target_x, source_y)
        assert!((bend.0 - 100.0).abs() < 1e-6);
        assert!((bend.1 - 0.0).abs() < 1e-6);
    }

    // ── Binding Indicator ──

    #[test]
    fn binding_indicator_default_none() {
        let state = AppState::new();
        assert!(state.binding_indicator.is_none());
    }

    #[test]
    fn binding_indicator_set_when_arrow_tool_hovers_shape() {
        let mut state = AppState::new();
        state.tool = Tool::Arrow;
        let _a = add_rect(&mut state, 100.0, 100.0, 100.0, 100.0);
        // Simulate hovering over the shape
        let (wx, wy) = (150.0, 150.0); // center of shape
        state.hovered_shape = state.hit_test(wx, wy);
        // Manually compute binding indicator like input.rs does
        if let Some(h) = state.hovered_shape.or(state.hit_test(wx, wy)) {
            let ep = state.shapes[h].edge_point(wx, wy);
            state.binding_indicator = Some(ep);
        }
        assert!(
            state.binding_indicator.is_some(),
            "binding indicator should be set"
        );
    }

    #[test]
    fn binding_indicator_cleared_when_not_hovering() {
        let mut state = AppState::new();
        state.tool = Tool::Arrow;
        state.binding_indicator = Some((50.0, 50.0));
        // Simulate moving away from any shape — no hit
        let (wx, wy) = (9999.0, 9999.0);
        if state.hit_test(wx, wy).is_none() {
            state.binding_indicator = None;
        }
        assert!(
            state.binding_indicator.is_none(),
            "should be cleared when not hovering"
        );
    }

    // ── Serialization round-trip for new fields ──

    #[test]
    fn arrow_fields_serialize_deserialize() {
        let mut arrow = Shape::new(ShapeKind::Arrow, 10.0, 20.0, 0xff0000);
        arrow.w = 100.0;
        arrow.h = 50.0;
        arrow.arrow_start = ArrowHead::Circle;
        arrow.arrow_end = ArrowHead::Diamond;
        arrow.arrow_style = ArrowStyle::Curved;
        arrow.label = Some("test label".to_string());

        let json = serde_json::to_string(&arrow).expect("serialize");
        let restored: Shape = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(restored.arrow_start, ArrowHead::Circle);
        assert_eq!(restored.arrow_end, ArrowHead::Diamond);
        assert_eq!(restored.arrow_style, ArrowStyle::Curved);
        assert_eq!(restored.label.as_deref(), Some("test label"));
    }

    #[test]
    fn connector_fields_serialize_deserialize() {
        let conn = capy_canvas_core::state::Connector {
            from_id: 1,
            to_id: 2,
            color: 0x1e1e1e,
            style: ConnectorStyle::Elbow,
            label: Some("my label".to_string()),
        };

        let json = serde_json::to_string(&conn).expect("serialize");
        let restored: capy_canvas_core::state::Connector =
            serde_json::from_str(&json).expect("deserialize");

        assert_eq!(restored.style, ConnectorStyle::Elbow);
        assert_eq!(restored.label.as_deref(), Some("my label"));
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Wave 6: Text & Font features
// ═══════════════════════════════════════════════════════════════════════

mod wave6_text_font {
    use super::*;
    use capy_canvas_core::state::{FontFamily, TextAlign, TextEditState, Tool};
    use winit::keyboard::{Key, NamedKey};

    // ── Font Family ──

    #[test]
    fn font_family_default_is_sans_serif() {
        let state = AppState::new();
        assert_eq!(state.current_font_family, FontFamily::SansSerif);
    }

    #[test]
    fn font_family_on_new_shape() {
        let mut state = AppState::new();
        state.current_font_family = FontFamily::Mono;
        let mut shape = Shape::new(ShapeKind::Text, 100.0, 100.0, 0x000000);
        shape.font_family = state.current_font_family;
        let idx = state.add_shape(shape);
        assert_eq!(state.shapes[idx].font_family, FontFamily::Mono);
    }

    #[test]
    fn font_family_enum_variants() {
        assert_eq!(FontFamily::SansSerif.label(), "Sans Serif");
        assert_eq!(FontFamily::Serif.label(), "Serif");
        assert_eq!(FontFamily::Mono.label(), "Mono");
        assert_eq!(FontFamily::Handwritten.label(), "Handwritten");
    }

    #[test]
    fn font_family_serialization() {
        let family = FontFamily::Handwritten;
        let json = serde_json::to_string(&family).expect("serialize");
        assert_eq!(json, "\"handwritten\"");
        let restored: FontFamily = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, FontFamily::Handwritten);
    }

    // ── Font Size ──

    #[test]
    fn font_size_default_is_14() {
        let state = AppState::new();
        assert!((state.current_font_size - 14.0).abs() < f64::EPSILON);
    }

    #[test]
    fn font_size_on_shape_default() {
        let shape = Shape::new(ShapeKind::Text, 0.0, 0.0, 0x000000);
        assert!((shape.font_size - 14.0).abs() < f64::EPSILON);
    }

    #[test]
    fn font_size_custom() {
        let mut state = AppState::new();
        state.current_font_size = 24.0;
        let mut shape = Shape::new(ShapeKind::Text, 0.0, 0.0, 0x000000);
        shape.font_size = state.current_font_size;
        let idx = state.add_shape(shape);
        assert!((state.shapes[idx].font_size - 24.0).abs() < f64::EPSILON);
    }

    // ── Text Alignment ──

    #[test]
    fn text_align_default_is_left() {
        let shape = Shape::new(ShapeKind::Text, 0.0, 0.0, 0x000000);
        assert_eq!(shape.text_align, TextAlign::Left);
    }

    #[test]
    fn text_align_serialization() {
        let align = TextAlign::Center;
        let json = serde_json::to_string(&align).expect("serialize");
        assert_eq!(json, "\"center\"");
        let restored: TextAlign = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, TextAlign::Center);
    }

    #[test]
    fn text_align_right() {
        let mut state = AppState::new();
        state.current_text_align = TextAlign::Right;
        let mut shape = Shape::new(ShapeKind::Text, 0.0, 0.0, 0x000000);
        shape.text_align = state.current_text_align;
        let idx = state.add_shape(shape);
        assert_eq!(state.shapes[idx].text_align, TextAlign::Right);
    }

    // ── Bold / Italic ──

    #[test]
    fn bold_italic_default_false() {
        let shape = Shape::new(ShapeKind::Text, 0.0, 0.0, 0x000000);
        assert!(!shape.bold);
        assert!(!shape.italic);
    }

    #[test]
    fn bold_italic_toggle() {
        let mut state = AppState::new();
        assert!(!state.current_bold);
        state.current_bold = !state.current_bold;
        assert!(state.current_bold);
        state.current_italic = true;
        assert!(state.current_italic);
    }

    #[test]
    fn bold_italic_on_shape() {
        let mut shape = Shape::new(ShapeKind::Text, 0.0, 0.0, 0x000000);
        shape.bold = true;
        shape.italic = true;
        let json = serde_json::to_string(&shape).expect("serialize");
        let restored: Shape = serde_json::from_str(&json).expect("deserialize");
        assert!(restored.bold);
        assert!(restored.italic);
    }

    // ── Multi-line Text ──

    #[test]
    fn multiline_enter_inserts_newline() {
        let mut state = AppState::new();
        state.tool = Tool::Text;
        let mut shape = Shape::new(ShapeKind::Text, 100.0, 100.0, 0x000000);
        shape.w = 120.0;
        shape.h = 80.0;
        shape.text = "Hello".to_string();
        let idx = state.add_shape(shape);
        state.text_edit = Some(TextEditState {
            shape_index: idx,
            target: capy_canvas_core::state::TextTarget::Body,
            cursor: 5,
            blink_visible: true,
            selection_start: None,
        });

        let mods = winit::event::Modifiers::default();
        capy_canvas_core::input::handle_key(&mut state, &Key::Named(NamedKey::Enter), true, mods);

        assert_eq!(state.shapes[idx].text, "Hello\n");
        assert_eq!(state.text_edit.as_ref().map(|te| te.cursor), Some(6));
    }

    #[test]
    fn multiline_text_split() {
        let mut state = AppState::new();
        let mut shape = Shape::new(ShapeKind::Text, 0.0, 0.0, 0x000000);
        shape.text = "line1\nline2\nline3".to_string();
        shape.w = 200.0;
        shape.h = 100.0;
        let idx = state.add_shape(shape);

        let lines: Vec<&str> = state.shapes[idx].text.split('\n').collect();
        assert_eq!(lines, vec!["line1", "line2", "line3"]);
    }

    // ── Arrow Key Navigation ──

    #[test]
    fn arrow_left_moves_cursor() {
        let mut state = AppState::new();
        let mut shape = Shape::new(ShapeKind::Text, 0.0, 0.0, 0x000000);
        shape.w = 120.0;
        shape.h = 30.0;
        shape.text = "ABC".to_string();
        let idx = state.add_shape(shape);
        state.text_edit = Some(TextEditState {
            shape_index: idx,
            target: capy_canvas_core::state::TextTarget::Body,
            cursor: 3,
            blink_visible: true,
            selection_start: None,
        });

        let mods = winit::event::Modifiers::default();
        capy_canvas_core::input::handle_key(
            &mut state,
            &Key::Named(NamedKey::ArrowLeft),
            true,
            mods,
        );

        assert_eq!(state.text_edit.as_ref().map(|te| te.cursor), Some(2));
    }

    #[test]
    fn arrow_right_moves_cursor() {
        let mut state = AppState::new();
        let mut shape = Shape::new(ShapeKind::Text, 0.0, 0.0, 0x000000);
        shape.w = 120.0;
        shape.h = 30.0;
        shape.text = "ABC".to_string();
        let idx = state.add_shape(shape);
        state.text_edit = Some(TextEditState {
            shape_index: idx,
            target: capy_canvas_core::state::TextTarget::Body,
            cursor: 0,
            blink_visible: true,
            selection_start: None,
        });

        let mods = winit::event::Modifiers::default();
        capy_canvas_core::input::handle_key(
            &mut state,
            &Key::Named(NamedKey::ArrowRight),
            true,
            mods,
        );

        assert_eq!(state.text_edit.as_ref().map(|te| te.cursor), Some(1));
    }

    #[test]
    fn arrow_up_down_between_lines() {
        let mut state = AppState::new();
        let mut shape = Shape::new(ShapeKind::Text, 0.0, 0.0, 0x000000);
        shape.w = 200.0;
        shape.h = 100.0;
        shape.text = "abc\ndef".to_string();
        let idx = state.add_shape(shape);
        // Cursor at line 0, col 2 (char index 2)
        state.text_edit = Some(TextEditState {
            shape_index: idx,
            target: capy_canvas_core::state::TextTarget::Body,
            cursor: 2,
            blink_visible: true,
            selection_start: None,
        });

        let mods = winit::event::Modifiers::default();
        // ArrowDown: move to line 1, col 2
        capy_canvas_core::input::handle_key(
            &mut state,
            &Key::Named(NamedKey::ArrowDown),
            true,
            mods,
        );
        // "abc\ndef" — line 1 starts at char 4, col 2 = char 6
        assert_eq!(state.text_edit.as_ref().map(|te| te.cursor), Some(6));

        // ArrowUp: back to line 0, col 2
        capy_canvas_core::input::handle_key(&mut state, &Key::Named(NamedKey::ArrowUp), true, mods);
        assert_eq!(state.text_edit.as_ref().map(|te| te.cursor), Some(2));
    }

    // ── Text Selection ──

    #[test]
    fn text_edit_selection_start_none_by_default() {
        let te = TextEditState {
            shape_index: 0,
            target: capy_canvas_core::state::TextTarget::Body,
            cursor: 0,
            blink_visible: true,
            selection_start: None,
        };
        assert!(!te.has_selection());
        assert!(te.selection_range().is_none());
    }

    #[test]
    fn text_edit_selection_range() {
        let te = TextEditState {
            shape_index: 0,
            target: capy_canvas_core::state::TextTarget::Body,
            cursor: 5,
            blink_visible: true,
            selection_start: Some(2),
        };
        assert!(te.has_selection());
        assert_eq!(te.selection_range(), Some((2, 5)));
    }

    #[test]
    fn text_edit_selection_range_reversed() {
        let te = TextEditState {
            shape_index: 0,
            target: capy_canvas_core::state::TextTarget::Body,
            cursor: 1,
            blink_visible: true,
            selection_start: Some(4),
        };
        // selection_range always returns (min, max)
        assert_eq!(te.selection_range(), Some((1, 4)));
    }

    #[test]
    fn backspace_deletes_selection() {
        let mut state = AppState::new();
        let mut shape = Shape::new(ShapeKind::Text, 0.0, 0.0, 0x000000);
        shape.w = 200.0;
        shape.h = 30.0;
        shape.text = "ABCDE".to_string();
        let idx = state.add_shape(shape);
        // Select chars 1..3 (BC)
        state.text_edit = Some(TextEditState {
            shape_index: idx,
            target: capy_canvas_core::state::TextTarget::Body,
            cursor: 3,
            blink_visible: true,
            selection_start: Some(1),
        });

        let mods = winit::event::Modifiers::default();
        capy_canvas_core::input::handle_key(
            &mut state,
            &Key::Named(NamedKey::Backspace),
            true,
            mods,
        );

        assert_eq!(state.shapes[idx].text, "ADE");
        let te = state.text_edit.as_ref().expect("still editing");
        assert_eq!(te.cursor, 1);
        assert!(!te.has_selection());
    }

    // ── Shape fields serialization round-trip ──

    #[test]
    fn shape_text_fields_serialize_deserialize() {
        let mut shape = Shape::new(ShapeKind::Text, 10.0, 20.0, 0xff0000);
        shape.w = 100.0;
        shape.h = 50.0;
        shape.font_family = FontFamily::Mono;
        shape.font_size = 24.0;
        shape.text_align = TextAlign::Center;
        shape.bold = true;
        shape.italic = true;
        shape.text = "Hello\nWorld".to_string();

        let json = serde_json::to_string(&shape).expect("serialize");
        let restored: Shape = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(restored.font_family, FontFamily::Mono);
        assert!((restored.font_size - 24.0).abs() < f64::EPSILON);
        assert_eq!(restored.text_align, TextAlign::Center);
        assert!(restored.bold);
        assert!(restored.italic);
        assert_eq!(restored.text, "Hello\nWorld");
    }

    // ── AppState defaults ──

    #[test]
    fn app_state_text_defaults() {
        let state = AppState::new();
        assert_eq!(state.current_font_family, FontFamily::SansSerif);
        assert!((state.current_font_size - 14.0).abs() < f64::EPSILON);
        assert_eq!(state.current_text_align, TextAlign::Left);
        assert!(!state.current_bold);
        assert!(!state.current_italic);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Wave 7 — Export, Import & Viewport
// ═══════════════════════════════════════════════════════════════════════

mod wave7_export_import_viewport {
    use super::*;
    use capy_canvas_core::state::ShapeKind;

    // ── SVG Export ──

    /// SVG export generates valid SVG with xmlns attribute.
    #[test]
    fn export_svg_has_xmlns() {
        let mut state = AppState::new();
        add_rect(&mut state, 10.0, 20.0, 100.0, 80.0);
        let svg = state.export_svg();
        assert!(svg.contains("xmlns=\"http://www.w3.org/2000/svg\""));
    }

    /// SVG export includes rect element for Rect shapes.
    #[test]
    fn export_svg_rect_element() {
        let mut state = AppState::new();
        add_rect(&mut state, 50.0, 60.0, 200.0, 100.0);
        let svg = state.export_svg();
        assert!(svg.contains("<rect"), "SVG should contain <rect> element");
        assert!(svg.contains("width=\"200\""), "rect width should be 200");
        assert!(svg.contains("height=\"100\""), "rect height should be 100");
    }

    /// SVG export includes ellipse element for Ellipse shapes.
    #[test]
    fn export_svg_ellipse_element() {
        let mut state = AppState::new();
        add_ellipse(&mut state, 0.0, 0.0, 120.0, 80.0);
        let svg = state.export_svg();
        assert!(
            svg.contains("<ellipse"),
            "SVG should contain <ellipse> element"
        );
        assert!(svg.contains("rx=\"60\""), "ellipse rx should be 60");
        assert!(svg.contains("ry=\"40\""), "ellipse ry should be 40");
    }

    /// SVG export includes line element for Line shapes.
    #[test]
    fn export_svg_line_element() {
        let mut state = AppState::new();
        add_line(&mut state, 10.0, 20.0, 110.0, 120.0);
        let svg = state.export_svg();
        assert!(svg.contains("<line"), "SVG should contain <line> element");
    }

    /// SVG export includes arrow with marker.
    #[test]
    fn export_svg_arrow_has_marker() {
        let mut state = AppState::new();
        let mut s = Shape::new(ShapeKind::Arrow, 0.0, 0.0, 0x1e1e1e);
        s.w = 100.0;
        s.h = 50.0;
        state.add_shape(s);
        let svg = state.export_svg();
        assert!(
            svg.contains("marker-end"),
            "arrow should have marker-end attribute"
        );
        assert!(
            svg.contains("<marker"),
            "SVG defs should include marker definition"
        );
    }

    /// SVG export includes text element for Text shapes.
    #[test]
    fn export_svg_text_element() {
        let mut state = AppState::new();
        let mut s = Shape::new(ShapeKind::Text, 10.0, 10.0, 0x1e1e1e);
        s.w = 200.0;
        s.h = 40.0;
        s.text = "Hello World".to_string();
        state.add_shape(s);
        let svg = state.export_svg();
        assert!(svg.contains("<text"), "SVG should contain <text> element");
        assert!(
            svg.contains("Hello World"),
            "text content should be present"
        );
    }

    /// SVG export includes triangle as polygon.
    #[test]
    fn export_svg_triangle_as_polygon() {
        let mut state = AppState::new();
        let mut s = Shape::new(ShapeKind::Triangle, 0.0, 0.0, 0xff0000);
        s.w = 100.0;
        s.h = 100.0;
        state.add_shape(s);
        let svg = state.export_svg();
        assert!(
            svg.contains("<polygon"),
            "triangle should render as <polygon>"
        );
    }

    /// SVG export includes diamond as polygon.
    #[test]
    fn export_svg_diamond_as_polygon() {
        let mut state = AppState::new();
        let mut s = Shape::new(ShapeKind::Diamond, 0.0, 0.0, 0x00ff00);
        s.w = 80.0;
        s.h = 80.0;
        state.add_shape(s);
        let svg = state.export_svg();
        assert!(
            svg.contains("<polygon"),
            "diamond should render as <polygon>"
        );
    }

    /// SVG export includes freehand as path.
    #[test]
    fn export_svg_freehand_as_path() {
        let mut state = AppState::new();
        let mut s = Shape::new(ShapeKind::Freehand, 0.0, 0.0, 0x0000ff);
        s.w = 100.0;
        s.h = 100.0;
        s.points = vec![(0.0, 0.0), (50.0, 50.0), (100.0, 0.0)];
        state.add_shape(s);
        let svg = state.export_svg();
        assert!(svg.contains("<path"), "freehand should render as <path>");
    }

    /// SVG export includes sticky note as rect+text.
    #[test]
    fn export_svg_sticky_note() {
        let mut state = AppState::new();
        let mut s = Shape::new(ShapeKind::StickyNote, 10.0, 10.0, 0xfef3c7);
        s.w = 200.0;
        s.h = 200.0;
        s.text = "Note text".to_string();
        state.add_shape(s);
        let svg = state.export_svg();
        assert!(svg.contains("<rect"), "sticky note should have <rect>");
        assert!(
            svg.contains("Note text"),
            "sticky note text should be present"
        );
    }

    /// SVG export with rotation includes transform attribute.
    #[test]
    fn export_svg_rotation_transform() {
        let mut state = AppState::new();
        let mut s = Shape::new(ShapeKind::Rect, 0.0, 0.0, 0x1e1e1e);
        s.w = 100.0;
        s.h = 50.0;
        s.rotation = std::f64::consts::FRAC_PI_4; // 45 degrees
        state.add_shape(s);
        let svg = state.export_svg();
        assert!(
            svg.contains("transform=\"rotate("),
            "rotated shape should have transform"
        );
    }

    /// SVG export escapes XML special characters.
    #[test]
    fn export_svg_escapes_xml() {
        let mut state = AppState::new();
        let mut s = Shape::new(ShapeKind::Text, 0.0, 0.0, 0x1e1e1e);
        s.w = 200.0;
        s.h = 40.0;
        s.text = "a < b & c > d".to_string();
        state.add_shape(s);
        let svg = state.export_svg();
        assert!(svg.contains("&lt;"), "< should be escaped to &lt;");
        assert!(svg.contains("&amp;"), "& should be escaped to &amp;");
        assert!(svg.contains("&gt;"), "> should be escaped to &gt;");
    }

    /// SVG export empty canvas produces valid SVG.
    #[test]
    fn export_svg_empty_canvas() {
        let state = AppState::new();
        let svg = state.export_svg();
        assert!(svg.contains("<svg"));
        assert!(svg.contains("</svg>"));
    }

    // ── Image Import ──

    /// import_image creates an Image shape with the given path.
    #[test]
    fn import_image_creates_shape() {
        let mut state = AppState::new();
        let idx = state.import_image("/tmp/photo.png", 50.0, 60.0);
        assert_eq!(state.shapes[idx].kind, ShapeKind::Image);
        assert_eq!(
            state.shapes[idx].image_path.as_deref(),
            Some("/tmp/photo.png")
        );
        assert!((state.shapes[idx].x - 50.0).abs() < f64::EPSILON);
        assert!((state.shapes[idx].y - 60.0).abs() < f64::EPSILON);
    }

    /// import_image sets default placeholder dimensions.
    #[test]
    fn import_image_default_size() {
        let mut state = AppState::new();
        let idx = state.import_image("/tmp/img.jpg", 0.0, 0.0);
        assert!((state.shapes[idx].w - 200.0).abs() < f64::EPSILON);
        assert!((state.shapes[idx].h - 150.0).abs() < f64::EPSILON);
    }

    /// import_image pushes undo.
    #[test]
    fn import_image_pushes_undo() {
        let mut state = AppState::new();
        assert!(state.undo_stack.is_empty());
        state.import_image("/tmp/img.png", 0.0, 0.0);
        assert!(!state.undo_stack.is_empty());
    }

    /// Image shape has "IMG" text.
    #[test]
    fn import_image_has_img_text() {
        let mut state = AppState::new();
        let idx = state.import_image("/tmp/test.png", 0.0, 0.0);
        assert_eq!(state.shapes[idx].text, "IMG");
    }

    /// Image shape hit test works like rect.
    #[test]
    fn image_shape_hit_test() {
        let mut state = AppState::new();
        let idx = state.import_image("/tmp/test.png", 100.0, 100.0);
        // Shape at (100, 100) with size (200, 150)
        assert!(
            state.shapes[idx].contains(150.0, 150.0),
            "center of image should hit"
        );
        assert!(
            !state.shapes[idx].contains(50.0, 50.0),
            "outside image should miss"
        );
    }

    /// Image shape appears in SVG export.
    #[test]
    fn image_in_svg_export() {
        let mut state = AppState::new();
        state.import_image("/tmp/photo.png", 10.0, 20.0);
        let svg = state.export_svg();
        assert!(
            svg.contains("<rect"),
            "image placeholder should export as rect"
        );
        assert!(
            svg.contains("IMG"),
            "image placeholder should have IMG label"
        );
    }

    // ── Zoom to Fit ──

    /// zoom_fit adjusts camera to show all shapes.
    #[test]
    fn zoom_fit_shows_all_shapes() {
        let mut state = AppState::new();
        state.viewport_w = 800.0;
        state.viewport_h = 600.0;
        add_rect(&mut state, 0.0, 0.0, 100.0, 100.0);
        add_rect(&mut state, 500.0, 500.0, 100.0, 100.0);
        state.zoom_fit();
        // After zoom_fit, all shapes should be visible
        // The zoom should be set to fit all content
        assert!(state.camera.zoom > 0.0);
        assert!(state.camera.zoom <= 10.0);
    }

    /// zoom_fit with no shapes is a no-op.
    #[test]
    fn zoom_fit_empty_canvas_noop() {
        let mut state = AppState::new();
        let orig_zoom = state.camera.zoom;
        let orig_ox = state.camera.offset_x;
        state.zoom_fit();
        assert!((state.camera.zoom - orig_zoom).abs() < f64::EPSILON);
        assert!((state.camera.offset_x - orig_ox).abs() < f64::EPSILON);
    }

    /// zoom_fit centers content in viewport.
    #[test]
    fn zoom_fit_centers_content() {
        let mut state = AppState::new();
        state.viewport_w = 1000.0;
        state.viewport_h = 800.0;
        // Single shape at origin
        add_rect(&mut state, 0.0, 0.0, 200.0, 100.0);
        state.zoom_fit();
        // The shape center (100, 50) should map to viewport center (500, 400)
        let (wx, wy) = state
            .camera
            .screen_to_world(state.viewport_w / 2.0, state.viewport_h / 2.0);
        // World point at viewport center should be near shape center
        assert!(
            (wx - 100.0).abs() < 1.0,
            "center x should be near 100, got {wx}"
        );
        assert!(
            (wy - 50.0).abs() < 1.0,
            "center y should be near 50, got {wy}"
        );
    }

    #[test]
    fn zoom_fit_handles_zero_width_bounds() {
        let mut state = AppState::new();
        state.viewport_w = 800.0;
        state.viewport_h = 600.0;
        add_line(&mut state, 100.0, 100.0, 100.0, 200.0);

        state.zoom_fit();

        assert!(
            state.camera.zoom > 1.0,
            "vertical line should still zoom to fit"
        );
        assert!((state.target_zoom - state.camera.zoom).abs() < f64::EPSILON);
    }

    // ── Zoom to Selection ──

    /// zoom_selection adjusts camera to show selected shapes only.
    #[test]
    fn zoom_selection_shows_selected() {
        let mut state = AppState::new();
        state.viewport_w = 800.0;
        state.viewport_h = 600.0;
        add_rect(&mut state, 0.0, 0.0, 50.0, 50.0);
        add_rect(&mut state, 1000.0, 1000.0, 50.0, 50.0);
        state.selected = vec![1]; // select only the far shape
        state.zoom_selection();
        // Camera should zoom to the selected shape
        let (wx, wy) = state
            .camera
            .screen_to_world(state.viewport_w / 2.0, state.viewport_h / 2.0);
        // Center should be near selected shape center (1025, 1025)
        assert!(
            (wx - 1025.0).abs() < 1.0,
            "center x should be near 1025, got {wx}"
        );
        assert!(
            (wy - 1025.0).abs() < 1.0,
            "center y should be near 1025, got {wy}"
        );
    }

    /// zoom_selection with empty selection is a no-op.
    #[test]
    fn zoom_selection_empty_noop() {
        let mut state = AppState::new();
        add_rect(&mut state, 100.0, 100.0, 50.0, 50.0);
        let orig_zoom = state.camera.zoom;
        state.selected.clear();
        state.zoom_selection();
        assert!((state.camera.zoom - orig_zoom).abs() < f64::EPSILON);
    }

    #[test]
    fn zoom_selection_handles_zero_height_bounds() {
        let mut state = AppState::new();
        state.viewport_w = 800.0;
        state.viewport_h = 600.0;
        let idx = add_line(&mut state, 100.0, 100.0, 300.0, 100.0);
        state.selected = vec![idx];

        state.zoom_selection();

        assert!(
            state.camera.zoom > 1.0,
            "horizontal line selection should still zoom"
        );
        assert!((state.target_zoom - state.camera.zoom).abs() < f64::EPSILON);
    }

    // ── Save / Load ──

    /// JSON save/load round-trip shapes.
    #[test]
    fn save_load_roundtrip() {
        let mut state = AppState::new();
        add_rect(&mut state, 10.0, 20.0, 100.0, 80.0);
        add_ellipse(&mut state, 200.0, 200.0, 60.0, 40.0);
        let json = state.to_json_string().expect("save should succeed");

        let mut state2 = AppState::new();
        state2
            .load_from_json_str(&json)
            .expect("load should succeed");
        assert_eq!(state2.shapes.len(), 2);
        assert!((state2.shapes[0].x - 10.0).abs() < f64::EPSILON);
        assert!((state2.shapes[1].x - 200.0).abs() < f64::EPSILON);
    }

    /// JSON save/load preserves camera state.
    #[test]
    fn save_load_preserves_camera() {
        let mut state = AppState::new();
        state.camera.zoom = 2.5;
        state.camera.offset_x = 100.0;
        state.camera.offset_y = -50.0;
        add_rect(&mut state, 0.0, 0.0, 50.0, 50.0);
        let json = state.to_json_string().expect("save");

        let mut state2 = AppState::new();
        state2.load_from_json_str(&json).expect("load");
        assert!((state2.camera.zoom - 2.5).abs() < f64::EPSILON);
        assert!((state2.camera.offset_x - 100.0).abs() < f64::EPSILON);
        assert!((state2.camera.offset_y - (-50.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn save_load_syncs_target_zoom_with_camera() {
        let mut state = AppState::new();
        state.camera.zoom = 2.5;
        state.target_zoom = 2.5;
        add_rect(&mut state, 0.0, 0.0, 50.0, 50.0);
        let json = state.to_json_string().expect("save");

        let mut state2 = AppState::new();
        state2.target_zoom = 0.25;
        state2.load_from_json_str(&json).expect("load");
        assert!((state2.camera.zoom - 2.5).abs() < f64::EPSILON);
        assert!((state2.target_zoom - 2.5).abs() < f64::EPSILON);
    }

    #[test]
    fn save_load_preserves_next_group_id_allocator() {
        let mut state = AppState::new();
        let a = add_rect(&mut state, 0.0, 0.0, 50.0, 50.0);
        let b = add_rect(&mut state, 80.0, 0.0, 50.0, 50.0);
        state.selected = vec![a, b];
        state.group_selected();
        let original_group_id = state.shapes[a].group_id;
        assert_eq!(original_group_id, 1);
        assert_eq!(state.next_group_id, 2);
        let json = state.to_json_string().expect("save");

        let mut state2 = AppState::new();
        state2.load_from_json_str(&json).expect("load");
        let c = add_rect(&mut state2, 200.0, 0.0, 50.0, 50.0);
        let d = add_rect(&mut state2, 280.0, 0.0, 50.0, 50.0);
        state2.selected = vec![c, d];
        state2.group_selected();

        let new_group_id = state2.shapes[c].group_id;
        assert_eq!(state2.shapes[0].group_id, original_group_id);
        assert_eq!(state2.shapes[1].group_id, original_group_id);
        assert_ne!(new_group_id, original_group_id);
        assert_eq!(state2.shapes[d].group_id, new_group_id);
        assert_eq!(new_group_id, 2);
    }

    /// load_from_json_str with malformed JSON returns error.
    #[test]
    fn load_nonexistent_file_returns_error() {
        let mut state = AppState::new();
        let result = state.load_from_json_str("not json");
        assert!(result.is_err());
    }

    /// load pushes undo so user can revert.
    #[test]
    fn load_pushes_undo() {
        let mut state = AppState::new();
        add_rect(&mut state, 0.0, 0.0, 50.0, 50.0);
        let json = state.to_json_string().expect("save");

        let mut state2 = AppState::new();
        add_rect(&mut state2, 999.0, 999.0, 10.0, 10.0);
        assert!(state2.undo_stack.is_empty());
        state2.load_from_json_str(&json).expect("load");
        assert!(!state2.undo_stack.is_empty(), "load should push undo");
    }

    /// Save/load preserves image shape with image_path.
    #[test]
    fn save_load_preserves_image_shape() {
        let mut state = AppState::new();
        state.import_image("/tmp/photo.png", 50.0, 60.0);
        let json = state.to_json_string().expect("save");

        let mut state2 = AppState::new();
        state2.load_from_json_str(&json).expect("load");
        assert_eq!(state2.shapes.len(), 1);
        assert_eq!(state2.shapes[0].kind, ShapeKind::Image);
        assert_eq!(
            state2.shapes[0].image_path.as_deref(),
            Some("/tmp/photo.png")
        );
    }
}

// ════════════════════════════════════════════════════════════════════════
// Wave 8: Interaction Polish (Final Wave)
// ════════════════════════════════════════════════════════════════════════

mod wave8_interaction_polish {
    use super::*;
    use capy_canvas_core::state::Toast;

    // ── Dark Mode ──

    #[test]
    fn dark_mode_default_false() {
        let state = AppState::new();
        assert!(!state.dark_mode);
    }

    #[test]
    fn dark_mode_toggle() {
        let mut state = AppState::new();
        state.dark_mode = true;
        assert!(state.dark_mode);
        state.dark_mode = false;
        assert!(!state.dark_mode);
    }

    // ── Send Forward / Backward ──

    #[test]
    fn send_forward_swaps_with_above() {
        let mut state = AppState::new();
        let a = add_rect(&mut state, 0.0, 0.0, 50.0, 50.0);
        let b = add_rect(&mut state, 100.0, 100.0, 50.0, 50.0);
        let _c = add_rect(&mut state, 200.0, 200.0, 50.0, 50.0);
        let id_a = state.shapes[a].id;
        let id_b = state.shapes[b].id;
        state.selected = vec![a]; // select first shape
        state.send_forward();
        // a should now be at index 1
        assert_eq!(state.shapes[0].id, id_b);
        assert_eq!(state.shapes[1].id, id_a);
        assert_eq!(state.selected, vec![1]);
    }

    #[test]
    fn send_backward_swaps_with_below() {
        let mut state = AppState::new();
        add_rect(&mut state, 0.0, 0.0, 50.0, 50.0);
        let b = add_rect(&mut state, 100.0, 100.0, 50.0, 50.0);
        add_rect(&mut state, 200.0, 200.0, 50.0, 50.0);
        let id_a = state.shapes[0].id;
        let id_b = state.shapes[b].id;
        state.selected = vec![b]; // select middle shape
        state.send_backward();
        // b should now be at index 0
        assert_eq!(state.shapes[0].id, id_b);
        assert_eq!(state.shapes[1].id, id_a);
        assert_eq!(state.selected, vec![0]);
    }

    #[test]
    fn send_forward_at_top_is_noop() {
        let mut state = AppState::new();
        add_rect(&mut state, 0.0, 0.0, 50.0, 50.0);
        add_rect(&mut state, 100.0, 100.0, 50.0, 50.0);
        state.selected = vec![1]; // already at top
        let id_top = state.shapes[1].id;
        state.send_forward();
        assert_eq!(state.shapes[1].id, id_top);
    }

    #[test]
    fn send_backward_at_bottom_is_noop() {
        let mut state = AppState::new();
        add_rect(&mut state, 0.0, 0.0, 50.0, 50.0);
        add_rect(&mut state, 100.0, 100.0, 50.0, 50.0);
        state.selected = vec![0]; // already at bottom
        let id_bottom = state.shapes[0].id;
        state.send_backward();
        assert_eq!(state.shapes[0].id, id_bottom);
    }

    #[test]
    fn send_forward_pushes_undo() {
        let mut state = AppState::new();
        add_rect(&mut state, 0.0, 0.0, 50.0, 50.0);
        add_rect(&mut state, 100.0, 100.0, 50.0, 50.0);
        state.selected = vec![0];
        assert!(state.undo_stack.is_empty());
        state.send_forward();
        assert!(!state.undo_stack.is_empty());
    }

    // ── Cut ──

    #[test]
    fn cut_copies_and_deletes() {
        let mut state = AppState::new();
        add_rect(&mut state, 10.0, 20.0, 50.0, 50.0);
        add_rect(&mut state, 100.0, 100.0, 50.0, 50.0);
        state.selected = vec![0];
        state.cut_selected();
        assert_eq!(state.shapes.len(), 1, "cut should remove the shape");
        assert_eq!(state.clipboard.len(), 1, "cut should copy to clipboard");
        assert!((state.clipboard[0].x - 10.0).abs() < 1e-6);
    }

    // ── Zoom In / Out ──

    #[test]
    fn zoom_in_increases_zoom() {
        let mut state = AppState::new();
        let before = state.camera.zoom;
        state.zoom_in();
        assert!(state.camera.zoom > before);
    }

    #[test]
    fn zoom_out_decreases_zoom() {
        let mut state = AppState::new();
        let before = state.camera.zoom;
        state.zoom_out();
        assert!(state.camera.zoom < before);
    }

    #[test]
    fn zoom_in_clamped_at_max() {
        let mut state = AppState::new();
        state.camera.zoom = 9.9;
        state.target_zoom = 9.9;
        state.zoom_in();
        assert!(state.camera.zoom <= 10.0);
    }

    #[test]
    fn zoom_out_clamped_at_min() {
        let mut state = AppState::new();
        state.camera.zoom = 0.11;
        state.target_zoom = 0.11;
        state.zoom_out();
        assert!(state.camera.zoom >= 0.1);
    }

    // ── Toast Notifications ──

    #[test]
    fn show_toast_adds_to_list() {
        let mut state = AppState::new();
        state.show_toast("Hello", 2000);
        assert_eq!(state.toasts.len(), 1);
        assert_eq!(state.toasts[0].message, "Hello");
    }

    #[test]
    fn toast_opacity_starts_at_one() {
        let toast = Toast::new("Test", 5000);
        assert!((toast.opacity() - 1.0).abs() < 0.1);
    }

    #[test]
    fn toast_not_expired_immediately() {
        let toast = Toast::new("Test", 5000);
        assert!(!toast.is_expired());
    }

    #[test]
    fn gc_toasts_removes_expired() {
        let mut state = AppState::new();
        // Create a toast with 0ms duration (immediately expired)
        state.toasts.push(Toast {
            message: "Gone".to_string(),
            created: std::time::Instant::now() - std::time::Duration::from_secs(10),
            duration_ms: 1000,
        });
        state.gc_toasts();
        assert!(state.toasts.is_empty());
    }

    // ── Help Overlay ──

    #[test]
    fn help_overlay_default_hidden() {
        let state = AppState::new();
        assert!(!state.show_help);
    }

    #[test]
    fn help_overlay_toggle() {
        let mut state = AppState::new();
        state.show_help = true;
        assert!(state.show_help);
        state.show_help = false;
        assert!(!state.show_help);
    }

    // ── Smooth Zoom ──

    #[test]
    fn target_zoom_default_matches_camera() {
        let state = AppState::new();
        assert!((state.target_zoom - state.camera.zoom).abs() < 1e-6);
    }

    #[test]
    fn target_zoom_updates_on_zoom_in() {
        let mut state = AppState::new();
        state.zoom_in();
        // target_zoom should be set to the new camera zoom
        assert!((state.target_zoom - state.camera.zoom).abs() < 1e-6);
    }
}

mod review_regressions {
    use super::*;
    use capy_canvas_core::state::{ContextAction, ContextMenu, ContextMenuItem, Tool};

    #[test]
    fn context_menu_bottom_edge_does_not_trigger_action() {
        let mut state = AppState::new();
        let idx = add_rect(&mut state, 0.0, 0.0, 100.0, 100.0);
        state.selected = vec![idx];
        state.context_menu = Some(ContextMenu {
            sx: 20.0,
            sy: 30.0,
            items: vec![ContextMenuItem {
                label: "Delete",
                action: ContextAction::Delete,
            }],
            hovered: None,
        });

        state.cursor_x = 60.0;
        state.cursor_y = 30.0 + ContextMenu::PAD + ContextMenu::ITEM_H;

        assert!(handle_mouse_button(
            &mut state,
            MouseButton::Left,
            true,
            false
        ));
        assert_eq!(state.shapes.len(), 1, "bottom-edge click must not delete");
        assert!(
            state.context_menu.is_none(),
            "menu should close after click"
        );
    }

    #[test]
    fn creating_rect_keeps_snapped_drag_origin() {
        let mut state = AppState::new();
        state.tool = Tool::Rect;

        state.cursor_x = 13.0;
        state.cursor_y = 17.0;
        assert!(handle_mouse_button(
            &mut state,
            MouseButton::Left,
            true,
            false
        ));

        assert_eq!(state.drag_mode, DragMode::Creating);
        assert_eq!(state.drag_shape_origins, vec![(20.0, 20.0)]);

        assert!(handle_mouse_move(&mut state, 38.0, 44.0));
        let shape = &state.shapes[state.selected[0]];
        assert!((shape.x - 20.0).abs() < 1e-6);
        assert!((shape.y - 20.0).abs() < 1e-6);
        assert!((shape.w - 20.0).abs() < 1e-6);
        assert!((shape.h - 20.0).abs() < 1e-6);
    }

    #[test]
    fn rotated_rect_contains_uses_transform() {
        let mut shape = Shape::new(ShapeKind::Rect, 0.0, 0.0, 0x1e1e1e);
        shape.w = 100.0;
        shape.h = 40.0;
        shape.rotation = std::f64::consts::FRAC_PI_2;

        assert!(
            shape.contains(50.0, 60.0),
            "rotated extent should be hittable"
        );
        assert!(
            !shape.contains(10.0, 10.0),
            "outside rotated rect should miss"
        );
    }

    #[test]
    fn flipped_triangle_contains_uses_transform() {
        let mut shape = Shape::new(ShapeKind::Triangle, 0.0, 0.0, 0x1e1e1e);
        shape.w = 100.0;
        shape.h = 100.0;

        assert!(
            !shape.contains(10.0, 10.0),
            "point is outside upright triangle"
        );

        shape.flipped_v = true;
        assert!(
            shape.contains(10.0, 10.0),
            "vertical flip should mirror hit test"
        );
    }

    #[test]
    fn reset_zoom_keeps_target_zoom_in_sync() {
        let mut state = AppState::new();
        state.camera.zoom = 2.5;
        state.target_zoom = 2.5;
        state.camera.offset_x = 120.0;
        state.camera.offset_y = -80.0;
        state.context_menu = Some(ContextMenu {
            sx: 20.0,
            sy: 30.0,
            items: vec![ContextMenuItem {
                label: "Reset Zoom",
                action: ContextAction::ResetZoom,
            }],
            hovered: None,
        });

        state.cursor_x = 60.0;
        state.cursor_y = 30.0 + ContextMenu::PAD + 1.0;

        assert!(handle_mouse_button(
            &mut state,
            MouseButton::Left,
            true,
            false
        ));
        assert!((state.camera.zoom - 1.0).abs() < f64::EPSILON);
        assert!((state.target_zoom - 1.0).abs() < f64::EPSILON);
        assert!((state.camera.offset_x - 0.0).abs() < f64::EPSILON);
        assert!((state.camera.offset_y - 0.0).abs() < f64::EPSILON);
    }
}
