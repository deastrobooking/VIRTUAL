//! Keep text editing and modified keys separate from performance controls.

use winit::keyboard::{KeyCode, ModifiersState};

pub(crate) fn allowed(code: KeyCode, modifiers: ModifiersState, editing: bool) -> bool {
    let command = modifiers.control_key() || modifiers.super_key();
    if command && !modifiers.alt_key() {
        return code == KeyCode::KeyS
            || (code == KeyCode::KeyZ && !editing)
            || (code == KeyCode::KeyO && !modifiers.shift_key());
    }
    !editing && modifiers.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_editing_never_triggers_performance_controls() {
        for key in [
            KeyCode::KeyB,
            KeyCode::KeyO,
            KeyCode::Space,
            KeyCode::Digit1,
            KeyCode::Digit8,
            KeyCode::ArrowLeft,
            KeyCode::ArrowRight,
            KeyCode::Home,
            KeyCode::Delete,
            KeyCode::Backspace,
            KeyCode::Escape,
        ] {
            assert!(!allowed(key, ModifiersState::empty(), true), "{key:?}");
            assert!(allowed(key, ModifiersState::empty(), false), "{key:?}");
            for modifier in [
                ModifiersState::CONTROL,
                ModifiersState::SUPER,
                ModifiersState::ALT,
                ModifiersState::SHIFT,
            ] {
                if key != KeyCode::KeyO {
                    assert!(!allowed(key, modifier, false), "{key:?}");
                }
            }
        }
    }

    #[test]
    fn timeline_undo_leaves_text_editing_alone() {
        assert!(!allowed(KeyCode::KeyZ, ModifiersState::SUPER, true));
        assert!(allowed(KeyCode::KeyZ, ModifiersState::SUPER, false));
    }

    #[test]
    fn document_shortcuts_remain_global() {
        for modifier in [ModifiersState::CONTROL, ModifiersState::SUPER] {
            assert!(allowed(KeyCode::KeyS, modifier, true));
            assert!(allowed(
                KeyCode::KeyS,
                modifier | ModifiersState::SHIFT,
                true
            ));
            assert!(allowed(KeyCode::KeyO, modifier, true));
            assert!(!allowed(
                KeyCode::KeyO,
                modifier | ModifiersState::SHIFT,
                true
            ));
            assert!(!allowed(
                KeyCode::KeyS,
                modifier | ModifiersState::ALT,
                true
            ));
        }
    }
}
