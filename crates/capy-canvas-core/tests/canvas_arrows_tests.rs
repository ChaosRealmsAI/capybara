mod common;
use common::*;

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
