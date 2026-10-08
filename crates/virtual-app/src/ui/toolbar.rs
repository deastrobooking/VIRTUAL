//! Always-visible show controls and release preflight summary.

use virtual_core::{ClockSource, ControlTarget};
use virtual_media::{CLIPS_PER_DECK, ClipAddress, ClipBank, DeckId};

use super::buttons::midi_button;
use super::theme::ThemePalette;
use super::{MidiMapUi, PerformanceMetrics, UiAction, UiState, draw_layer_order, mappable};

pub(super) fn draw_toolbar(
    ui: &mut egui::Ui,
    state: &mut UiState,
    clips: &ClipBank,
    metrics: &PerformanceMetrics<'_>,
    palette: ThemePalette,
    midi_map: &MidiMapUi,
    actions: &mut Vec<UiAction>,
) {
    let mut missing_media = 0;
    let mut loading_media = 0;
    for deck in DeckId::ALL {
        for slot in 0..CLIPS_PER_DECK {
            let address = ClipAddress { deck, slot };
            if let Some(slot_state) = clips.slot(address) {
                if slot_state.error.is_some() {
                    missing_media += 1;
                } else if slot_state.movie.is_none() && slot_state.pending_path.is_some() {
                    loading_media += 1;
                }
            }
        }
    }
    let midi_waiting = metrics
        .midi
        .devices
        .iter()
        .filter(|device| device.wanted && !device.connected)
        .count();
    let output_ready = state.output_enabled
        && !metrics.output_displays.is_empty()
        && metrics.output_health.status == "Healthy";
    let effect_rejected = state.effect_reload_failed
        || state.deck_effect_reload_failed
        || state.effect_registry_errors > 0;
    let audio_missing = metrics.audio_required && !metrics.audio_connected;
    let preflight_ready = preflight_ready([
        !output_ready,
        missing_media > 0,
        loading_media > 0,
        midi_waiting > 0,
        audio_missing,
        effect_rejected,
        metrics.project_dirty,
        state.save_status.pending > 0,
        state.save_status.error.is_some(),
    ]);

    ui.horizontal_wrapped(|ui| {
        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new("VIRTUAL")
                    .size(22.0)
                    .strong()
                    .color(palette.accent),
            );
            ui.weak("LIVE VISUAL PERFORMANCE SYSTEM");
        });
        ui.add_space(12.0);
        status_dot(
            ui,
            &palette,
            "PROGRAM",
            state.output_enabled,
            state.output_enabled && metrics.output_health.status != "Healthy",
        );
        status_dot(
            ui,
            &palette,
            "AUDIO",
            metrics.audio_connected,
            metrics.audio_snapshot.callback_errors > 0,
        );
        status_dot(ui, &palette, "MIDI", metrics.midi.any_connected(), false);
        status_dot(ui, &palette, "OSC", metrics.osc.connected, false);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Reverse insertion keeps the visible channel order A–D.
            for deck in DeckId::ALL.into_iter().rev() {
                let muted = &mut state.bypassed[deck.index()];
                let name = ["A", "B", "C", "D"][deck.index()];
                let color = palette.deck_color(deck);
                let label = if *muted {
                    format!("{name} · MUTED")
                } else {
                    format!("{name} · MUTE")
                };
                let target = ControlTarget::DeckMute(deck.index() as u8);
                if mappable(ui, midi_map, target, actions, |ui| {
                    ui.add(
                        egui::Button::new(egui::RichText::new(label).strong())
                            .selected(*muted)
                            .fill(palette.control_tint(color, if *muted { 0.55 } else { 0.12 }))
                            .stroke(egui::Stroke::new(1.0, color))
                            .min_size(egui::vec2(94.0, 34.0)),
                    )
                    .on_hover_text(format!(
                        "{} deck {name} in the program mix. Playback and channel settings are preserved.",
                        if *muted { "Unmute" } else { "Mute" }
                    ))
                })
                .clicked()
                {
                    *muted = !*muted;
                }
            }
            ui.weak("CHANNEL MUTES");
        });
    });
    ui.horizontal_wrapped(|ui| {
        let blackout_fill = if state.blackout {
            palette.danger
        } else {
            palette.control_tint(palette.danger, 0.18)
        };
        if mappable(ui, midi_map, ControlTarget::MasterBlackout, actions, |ui| {
            ui.add(
                egui::Button::new(
                    egui::RichText::new("BLACKOUT")
                        .strong()
                        .color(egui::Color32::WHITE),
                )
                .fill(blackout_fill)
                .min_size(egui::vec2(104.0, 34.0)),
            )
            .on_hover_text("Emergency program blackout")
        })
        .clicked()
        {
            state.blackout = !state.blackout;
        }
        if mappable(ui, midi_map, ControlTarget::MasterFreeze, actions, |ui| {
            ui.add(
                egui::Button::new(if state.master_freeze {
                    "Resume master"
                } else {
                    "Freeze master"
                })
                .selected(state.master_freeze),
            )
        })
        .clicked()
        {
            state.master_freeze = !state.master_freeze;
        }
        ui.separator();
        draw_layer_order(ui, state, &palette, midi_map, actions, true);
        ui.separator();
        if midi_button(ui, "toolbar.open_output", "Toolbar · Open output", |ui| {
            ui.button("Open output")
                .on_hover_text("Reopen or bring forward the external program output window")
        })
        .clicked()
        {
            actions.push(UiAction::SetOutputEnabled(true));
        }
        if ui
            .add_enabled(
                state.output_enabled || state.output_locked,
                egui::Button::new(if state.output_locked {
                    "Unlock output"
                } else {
                    "Lock output"
                })
                .selected(state.output_locked),
            )
            .on_hover_text("Prevent closing or disabling the output until unlocked")
            .clicked()
        {
            state.output_locked = !state.output_locked;
        }
        let show_label = if state.show_mode {
            "EXIT SHOW MODE"
        } else {
            "SHOW MODE"
        };
        if midi_button(ui, "toolbar.show_mode", "Toolbar · Show Mode", |ui| {
            ui.add(
                egui::Button::new(egui::RichText::new(show_label).strong()).fill(
                    if state.show_mode {
                        palette.control_tint(palette.success, 0.36)
                    } else {
                        palette.control
                    },
                ),
            )
            .on_hover_text("Lock setup, media management and structural effect editors")
        })
        .clicked()
        {
            state.show_mode = !state.show_mode;
            if state.show_mode {
                state.audio_learn = None;
                state.midi_map_mode = false;
                state.midi_manager_open = false;
                actions.push(UiAction::MidiCancelLearn);
            }
        }
        if !state.show_mode {
            if midi_button(ui, "toolbar.appearance", "Toolbar · Appearance", |ui| {
                ui.selectable_label(state.theme.editor_open, "Appearance")
            })
            .clicked()
            {
                state.theme.editor_open = !state.theme.editor_open;
            }
            if midi_button(ui, "toolbar.midi_manager", "Toolbar · MIDI Manager", |ui| {
                ui.selectable_label(state.midi_manager_open, "MIDI")
                    .on_hover_text("Open the MIDI Manager window")
            })
            .clicked()
            {
                state.midi_manager_open = !state.midi_manager_open;
            }
            if state.midi_map_mode {
                let response = ui.selectable_label(
                    true,
                    egui::RichText::new("MAP").strong().color(palette.accent),
                );
                if response
                    .on_hover_text("MIDI map mode armed · click to exit")
                    .clicked()
                {
                    state.midi_map_mode = false;
                    actions.push(UiAction::MidiCancelLearn);
                }
            }
        }
        ui.weak(format!("{:.0} fps", state.fps.fps()));
    });
    ui.horizontal_wrapped(|ui| {
        let external =
            state.midi_clock_source == ClockSource::MidiInput && metrics.midi.clock.locked;
        ui.add_enabled(
            !external,
            egui::DragValue::new(&mut state.bpm)
                .range(20.0..=400.0)
                .speed(0.25)
                .suffix(" BPM"),
        );
        if mappable(ui, midi_map, ControlTarget::TapTempo, actions, |ui| {
            ui.add_enabled(!external, egui::Button::new("Tap tempo"))
                .on_hover_text("Tap with the DJ's beat; available in Show Mode")
        })
        .clicked()
        {
            actions.push(UiAction::TapTempo);
        }
        ui.weak(match state.midi_clock_source {
            ClockSource::Internal => "Internal",
            ClockSource::MidiInput => "MIDI clock",
            ClockSource::AbletonLink => "Ableton Link",
        });
        if metrics.audio_connected {
            ui.separator();
            let audio = metrics.audio_snapshot.analysis;
            for (label, value) in [
                ("Level", audio.rms),
                ("Bass", audio.bass),
                ("Mid", audio.mid),
                ("High", audio.high),
                ("Hit", audio.transient),
            ] {
                ui.add(
                    egui::ProgressBar::new(value)
                        .text(label)
                        .desired_width(72.0),
                );
            }
        }
    });
    ui.horizontal_wrapped(|ui| {
        ui.weak(metrics.gpu_info);
        ui.separator();
        ui.weak(metrics.runtime_status);
    });
    ui.horizontal_wrapped(|ui| {
        if state.save_status.pending > 0 {
            ui.spinner();
            ui.label(format!("Saving · {} pending", state.save_status.pending));
        }
        if let Some((path, error)) = &state.save_status.error {
            ui.colored_label(
                palette.danger,
                format!("Save failed · {}: {error}", path.display()),
            );
            if midi_button(
                ui,
                "toolbar.dismiss_save_error",
                "Toolbar · Dismiss save error",
                |ui| ui.button("Dismiss save error"),
            )
            .clicked()
            {
                state.save_status.error = None;
            }
        }
        if !metrics.project_status.is_empty() {
            ui.weak(metrics.project_status);
        }
    });
    ui.horizontal_wrapped(|ui| {
        let (label, color) = if preflight_ready {
            ("● PREFLIGHT READY", palette.success)
        } else {
            ("● PREFLIGHT ATTENTION", palette.warning)
        };
        ui.colored_label(color, egui::RichText::new(label).strong());
        ui.separator();
        ui.label(if output_ready {
            "output healthy"
        } else {
            "output not ready"
        });
        ui.label(format!("missing {missing_media}"));
        ui.label(format!("loading {loading_media}"));
        ui.label(format!("MIDI waiting {midi_waiting}"));
        if audio_missing {
            ui.colored_label(palette.warning, "requested audio disconnected");
        }
        ui.label(if metrics.project_dirty {
            "project modified"
        } else {
            "project saved"
        });
        if effect_rejected {
            ui.colored_label(palette.danger, "effect attention")
                .on_hover_text(format!(
                    "{}\n{}\n{}",
                    state.effect_reload_status,
                    state.deck_effect_reload_status,
                    state.effect_registry_status
                ));
        }
        if state.show_mode {
            ui.separator();
            ui.colored_label(palette.success, "SHOW LOCKED");
        }
    });
}

fn preflight_ready(blockers: [bool; 9]) -> bool {
    !blockers.into_iter().any(|blocked| blocked)
}

fn status_dot(ui: &mut egui::Ui, palette: &ThemePalette, label: &str, active: bool, warning: bool) {
    let color = if warning {
        palette.danger
    } else if active {
        palette.success
    } else {
        palette.idle
    };
    ui.label(egui::RichText::new("●").color(color));
    ui.weak(label);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_required_show_check_blocks_readiness_independently() {
        assert!(preflight_ready([false; 9]));
        for index in 0..9 {
            let mut blockers = [false; 9];
            blockers[index] = true;
            assert!(!preflight_ready(blockers), "blocker {index}");
        }
    }
}
