//! Keyboard input handlers: text editing and character key shortcuts.

use winit::keyboard::{Key, NamedKey};

use crate::state::{AppState, TextTarget, Tool};
use crate::text_edit::{
    char_to_byte_index, cursor_to_line_col, edit_buffer, edit_buffer_mut, line_col_to_cursor,
};

/// Handle text editing key press (shape is in text edit mode).
pub(crate) fn handle_text_edit_key(
    state: &mut AppState,
    key: &Key,
    cmd: bool,
    shift: bool,
    idx: usize,
) -> bool {
    let Some(target) = state.text_edit.as_ref().map(|te| te.target) else {
        return false;
    };
    match key {
        Key::Named(NamedKey::Escape) => {
            state.text_edit = None;
            true
        }
        Key::Named(NamedKey::Backspace) => {
            let Some(te) = state.text_edit.as_mut() else {
                return false;
            };
            if let Some((a, b)) = te.selection_range() {
                if a != b {
                    let Some(text) = edit_buffer_mut(state, idx, target) else {
                        return false;
                    };
                    let byte_a = char_to_byte_index(text, a);
                    let byte_b = char_to_byte_index(text, b);
                    text.replace_range(byte_a..byte_b, "");
                    let Some(te) = state.text_edit.as_mut() else {
                        return true;
                    };
                    te.cursor = a;
                    te.clear_selection();
                    return true;
                }
            }
            let cursor = state.text_edit.as_ref().map(|te| te.cursor).unwrap_or(0);
            if cursor > 0 {
                let Some(text) = edit_buffer_mut(state, idx, target) else {
                    return false;
                };
                let byte_idx = char_to_byte_index(text, cursor - 1);
                let byte_end = char_to_byte_index(text, cursor);
                text.replace_range(byte_idx..byte_end, "");
                let Some(te) = state.text_edit.as_mut() else {
                    return true;
                };
                te.cursor -= 1;
                te.clear_selection();
            }
            true
        }
        Key::Named(NamedKey::Enter) => {
            if target == TextTarget::Label {
                state.text_edit = None;
                return true;
            }
            let cursor = state.text_edit.as_ref().map(|te| te.cursor).unwrap_or(0);
            let Some(text) = edit_buffer_mut(state, idx, target) else {
                return false;
            };
            let byte_idx = char_to_byte_index(text, cursor);
            text.insert(byte_idx, '\n');
            let Some(te) = state.text_edit.as_mut() else {
                return true;
            };
            te.cursor += 1;
            te.clear_selection();
            true
        }
        Key::Named(NamedKey::ArrowLeft) => {
            let Some(te) = state.text_edit.as_mut() else {
                return false;
            };
            if te.cursor > 0 {
                if shift {
                    if te.selection_start.is_none() {
                        te.selection_start = Some(te.cursor);
                    }
                } else {
                    te.clear_selection();
                }
                te.cursor -= 1;
            }
            true
        }
        Key::Named(NamedKey::ArrowRight) => {
            let char_count = edit_buffer(state, idx, target)
                .unwrap_or("")
                .chars()
                .count();
            let Some(te) = state.text_edit.as_mut() else {
                return false;
            };
            if te.cursor < char_count {
                if shift {
                    if te.selection_start.is_none() {
                        te.selection_start = Some(te.cursor);
                    }
                } else {
                    te.clear_selection();
                }
                te.cursor += 1;
            }
            true
        }
        Key::Named(NamedKey::ArrowUp) => {
            if target == TextTarget::Label {
                return true;
            }
            let text = edit_buffer(state, idx, target).unwrap_or("").to_string();
            let Some(te) = state.text_edit.as_ref() else {
                return false;
            };
            let (line, col) = cursor_to_line_col(&text, te.cursor);
            if line > 0 {
                let Some(te) = state.text_edit.as_mut() else {
                    return true;
                };
                if shift && te.selection_start.is_none() {
                    te.selection_start = Some(te.cursor);
                } else if !shift {
                    te.clear_selection();
                }
                te.cursor = line_col_to_cursor(&text, line - 1, col);
            }
            true
        }
        Key::Named(NamedKey::ArrowDown) => {
            if target == TextTarget::Label {
                return true;
            }
            let text = edit_buffer(state, idx, target).unwrap_or("").to_string();
            let lines: Vec<&str> = text.split('\n').collect();
            let Some(te) = state.text_edit.as_ref() else {
                return false;
            };
            let (line, col) = cursor_to_line_col(&text, te.cursor);
            if line + 1 < lines.len() {
                let Some(te) = state.text_edit.as_mut() else {
                    return true;
                };
                if shift && te.selection_start.is_none() {
                    te.selection_start = Some(te.cursor);
                } else if !shift {
                    te.clear_selection();
                }
                te.cursor = line_col_to_cursor(&text, line + 1, col);
            }
            true
        }
        Key::Character(ch) => {
            let s = ch.as_str();
            if cmd && s == "a" {
                let char_count = edit_buffer(state, idx, target)
                    .unwrap_or("")
                    .chars()
                    .count();
                let Some(te) = state.text_edit.as_mut() else {
                    return false;
                };
                te.selection_start = Some(0);
                te.cursor = char_count;
                return true;
            }
            let Some(te) = state.text_edit.as_mut() else {
                return false;
            };
            if let Some((a, b)) = te.selection_range() {
                if a != b {
                    let Some(text) = edit_buffer_mut(state, idx, target) else {
                        return false;
                    };
                    let byte_a = char_to_byte_index(text, a);
                    let byte_b = char_to_byte_index(text, b);
                    text.replace_range(byte_a..byte_b, "");
                    let Some(te) = state.text_edit.as_mut() else {
                        return true;
                    };
                    te.cursor = a;
                    te.clear_selection();
                }
            }
            let Some(te) = state.text_edit.as_ref() else {
                return false;
            };
            let cursor = te.cursor;
            let Some(text) = edit_buffer_mut(state, idx, target) else {
                return false;
            };
            let byte_idx = char_to_byte_index(text, cursor);
            text.insert_str(byte_idx, s);
            let Some(te) = state.text_edit.as_mut() else {
                return true;
            };
            te.cursor += s.chars().count();
            true
        }
        _ => false,
    }
}

/// Handle character key shortcuts (non-text-editing mode).
pub(crate) fn handle_char_key(state: &mut AppState, s: &str, cmd: bool, shift: bool) -> bool {
    if cmd {
        match s {
            "z" if shift => {
                state.redo();
                return true;
            }
            "z" => {
                state.undo();
                return true;
            }
            "e" if shift => {
                let svg = state.export_svg();
                state.pending_svg_export = Some(svg);
                return true;
            }
            "e" => {
                let svg = state.export_svg();
                state.pending_svg_export = Some(svg);
                state.show_toast("Exporting SVG...", 2000);
                return true;
            }
            "1" => {
                state.zoom_fit();
                return true;
            }
            "2" => {
                state.zoom_selection();
                return true;
            }
            "a" => {
                state.selected = (0..state.shapes.len()).collect();
                return true;
            }
            "c" => {
                state.copy_selected();
                state.show_toast("Copied to clipboard", 1500);
                return true;
            }
            "x" => {
                state.cut_selected();
                return true;
            }
            "v" => {
                let (wx, wy) = state.camera.screen_to_world(state.cursor_x, state.cursor_y);
                state.paste_at(wx, wy);
                return true;
            }
            "d" if shift => {
                state.dark_mode = !state.dark_mode;
                return true;
            }
            "d" => {
                state.duplicate_selected(20.0, 20.0);
                return true;
            }
            "g" if shift => {
                state.ungroup_selected();
                return true;
            }
            "g" => {
                state.group_selected();
                return true;
            }
            "s" => {
                state.pending_save_request = true;
                return true;
            }
            "o" => {
                state.pending_load_request = true;
                return true;
            }
            "=" | "+" => {
                state.zoom_in();
                return true;
            }
            "-" => {
                state.zoom_out();
                return true;
            }
            "/" => {
                state.show_help = !state.show_help;
                return true;
            }
            _ => return false,
        }
    }
    if shift && !state.selected.is_empty() {
        match s {
            "H" => {
                state.flip_selected_h();
                return true;
            }
            "V" => {
                state.flip_selected_v();
                return true;
            }
            _ => {}
        }
    }
    // Tool shortcuts
    match s {
        "v" | "V" => state.tool = Tool::Select,
        "r" | "R" => state.tool = Tool::Rect,
        "e" | "E" => state.tool = Tool::Ellipse,
        "g" | "G" => state.tool = Tool::Triangle,
        "b" | "B" => state.tool = Tool::Diamond,
        "l" | "L" => state.tool = Tool::Line,
        "a" | "A" => state.tool = Tool::Arrow,
        "d" | "D" => state.tool = Tool::Freehand,
        "h" | "H" => state.tool = Tool::Highlighter,
        "s" | "S" => state.tool = Tool::StickyNote,
        "t" | "T" => state.tool = Tool::Text,
        "x" | "X" => state.tool = Tool::Eraser,
        "q" | "Q" => state.tool = Tool::Lasso,
        _ => return false,
    }
    state.connector_from = None;
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Shape, ShapeKind};
    use crate::text_edit::begin_label_edit;

    #[test]
    fn label_edit_writes_into_arrow_label() {
        let mut state = AppState::new();
        let mut arrow = Shape::new(ShapeKind::Arrow, 0.0, 0.0, 0);
        arrow.w = 100.0;
        let idx = state.add_shape(arrow);
        begin_label_edit(&mut state, idx);

        assert!(handle_text_edit_key(
            &mut state,
            &Key::Character("x".into()),
            false,
            false,
            idx,
        ));
        assert_eq!(state.shapes[idx].label.as_deref(), Some("x"));
    }
}
