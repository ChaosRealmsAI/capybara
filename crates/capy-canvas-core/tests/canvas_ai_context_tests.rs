mod common;
use common::*;

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
    fn imported_generated_image_exports_generation_metadata() {
        let mut state = AppState::new();
        let idx =
            state.import_image_asset_bytes(capy_canvas_core::state_shapes::ImageAssetImport {
                x: 10.0,
                y: 20.0,
                rgba: std::sync::Arc::new(vec![255, 0, 0, 255]),
                width: 1,
                height: 1,
                mime: "image/png".to_string(),
                title: Some("Hero generated".to_string()),
                source_path: Some("/tmp/hero.png".to_string()),
                generation_provider: Some("apimart-gpt-image-2".to_string()),
                generation_prompt: Some("Scene: warm studio".to_string()),
            });
        state.selected = vec![idx];

        let context = state.selected_context();
        assert_eq!(context.items[0].title, "Hero generated");
        assert_eq!(
            context.items[0].source_path.as_deref(),
            Some("/tmp/hero.png")
        );
        assert_eq!(
            context.items[0].generation_provider.as_deref(),
            Some("apimart-gpt-image-2")
        );

        let snapshot = state.ai_snapshot();
        assert_eq!(
            snapshot.nodes[0].generation_prompt.as_deref(),
            Some("Scene: warm studio")
        );
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

    #[test]
    fn id_based_move_keeps_highlighter_points_with_frame() {
        let mut state = AppState::new();
        let mut shape = Shape::new(ShapeKind::Highlighter, 100.0, 80.0, 0xfbbf24);
        shape.w = 160.0;
        shape.h = 70.0;
        shape.points = vec![(112.0, 90.0), (180.0, 118.0), (258.0, 148.0)];
        let idx = state.add_shape(shape);
        let id = state.shapes[idx].id;

        state
            .move_shape_by_id(id, 220.0, 150.0)
            .expect("move by id");

        let moved = &state.shapes[idx];
        assert_eq!(moved.x, 220.0);
        assert_eq!(moved.y, 150.0);
        assert!((moved.points[0].0 - 232.0).abs() < 1e-6);
        assert!((moved.points[0].1 - 160.0).abs() < 1e-6);
        assert!((moved.points[2].0 - 378.0).abs() < 1e-6);
        assert!((moved.points[2].1 - 218.0).abs() < 1e-6);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Selection tests
// (from tldraw SelectTool.test.ts, select.test.tsx, excalidraw selection.test.ts)
// ═══════════════════════════════════════════════════════════════════════
