//! Input handling entry points.
//!
//! Text editing and character shortcut logic lives in `input_keys`.
//! Mouse flow lives in `mouse`.
//! Tool-specific handlers live in `input_tools`.

use winit::keyboard::{Key, NamedKey};

use crate::input_keys::{handle_char_key, handle_text_edit_key};
use crate::state::AppState;

// Re-export for external callers that use input::toolbar_hit etc.
pub use crate::input_tools::{color_picker_hit_with_viewport, toolbar_hit};
pub use crate::mouse::{
    handle_double_click, handle_mouse_button, handle_mouse_move, handle_scroll,
};

/// Handle key press. Returns true if state changed (needs redraw).
pub fn handle_key(
    state: &mut AppState,
    key: &Key,
    pressed: bool,
    modifiers: winit::event::Modifiers,
) -> bool {
    if !pressed {
        if matches!(key, Key::Named(NamedKey::Space)) {
            state.space_held = false;
        }
        if matches!(key, Key::Named(NamedKey::Alt)) {
            state.alt_held = false;
        }
        return false;
    }

    let cmd = modifiers.state().super_key();
    let shift = modifiers.state().shift_key();

    if matches!(key, Key::Named(NamedKey::Escape)) && state.context_menu.is_some() {
        state.context_menu = None;
        return true;
    }

    if matches!(key, Key::Named(NamedKey::Alt)) {
        state.alt_held = true;
        return false;
    }

    // Text editing mode
    if let Some(ref mut te) = state.text_edit {
        let idx = te.shape_index;
        if idx >= state.shapes.len() {
            state.text_edit = None;
            return true;
        }
        return handle_text_edit_key(state, key, cmd, shift, idx);
    }

    match key {
        Key::Named(NamedKey::Space) => {
            state.space_held = true;
            true
        }
        Key::Named(NamedKey::Backspace) | Key::Named(NamedKey::Delete) => {
            if let Some(conn_idx) = state.selected_connector {
                if conn_idx < state.connectors.len() {
                    state.push_undo();
                    state.connectors.remove(conn_idx);
                    state.selected_connector = None;
                    return true;
                }
            }
            state.delete_selected();
            true
        }
        Key::Named(NamedKey::Escape) => {
            state.selected.clear();
            state.connector_from = None;
            state.selected_connector = None;
            true
        }
        Key::Character(ch) => handle_char_key(state, ch.as_str(), cmd, shift),
        _ => false,
    }
}
