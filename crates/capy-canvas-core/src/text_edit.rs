//! Text editing helpers shared by keyboard input and shape-creation flows.

use crate::state::{AppState, TextEditState, TextTarget};

pub(crate) fn begin_text_edit(state: &mut AppState, shape_index: usize) {
    let cursor = state.shapes[shape_index].text.chars().count();
    begin_edit_target(state, shape_index, TextTarget::Body, cursor);
}

pub(crate) fn begin_label_edit(state: &mut AppState, shape_index: usize) {
    let cursor = state.shapes[shape_index]
        .label
        .as_deref()
        .unwrap_or("")
        .chars()
        .count();
    begin_edit_target(state, shape_index, TextTarget::Label, cursor);
}

fn begin_edit_target(state: &mut AppState, shape_index: usize, target: TextTarget, cursor: usize) {
    state.text_edit = Some(TextEditState {
        shape_index,
        target,
        cursor,
        blink_visible: true,
        selection_start: None,
    });
    state.selected = vec![shape_index];
}

pub(crate) fn char_to_byte_index(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(byte_i, _)| byte_i)
        .unwrap_or(s.len())
}

pub(crate) fn cursor_to_line_col(text: &str, cursor: usize) -> (usize, usize) {
    let mut line = 0;
    let mut col = 0;
    for (i, ch) in text.chars().enumerate() {
        if i == cursor {
            return (line, col);
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line, col)
}

pub(crate) fn line_col_to_cursor(text: &str, target_line: usize, target_col: usize) -> usize {
    let mut cursor = 0;
    for (line_idx, line) in text.split('\n').enumerate() {
        if line_idx == target_line {
            let line_len = line.chars().count();
            return cursor + target_col.min(line_len);
        }
        cursor += line.chars().count() + 1;
    }
    text.chars().count()
}

pub(crate) fn edit_buffer(
    state: &AppState,
    shape_index: usize,
    target: TextTarget,
) -> Option<&str> {
    let shape = state.shapes.get(shape_index)?;
    Some(match target {
        TextTarget::Body => &shape.text,
        TextTarget::Label => shape.label.as_deref().unwrap_or(""),
    })
}

pub(crate) fn edit_buffer_mut(
    state: &mut AppState,
    shape_index: usize,
    target: TextTarget,
) -> Option<&mut String> {
    let shape = state.shapes.get_mut(shape_index)?;
    Some(match target {
        TextTarget::Body => &mut shape.text,
        TextTarget::Label => shape.label.get_or_insert_with(String::new),
    })
}
