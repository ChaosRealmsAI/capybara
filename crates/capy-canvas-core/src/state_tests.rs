//! Unit tests — core state logic
//! Extracted from tldraw & excalidraw test suites.

use super::*;

// ── Shape::contains (hit testing) ──

/// excalidraw bounds.test.ts: rect absolute coords x1 = element.x
#[test]
fn shape_rect_contains_center() {
    let mut s = Shape::new(ShapeKind::Rect, 10.0, 20.0, 0x1e1e1e);
    s.w = 100.0;
    s.h = 80.0;
    assert!(s.contains(50.0, 50.0), "center of rect should be inside");
}

#[test]
fn shape_rect_excludes_outside() {
    let mut s = Shape::new(ShapeKind::Rect, 10.0, 20.0, 0x1e1e1e);
    s.w = 100.0;
    s.h = 80.0;
    assert!(!s.contains(200.0, 200.0), "point far outside rect");
}

#[test]
fn shape_rect_includes_edges() {
    let mut s = Shape::new(ShapeKind::Rect, 10.0, 20.0, 0x1e1e1e);
    s.w = 100.0;
    s.h = 80.0;
    assert!(s.contains(10.0, 20.0), "top-left corner");
    assert!(s.contains(110.0, 100.0), "bottom-right corner");
    assert!(s.contains(10.0, 50.0), "left edge");
    assert!(s.contains(110.0, 50.0), "right edge");
}

/// excalidraw: ellipse hit test uses normalized distance formula.
#[test]
fn shape_ellipse_contains_center() {
    let mut s = Shape::new(ShapeKind::Ellipse, 0.0, 0.0, 0x1e1e1e);
    s.w = 100.0;
    s.h = 80.0;
    assert!(s.contains(50.0, 40.0), "center of ellipse");
}

#[test]
fn shape_ellipse_excludes_corners() {
    let mut s = Shape::new(ShapeKind::Ellipse, 0.0, 0.0, 0x1e1e1e);
    s.w = 100.0;
    s.h = 80.0;
    // Corners of bounding box are outside the ellipse
    assert!(!s.contains(2.0, 2.0), "top-left corner outside ellipse");
    assert!(
        !s.contains(98.0, 78.0),
        "bottom-right corner outside ellipse"
    );
}

#[test]
fn shape_ellipse_zero_radius_returns_false() {
    let s = Shape::new(ShapeKind::Ellipse, 0.0, 0.0, 0x1e1e1e);
    // w=0, h=0 -> rx=0, ry=0
    assert!(
        !s.contains(0.0, 0.0),
        "zero-size ellipse should never contain"
    );
}

/// excalidraw: line hit within 5px threshold.
#[test]
fn shape_line_contains_near_point() {
    let mut s = Shape::new(ShapeKind::Line, 0.0, 0.0, 0x1e1e1e);
    s.w = 100.0;
    s.h = 100.0;
    // Point on the diagonal
    assert!(s.contains(50.0, 50.0), "point on line");
    // Point 3px off diagonal (within 5px threshold)
    assert!(s.contains(50.0, 53.0), "point 3px from line");
}

#[test]
fn shape_line_excludes_far_point() {
    let mut s = Shape::new(ShapeKind::Line, 0.0, 0.0, 0x1e1e1e);
    s.w = 100.0;
    s.h = 100.0;
    // Point 20px off diagonal
    assert!(!s.contains(50.0, 70.0), "point >5px from line");
}

/// Arrow uses same hit test as line.
#[test]
fn shape_arrow_contains_near_point() {
    let mut s = Shape::new(ShapeKind::Arrow, 10.0, 10.0, 0x1e1e1e);
    s.w = 80.0;
    s.h = 60.0;
    // Midpoint of the arrow line
    let mid_x = 10.0 + 40.0;
    let mid_y = 10.0 + 30.0;
    assert!(s.contains(mid_x, mid_y), "midpoint of arrow");
}

// ── point_to_segment_dist ──

#[test]
fn point_to_segment_on_line() {
    let d = point_to_segment_dist(5.0, 5.0, 0.0, 0.0, 10.0, 10.0);
    assert!(d < 1e-6, "point on segment should have 0 distance, got {d}");
}

#[test]
fn point_to_segment_perpendicular() {
    // Horizontal segment from (0,0) to (10,0), point at (5, 3)
    let d = point_to_segment_dist(5.0, 3.0, 0.0, 0.0, 10.0, 0.0);
    assert!(
        (d - 3.0).abs() < 1e-6,
        "perpendicular distance should be 3, got {d}"
    );
}

#[test]
fn point_to_segment_beyond_endpoint() {
    // Segment from (0,0) to (10,0), point at (15, 0)
    let d = point_to_segment_dist(15.0, 0.0, 0.0, 0.0, 10.0, 0.0);
    assert!(
        (d - 5.0).abs() < 1e-6,
        "distance to nearest endpoint should be 5, got {d}"
    );
}

#[test]
fn point_to_zero_length_segment() {
    let d = point_to_segment_dist(3.0, 4.0, 0.0, 0.0, 0.0, 0.0);
    assert!(
        (d - 5.0).abs() < 1e-6,
        "distance to zero-length segment = point distance"
    );
}

// ── Shape::center ──

#[test]
fn shape_center() {
    let mut s = Shape::new(ShapeKind::Rect, 10.0, 20.0, 0x1e1e1e);
    s.w = 100.0;
    s.h = 80.0;
    let (cx, cy) = s.center();
    assert!((cx - 60.0).abs() < 1e-6);
    assert!((cy - 60.0).abs() < 1e-6);
}

// ── Shape::edge_point ──

/// excalidraw binding.test.tsx: edge_point returns boundary intersection.
#[test]
fn edge_point_rightward() {
    let mut s = Shape::new(ShapeKind::Rect, 0.0, 0.0, 0x1e1e1e);
    s.w = 100.0;
    s.h = 100.0;
    let (ex, ey) = s.edge_point(200.0, 50.0);
    assert!((ex - 100.0).abs() < 1e-6, "edge x should be 100, got {ex}");
    assert!((ey - 50.0).abs() < 1e-6, "edge y should be 50, got {ey}");
}

#[test]
fn edge_point_downward() {
    let mut s = Shape::new(ShapeKind::Rect, 0.0, 0.0, 0x1e1e1e);
    s.w = 100.0;
    s.h = 100.0;
    let (ex, ey) = s.edge_point(50.0, 200.0);
    assert!((ex - 50.0).abs() < 1e-6, "edge x should be 50, got {ex}");
    assert!((ey - 100.0).abs() < 1e-6, "edge y should be 100, got {ey}");
}

#[test]
fn edge_point_from_center_returns_center() {
    let mut s = Shape::new(ShapeKind::Rect, 0.0, 0.0, 0x1e1e1e);
    s.w = 100.0;
    s.h = 100.0;
    let (ex, ey) = s.edge_point(50.0, 50.0);
    assert!((ex - 50.0).abs() < 1e-6);
    assert!((ey - 50.0).abs() < 1e-6);
}

// ── Camera ──

/// tldraw HandTool.test.ts: initial camera state.
#[test]
fn camera_default() {
    let c = Camera::default();
    assert!((c.offset_x).abs() < 1e-6);
    assert!((c.offset_y).abs() < 1e-6);
    assert!((c.zoom - 1.0).abs() < 1e-6);
}

/// tldraw: pan moves offset by delta.
#[test]
fn camera_pan() {
    let mut c = Camera::default();
    c.pan(10.0, 20.0);
    assert!((c.offset_x - 10.0).abs() < 1e-6);
    assert!((c.offset_y - 20.0).abs() < 1e-6);
}

/// tldraw: zoom_at preserves world position under cursor.
#[test]
fn camera_zoom_at_preserves_cursor_world_pos() {
    let mut c = Camera::default();
    let sx = 400.0;
    let sy = 300.0;
    let (wx0, wy0) = c.screen_to_world(sx, sy);
    c.zoom_at(sx, sy, 2.0);
    let (wx1, wy1) = c.screen_to_world(sx, sy);
    assert!((wx0 - wx1).abs() < 1e-6);
    assert!((wy0 - wy1).abs() < 1e-6);
}

/// Camera zoom is clamped.
#[test]
fn camera_zoom_clamp() {
    let mut c = Camera::default();
    c.zoom_at(0.0, 0.0, 100.0);
    assert!(c.zoom <= 10.0);
    c.zoom_at(0.0, 0.0, 0.001);
    assert!(c.zoom >= 0.1);
}

#[test]
fn camera_screen_to_world() {
    let c = Camera {
        offset_x: 50.0,
        offset_y: 30.0,
        zoom: 2.0,
    };
    let (wx, wy) = c.screen_to_world(150.0, 130.0);
    assert!((wx - 50.0).abs() < 1e-6); // (150-50)/2
    assert!((wy - 50.0).abs() < 1e-6); // (130-30)/2
}

// ── AppState ──

#[test]
fn add_shape_assigns_unique_ids() {
    let mut state = AppState::new();
    let a = state.add_shape(Shape::new(ShapeKind::Rect, 0.0, 0.0, 0));
    let b = state.add_shape(Shape::new(ShapeKind::Rect, 0.0, 0.0, 0));
    assert_ne!(state.shapes[a].id, state.shapes[b].id);
}

#[test]
fn create_content_card_sets_ai_metadata_and_selection() {
    let mut state = AppState::new();
    let idx = state.create_content_card(CanvasContentKind::Video, "Launch storyboard", 80.0, 120.0);
    let shape = &state.shapes[idx];
    assert_eq!(shape.kind, ShapeKind::StickyNote);
    assert_eq!(shape.content_kind(), CanvasContentKind::Video);
    assert_eq!(shape.display_title(), "Launch storyboard");
    assert_eq!(shape.metadata.status.as_deref(), Some("briefing"));
    assert!(
        shape
            .metadata
            .editor_route
            .as_deref()
            .unwrap_or_default()
            .contains("capy://canvas/video/")
    );
    assert_eq!(state.selected, vec![idx]);

    let snapshot = state.ai_snapshot();
    assert_eq!(snapshot.nodes.len(), 1);
    assert_eq!(snapshot.nodes[0].content_kind, CanvasContentKind::Video);
    assert!(
        snapshot.nodes[0]
            .available_actions
            .contains(&"open_detail".to_string())
    );
    assert_eq!(snapshot.selection.selected_count, 1);

    let brand_idx =
        state.create_content_card(CanvasContentKind::Brand, "Brand system", 440.0, 120.0);
    assert_ne!(
        state.shapes[idx].color, state.shapes[brand_idx].color,
        "content cards must not all render as identical yellow placeholders"
    );
}

/// excalidraw zindex: shapes are ordered by creation time.
#[test]
fn hit_test_returns_topmost() {
    let mut state = AppState::new();
    let mut s1 = Shape::new(ShapeKind::Rect, 0.0, 0.0, 0);
    s1.w = 100.0;
    s1.h = 100.0;
    state.add_shape(s1);
    let mut s2 = Shape::new(ShapeKind::Rect, 50.0, 50.0, 0);
    s2.w = 100.0;
    s2.h = 100.0;
    state.add_shape(s2);
    assert_eq!(state.hit_test(75.0, 75.0), Some(1));
}

#[test]
fn hit_test_returns_none_on_miss() {
    let mut state = AppState::new();
    let mut s = Shape::new(ShapeKind::Rect, 100.0, 100.0, 0);
    s.w = 50.0;
    s.h = 50.0;
    state.add_shape(s);
    assert_eq!(state.hit_test(0.0, 0.0), None);
}

// ── Undo / Redo ──

/// tldraw/excalidraw: create -> undo -> shapes gone.
#[test]
fn undo_restores_previous_state() {
    let mut state = AppState::new();
    state.push_undo();
    let mut s = Shape::new(ShapeKind::Rect, 0.0, 0.0, 0);
    s.w = 50.0;
    s.h = 50.0;
    state.add_shape(s);
    assert_eq!(state.shapes.len(), 1);
    state.undo();
    assert_eq!(state.shapes.len(), 0);
}

#[test]
fn redo_restores_undone_state() {
    let mut state = AppState::new();
    state.push_undo();
    let mut s = Shape::new(ShapeKind::Rect, 0.0, 0.0, 0);
    s.w = 50.0;
    s.h = 50.0;
    state.add_shape(s);
    state.undo();
    state.redo();
    assert_eq!(state.shapes.len(), 1);
}

#[test]
fn undo_clears_selection() {
    let mut state = AppState::new();
    state.push_undo();
    state.add_shape(Shape::new(ShapeKind::Rect, 0.0, 0.0, 0));
    state.selected = vec![0];
    state.undo();
    assert!(state.selected.is_empty());
}

#[test]
fn new_action_clears_redo_stack() {
    let mut state = AppState::new();
    state.push_undo();
    state.add_shape(Shape::new(ShapeKind::Rect, 0.0, 0.0, 0));
    state.undo();
    assert!(!state.redo_stack.is_empty());
    state.push_undo();
    assert!(state.redo_stack.is_empty());
}

// ── Delete ──

/// excalidraw: delete removes shape and clears selection.
#[test]
fn delete_selected_removes_shapes() {
    let mut state = AppState::new();
    state.add_shape(Shape::new(ShapeKind::Rect, 0.0, 0.0, 0));
    state.add_shape(Shape::new(ShapeKind::Rect, 100.0, 100.0, 0));
    state.selected = vec![0];
    state.delete_selected();
    assert_eq!(state.shapes.len(), 1);
    assert!(state.selected.is_empty());
}

/// excalidraw binding: deleting shape removes connected connectors.
#[test]
fn delete_removes_connectors() {
    let mut state = AppState::new();
    state.add_shape(Shape::new(ShapeKind::Rect, 0.0, 0.0, 0));
    state.add_shape(Shape::new(ShapeKind::Rect, 200.0, 200.0, 0));
    let id_a = state.shapes[0].id;
    let id_b = state.shapes[1].id;
    state.connectors.push(Connector {
        from_id: id_a,
        to_id: id_b,
        color: 0,
        style: ConnectorStyle::default(),
        label: None,
    });
    state.selected = vec![0];
    state.delete_selected();
    assert!(state.connectors.is_empty());
}

#[test]
fn delete_empty_selection_is_noop() {
    let mut state = AppState::new();
    state.add_shape(Shape::new(ShapeKind::Rect, 0.0, 0.0, 0));
    state.selected.clear();
    state.delete_selected();
    assert_eq!(
        state.shapes.len(),
        1,
        "should not delete when nothing selected"
    );
}

// ── Alignment guides ──

/// tldraw grid-align: guides detect alignment within threshold.
#[test]
fn alignment_guides_within_threshold() {
    let mut state = AppState::new();
    let mut s1 = Shape::new(ShapeKind::Rect, 100.0, 0.0, 0);
    s1.w = 50.0;
    s1.h = 50.0;
    state.add_shape(s1);
    let mut s2 = Shape::new(ShapeKind::Rect, 103.0, 100.0, 0);
    s2.w = 50.0;
    s2.h = 50.0;
    state.add_shape(s2);
    let guides = state.alignment_guides(&[1]);
    assert!(!guides.is_empty(), "should find alignment guide within 5px");
}

#[test]
fn alignment_guides_none_when_far() {
    let mut state = AppState::new();
    let mut s1 = Shape::new(ShapeKind::Rect, 0.0, 0.0, 0);
    s1.w = 50.0;
    s1.h = 50.0;
    state.add_shape(s1);
    let mut s2 = Shape::new(ShapeKind::Rect, 500.0, 500.0, 0);
    s2.w = 50.0;
    s2.h = 50.0;
    state.add_shape(s2);
    let guides = state.alignment_guides(&[1]);
    assert!(guides.is_empty(), "no guide when shapes far apart");
}

// ── Tool ──

#[test]
fn tool_labels_are_nonempty() {
    for tool in Tool::all_toolbar() {
        assert!(!tool.label().is_empty());
    }
}

#[test]
fn tool_shortcuts_are_nonempty() {
    for tool in Tool::all_toolbar() {
        assert!(!tool.shortcut().is_empty());
    }
}
