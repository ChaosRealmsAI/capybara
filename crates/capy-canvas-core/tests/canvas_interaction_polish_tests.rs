mod common;
use common::*;

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
