mod common;
use common::*;

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
