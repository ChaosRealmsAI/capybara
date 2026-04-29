mod common;
use common::*;

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
