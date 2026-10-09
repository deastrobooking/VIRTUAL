//! MIDI/OSC-mappable on-screen buttons.
//!
//! Every wrapped button has a stable, human-readable key such as
//! `deck.0.eject`, hashed into [`ControlTarget::UiButton`]. In MIDI Map mode
//! the button gets the same learn overlay as every other mappable control. A
//! mapped press clicks the button the next time it is drawn; presses for
//! buttons that are not on screen that frame (closed window, collapsed
//! section, hidden in Show Mode) are dropped rather than replayed later.
//!
//! The per-frame state lives in a thread local so wrapped buttons can sit in
//! any draw function, inside closures that already borrow `UiState`.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use virtual_core::ControlTarget;

use super::{MidiMapUi, UiAction, mappable};

/// Labels of every button drawn so far, for the MIDI Manager and feedback.
static LABELS: Mutex<BTreeMap<u64, String>> = Mutex::new(BTreeMap::new());

#[derive(Default)]
struct Frame {
    pending: BTreeSet<u64>,
    map: Option<MidiMapUi>,
    actions: Vec<UiAction>,
}

thread_local! {
    static FRAME: RefCell<Frame> = RefCell::new(Frame::default());
}

/// Stable target hash for a button key. Keys must never be renamed once
/// shipped, or saved mappings stop pointing at the button.
pub(crate) fn key_hash(key: &str) -> u64 {
    virtual_core::effect_parameter_key("ui-button", key)
}

#[cfg(test)]
pub(crate) fn target(key: &str) -> ControlTarget {
    ControlTarget::UiButton(key_hash(key))
}

/// Display label for a mapped button, or its hash before it has been drawn.
pub(super) fn label(key: u64) -> String {
    LABELS
        .lock()
        .ok()
        .and_then(|labels| labels.get(&key).cloned())
        .unwrap_or_else(|| format!("Button {key:016x}"))
}

/// Every button drawn since launch, for the MIDI Manager target list.
pub(super) fn known_targets() -> Vec<ControlTarget> {
    LABELS
        .lock()
        .map(|labels| {
            labels
                .keys()
                .map(|key| ControlTarget::UiButton(*key))
                .collect()
        })
        .unwrap_or_default()
}

/// Starts a UI frame: takes this frame's MIDI presses and the map state.
pub(super) fn begin_frame(pending: BTreeSet<u64>, map: MidiMapUi) {
    FRAME.with(|frame| {
        *frame.borrow_mut() = Frame {
            pending,
            map: Some(map),
            actions: Vec::new(),
        };
    });
}

/// Ends a UI frame, returning learn/clear actions and dropping unused presses.
pub(super) fn end_frame() -> Vec<UiAction> {
    FRAME.with(|frame| {
        let mut frame = frame.borrow_mut();
        frame.pending.clear();
        frame.map = None;
        std::mem::take(&mut frame.actions)
    })
}

/// This frame's map state, for wrapping a typed target with `mappable`
/// where no `MidiMapUi` is threaded through.
pub(super) fn current_map() -> Option<MidiMapUi> {
    FRAME.with(|frame| frame.borrow().map.clone())
}

/// Consumes a mapped press for an action whose button lives in a closed menu
/// or popup. Call it every frame the menu could be opened, after drawing any
/// open-menu item with the same key, so the press fires exactly once.
pub(super) fn pressed(key: &str, label: &str) -> bool {
    let hash = key_hash(key);
    register(hash, label);
    FRAME.with(|frame| {
        let mut frame = frame.borrow_mut();
        let armed = frame.map.as_ref().is_some_and(|map| map.active);
        frame.pending.remove(&hash) && !armed
    })
}

fn register(hash: u64, label: &str) {
    if let Ok(mut labels) = LABELS.lock()
        && labels.get(&hash).is_none_or(|existing| existing != label)
    {
        labels.insert(hash, label.to_owned());
    }
}

/// Draws a mappable widget and reports whether MIDI pressed it this frame.
fn mapped(
    ui: &mut egui::Ui,
    key: &str,
    label: &str,
    add: impl FnOnce(&mut egui::Ui) -> egui::Response,
) -> (egui::Response, bool) {
    let hash = key_hash(key);
    register(hash, label);
    let map = FRAME.with(|frame| frame.borrow().map.clone());
    let Some(map) = map else {
        return (add(ui), false);
    };
    let mut actions = Vec::new();
    let response = mappable(ui, &map, ControlTarget::UiButton(hash), &mut actions, add);
    let pressed = FRAME.with(|frame| {
        let mut frame = frame.borrow_mut();
        frame.actions.append(&mut actions);
        // Disabled buttons ignore presses, matching a mouse click.
        frame.pending.remove(&hash) && response.enabled() && !map.active
    });
    (response, pressed)
}

/// A button whose returned response reads as clicked when MIDI presses it.
pub(super) fn midi_button(
    ui: &mut egui::Ui,
    key: &str,
    label: &str,
    add: impl FnOnce(&mut egui::Ui) -> egui::Response,
) -> egui::Response {
    let (mut response, pressed) = mapped(ui, key, label, add);
    if pressed {
        response
            .flags
            .insert(egui::response::Flags::FAKE_PRIMARY_CLICKED);
    }
    response
}

/// A button that opens, closes or raises a window. Unlike [`midi_button`] it
/// stays usable in MIDI Map mode, so the operator can open the window holding
/// the controls they want to map. Shift-click maps the button itself and
/// right-click clears its mapping.
pub(super) fn midi_nav_button(
    ui: &mut egui::Ui,
    key: &str,
    label: &str,
    add: impl FnOnce(&mut egui::Ui) -> egui::Response,
) -> egui::Response {
    let hash = key_hash(key);
    register(hash, label);
    let target = ControlTarget::UiButton(hash);
    let map = current_map();
    let mut response = add(ui);
    let Some(map) = map else {
        return response;
    };
    if map.active && map.audio_learning.is_none() {
        let armed = map.learning == Some(target);
        let devices = map.devices_for(target);
        let stroke = if armed {
            egui::Stroke::new(2.0, map.palette.accent)
        } else if devices.is_empty() {
            egui::Stroke::new(1.0, map.palette.stroke)
        } else {
            egui::Stroke::new(1.0, map.palette.secondary)
        };
        ui.painter().rect_stroke(
            response.rect.expand(2.0),
            4.0,
            stroke,
            egui::StrokeKind::Outside,
        );
        let mut actions = Vec::new();
        if response.secondary_clicked() {
            actions.push(UiAction::MidiClearTarget(target));
        }
        if response.clicked() && ui.input(|input| input.modifiers.shift) {
            // Shift-click maps instead of navigating.
            response.flags.remove(
                egui::response::Flags::CLICKED | egui::response::Flags::FAKE_PRIMARY_CLICKED,
            );
            actions.push(if armed {
                UiAction::MidiCancelLearn
            } else {
                UiAction::MidiLearn(target)
            });
        }
        response = response.on_hover_text(if armed {
            "Armed · move a control on any connected device".to_owned()
        } else if devices.is_empty() {
            "Click to open · Shift-click to map to MIDI · right-click clears".to_owned()
        } else {
            format!(
                "Mapped to {} · click to open · Shift-click to remap · right-click clears",
                devices.join(", ")
            )
        });
        FRAME.with(|frame| frame.borrow_mut().actions.append(&mut actions));
    }
    let pressed = FRAME.with(|frame| frame.borrow_mut().pending.remove(&hash))
        && response.enabled()
        && !map.active;
    if pressed {
        response
            .flags
            .insert(egui::response::Flags::FAKE_PRIMARY_CLICKED);
    }
    response
}

/// A checkbox-style toggle; a MIDI press flips `value`.
pub(super) fn midi_toggle(
    ui: &mut egui::Ui,
    key: &str,
    label: &str,
    value: &mut bool,
    add: impl FnOnce(&mut egui::Ui, &mut bool) -> egui::Response,
) -> egui::Response {
    let (mut response, pressed) = mapped(ui, key, label, |ui| add(ui, value));
    if pressed {
        *value = !*value;
        response.mark_changed();
    }
    response
}

/// A `selectable_value`; a MIDI press selects `selected`.
pub(super) fn midi_select<T: PartialEq + Clone>(
    ui: &mut egui::Ui,
    key: &str,
    label: &str,
    current: &mut T,
    selected: T,
    text: impl Into<egui::WidgetText>,
) -> egui::Response {
    let choice = selected.clone();
    let (mut response, pressed) = mapped(ui, key, label, |ui| {
        ui.selectable_value(current, choice, text)
    });
    if pressed && *current != selected {
        *current = selected;
        response.mark_changed();
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(active: bool) -> MidiMapUi {
        MidiMapUi {
            active,
            learning: None,
            mapped: Vec::new(),
            audio_learning: None,
            audio_mapped: Vec::new(),
            palette: crate::ui::UiState::default().theme.palette(),
        }
    }

    /// Runs one egui frame with `pending` presses and returns the actions.
    fn frame(pending: &[&str], active: bool, draw: impl FnMut(&mut egui::Ui)) -> Vec<UiAction> {
        let ctx = egui::Context::default();
        begin_frame(
            pending.iter().map(|key| key_hash(key)).collect(),
            map(active),
        );
        let mut draw = draw;
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| draw(ui));
        end_frame()
    }

    #[test]
    fn keys_hash_stably_and_distinctly() {
        assert_eq!(key_hash("deck.0.eject"), key_hash("deck.0.eject"));
        assert_ne!(key_hash("deck.0.eject"), key_hash("deck.1.eject"));
        assert_eq!(
            target("toolbar.show_mode"),
            ControlTarget::UiButton(key_hash("toolbar.show_mode"))
        );
    }

    #[test]
    fn a_mapped_press_clicks_exactly_its_button() {
        let (mut eject, mut other) = (false, false);
        frame(&["test.eject"], false, |ui| {
            eject |=
                midi_button(ui, "test.eject", "Test · Eject", |ui| ui.button("Eject")).clicked();
            other |=
                midi_button(ui, "test.other", "Test · Other", |ui| ui.button("Other")).clicked();
        });
        assert!(eject && !other);
        assert_eq!(label(key_hash("test.eject")), "Test · Eject");
        assert!(known_targets().contains(&target("test.eject")));
    }

    #[test]
    fn toggles_and_selects_follow_a_press() {
        let mut enabled = false;
        let mut choice = 1;
        frame(&["test.toggle", "test.select"], false, |ui| {
            assert!(
                midi_toggle(
                    ui,
                    "test.toggle",
                    "Test · Toggle",
                    &mut enabled,
                    |ui, value| { ui.checkbox(value, "Toggle") }
                )
                .changed()
            );
            assert!(
                midi_select(ui, "test.select", "Test · Select", &mut choice, 3, "Three").changed()
            );
        });
        assert!(enabled);
        assert_eq!(choice, 3);
    }

    #[test]
    fn presses_are_ignored_while_disabled_or_mapping_and_never_replayed() {
        let mut clicked = false;
        frame(&["test.disabled"], false, |ui| {
            clicked |= midi_button(ui, "test.disabled", "Test · Disabled", |ui| {
                ui.add_enabled(false, egui::Button::new("Disabled"))
            })
            .clicked();
        });
        assert!(!clicked);
        frame(&["test.armed"], true, |ui| {
            clicked |=
                midi_button(ui, "test.armed", "Test · Armed", |ui| ui.button("Armed")).clicked();
        });
        assert!(!clicked, "map mode must not fire the button");
        // A press for a button that was not drawn is dropped at frame end.
        frame(&["test.hidden"], false, |_| {});
        frame(&[], false, |ui| {
            clicked |= midi_button(ui, "test.hidden", "Test · Hidden", |ui| {
                ui.button("Hidden")
            })
            .clicked();
        });
        assert!(!clicked, "stale presses must not fire on a later frame");
    }

    #[test]
    fn navigation_buttons_stay_usable_in_map_mode_and_still_take_presses() {
        // Map mode leaves window toggles enabled so their windows can open.
        frame(&[], true, |ui| {
            let response = midi_nav_button(ui, "test.window", "Test · Window", |ui| {
                ui.button("Window")
            });
            assert!(response.enabled());
        });
        let mut opened = false;
        frame(&["test.window"], false, |ui| {
            opened |= midi_nav_button(ui, "test.window", "Test · Window", |ui| {
                ui.button("Window")
            })
            .clicked();
        });
        assert!(opened, "a mapped press opens the window outside map mode");
    }

    #[test]
    fn closed_menu_actions_consume_their_press_once() {
        let mut fired = 0;
        frame(&["test.preset"], false, |_| {
            fired += usize::from(pressed("test.preset", "Test · Preset"));
            fired += usize::from(pressed("test.preset", "Test · Preset"));
        });
        assert_eq!(fired, 1);
    }
}
