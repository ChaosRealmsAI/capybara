mod common;
use common::*;

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
