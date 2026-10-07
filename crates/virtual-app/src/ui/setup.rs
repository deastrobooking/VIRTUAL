//! Pre-show output, project, device, and diagnostics workspace.

use virtual_core::{ClockSource, ControlTarget, Quantization};
use virtual_media::FourDeckMixer;

use super::diagnostics::{draw_output_health, draw_pipeline_health, draw_runtime_summary};
use super::{MidiMapUi, PerformanceMetrics, UiAction, UiState, buttons, draw_midi, mappable};

pub(super) fn draw_setup(
    ui: &mut egui::Ui,
    state: &mut UiState,
    mixer: &FourDeckMixer,
    midi_map: &MidiMapUi,
    actions: &mut Vec<UiAction>,
    metrics: &mut PerformanceMetrics<'_>,
) {
    let palette = midi_map.palette;
    if !state.show_mode {
        let setup_height = (ui.available_height() * 0.45).clamp(160.0, 420.0);
        egui::CollapsingHeader::new("Setup & diagnostics · output, project, devices")
        .default_open(false)
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("setup-region")
                .max_height(setup_height)
                .auto_shrink([false, true])
                .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Output");
                if buttons::midi_toggle(
                    ui,
                    "setup.output.enabled",
                    "Output · Enabled",
                    &mut state.output_enabled,
                    |ui, value| ui.checkbox(value, "Enabled"),
                )
                .changed()
                {
                    actions.push(UiAction::SetOutputEnabled(state.output_enabled));
                }
                let output_enabled = state.output_enabled;
                if buttons::midi_toggle(
                    ui,
                    "setup.output.fullscreen",
                    "Output · Fullscreen",
                    &mut state.output_fullscreen,
                    |ui, value| {
                        ui.add_enabled(output_enabled, egui::Checkbox::new(value, "Fullscreen"))
                    },
                )
                .changed()
                {
                    actions.push(UiAction::SetOutputFullscreen(state.output_fullscreen));
                }
                let display_label = metrics
                    .output_displays
                    .iter()
                    .find(|display| display.id == state.output_display_id)
                    .map_or("No display", |display| display.label.as_str());
                egui::ComboBox::from_id_salt("output-display")
                    .selected_text(display_label)
                    .show_ui(ui, |ui| {
                        for display in metrics.output_displays {
                            if ui
                                .selectable_value(
                                    &mut state.output_display_id,
                                    display.id.clone(),
                                    &display.label,
                                )
                                .changed()
                            {
                                actions.push(UiAction::SetOutputDisplay(display.id.clone()));
                            }
                        }
                    });
                if buttons::midi_button(ui, "setup.output.refresh_displays", "Output · Refresh displays", |ui| ui.button("Refresh displays")).clicked() {
                    actions.push(UiAction::RefreshDisplays);
                }
                ui.separator();
                egui::ComboBox::from_id_salt("composition-resolution")
                    .selected_text(format!(
                        "{} × {}",
                        state.composition_extent[0], state.composition_extent[1]
                    ))
                    .show_ui(ui, |ui| {
                        for (label, extent) in [
                            ("720p", [1280, 720]),
                            ("1080p", [1920, 1080]),
                            ("UHD", [3840, 2160]),
                        ] {
                            if ui
                                .selectable_value(
                                    &mut state.composition_extent,
                                    extent,
                                    format!("{label} · {} × {}", extent[0], extent[1]),
                                )
                                .changed()
                            {
                                state.custom_composition_extent = extent;
                                actions.push(UiAction::SetCompositionExtent(extent));
                            }
                        }
                    });
                ui.label("Custom");
                ui.add(
                    egui::DragValue::new(&mut state.custom_composition_extent[0])
                        .range(320..=7680)
                        .speed(8),
                );
                ui.label("×");
                ui.add(
                    egui::DragValue::new(&mut state.custom_composition_extent[1])
                        .range(180..=4320)
                        .speed(8),
                );
                if buttons::midi_button(ui, "setup.output.apply_extent", "Output · Apply custom size", |ui| ui.button("Apply")).clicked() {
                    actions.push(UiAction::SetCompositionExtent(
                        state.custom_composition_extent,
                    ));
                }
                ui.separator();
                buttons::midi_toggle(
                    ui,
                    "setup.output.test_card",
                    "Output · Test card",
                    &mut state.output_test_card,
                    |ui, value| ui.checkbox(value, "Test card"),
                );
                buttons::midi_toggle(
                    ui,
                    "setup.output.identify",
                    "Output · Identify",
                    &mut state.output_identify,
                    |ui, value| ui.checkbox(value, "Identify"),
                );
            });
            draw_output_health(ui, state, metrics, palette);
            ui.horizontal(|ui| {
                ui.label(if metrics.project_dirty {
                    "● Modified"
                } else {
                    "Saved"
                });
                ui.add_sized(
                    [320.0, 22.0],
                    egui::TextEdit::singleline(&mut state.project_path)
                        .hint_text("project.virtual"),
                );
                if buttons::midi_button(ui, "setup.project.open", "Project · Open path", |ui| ui.button("Open")).clicked() {
                    actions.push(UiAction::OpenProject);
                }
                if buttons::midi_button(ui, "setup.project.save", "Project · Save", |ui| ui.button("Save")).clicked() {
                    actions.push(UiAction::SaveProject);
                }
                if ui.button("Open…").clicked() {
                    actions.push(UiAction::BrowseOpenProject);
                }
                if ui.button("Save As…").clicked() {
                    actions.push(UiAction::SaveProjectAs);
                }
                if metrics.recovery_available && buttons::midi_button(ui, "setup.project.recover_autosave", "Project · Recover autosave", |ui| ui.button("Recover autosave")).clicked() {
                    actions.push(UiAction::RecoverProject);
                }
                if !metrics.project_status.is_empty() {
                    ui.weak(metrics.project_status);
                }
            });
            egui::CollapsingHeader::new("Session recovery")
                .default_open(false)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if buttons::midi_button(ui, "setup.recovery.scan", "Recovery · Scan journals", |ui| ui.button("Scan journals")).clicked() {
                            actions.push(UiAction::RefreshSessionRecoveries);
                        }
                        if !metrics.session_recoveries.is_empty() {
                            let selected = state
                                .session_recovery_selected
                                .min(metrics.session_recoveries.len() - 1);
                            state.session_recovery_selected = selected;
                            egui::ComboBox::from_id_salt("session-recovery-select")
                                .selected_text(metrics.session_recoveries[selected].file_name())
                                .show_ui(ui, |ui| {
                                    for (index, entry) in
                                        metrics.session_recoveries.iter().enumerate()
                                    {
                                        ui.selectable_value(
                                            &mut state.session_recovery_selected,
                                            index,
                                            entry.file_name(),
                                        );
                                    }
                                });
                            if buttons::midi_button(ui, "setup.recovery.restore_latest", "Recovery · Restore latest as branch", |ui| ui.button("Restore latest as branch")).clicked() {
                                actions.push(UiAction::RestoreSessionRecovery(
                                    state.session_recovery_selected,
                                ));
                            }
                        }
                    });
                    if !metrics.project_takes.is_empty() {
                        let selected = state
                            .project_take_selected
                            .min(metrics.project_takes.len() - 1);
                        state.project_take_selected = selected;
                        ui.horizontal(|ui| {
                            ui.label("Project take catalog");
                            egui::ComboBox::from_id_salt("project-take-select")
                                .selected_text(&metrics.project_takes[selected].name)
                                .show_ui(ui, |ui| {
                                    for (index, take) in metrics.project_takes.iter().enumerate() {
                                        ui.selectable_value(
                                            &mut state.project_take_selected,
                                            index,
                                            format!("{} · {}", take.name, take.journal_file),
                                        );
                                    }
                                });
                            if buttons::midi_button(ui, "setup.takes.rename", "Takes · Rename metadata", |ui| ui.button("Rename metadata")).clicked() {
                                actions.push(UiAction::RenameProjectTake(
                                    state.project_take_selected,
                                ));
                            }
                            if buttons::midi_button(ui, "setup.takes.remove", "Takes · Remove metadata", |ui| ui.button("Remove metadata")).clicked() {
                                actions.push(UiAction::RemoveProjectTake(
                                    state.project_take_selected,
                                ));
                            }
                            if buttons::midi_button(ui, "setup.takes.export", "Takes · Export copy", |ui| ui.button("Export copy")).clicked() {
                                actions.push(UiAction::ExportProjectTake(
                                    state.project_take_selected,
                                ));
                            }
                            if buttons::midi_button(ui, "setup.takes.archive", "Takes · Archive copy", |ui| ui.button("Archive copy")).clicked() {
                                actions.push(UiAction::ArchiveProjectTake(
                                    state.project_take_selected,
                                ));
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label("Export directory");
                            ui.text_edit_singleline(&mut state.take_export_directory);
                        });
                    }
                    ui.horizontal(|ui| {
                        ui.label("Take / branch name");
                        ui.text_edit_singleline(&mut state.take_name_input);
                        if buttons::midi_button(ui, "setup.takes.start", "Takes · Start named take", |ui| ui.button("Start named take")).clicked() {
                            actions.push(UiAction::StartNamedTake);
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Deterministic seed");
                        ui.text_edit_singleline(&mut state.random_seed_scope);
                        ui.add(egui::DragValue::new(&mut state.random_seed_value));
                        if buttons::midi_button(ui, "setup.seed.set", "Session · Set seed", |ui| ui.button("Set seed")).clicked() {
                            actions.push(UiAction::SetRandomSeed);
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Timeline marker");
                        ui.text_edit_singleline(&mut state.timeline_marker_input);
                        if buttons::midi_button(ui, "setup.marker.add", "Session · Add timeline marker", |ui| ui.button("Add marker")).clicked() {
                            actions.push(UiAction::AddTimelineMarker);
                        }
                    });
                    if let Some(entry) = metrics
                        .session_recoveries
                        .get(state.session_recovery_selected)
                    {
                        ui.label(format!(
                            "{} · {} command(s) · {:.1}s{}{}{}",
                            entry.take_name,
                            entry.command_count,
                            entry.latest_time.monotonic_ns as f64 / 1_000_000_000.0,
                            if entry.checkpointed { " · checkpoint" } else { "" },
                            if entry.ignored_partial_tail {
                                " · torn tail ignored"
                            } else {
                                ""
                            },
                            if entry.project_linked {
                                " · project linked"
                            } else {
                                " · legacy/unlinked"
                            }
                        ));
                        let maximum = entry.latest_time.monotonic_ns as f64 / 1_000_000_000.0;
                        state.session_replay_seconds =
                            state.session_replay_seconds.clamp(0.0, maximum);
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::Slider::new(
                                    &mut state.session_replay_seconds,
                                    0.0..=maximum.max(0.001),
                                )
                                .text("timeline seconds"),
                            );
                            if buttons::midi_button(ui, "setup.recovery.restore_cursor", "Recovery · Restore cursor as branch", |ui| ui.button("Restore cursor as branch")).clicked() {
                                actions.push(UiAction::RestoreSessionRecoveryAt {
                                    index: state.session_recovery_selected,
                                    monotonic_ns: (state.session_replay_seconds
                                        * 1_000_000_000.0)
                                        .round()
                                        as u64,
                                });
                            }
                        });
                        if !entry.markers().is_empty() {
                            ui.horizontal_wrapped(|ui| {
                                ui.label("Markers");
                                for marker in entry.markers() {
                                    let seconds =
                                        marker.at.monotonic_ns as f64 / 1_000_000_000.0;
                                    if buttons::midi_button(
                                        ui,
                                        &format!("setup.marker.jump.{}", marker.label),
                                        &format!("Recovery · Jump to marker {}", marker.label),
                                        |ui| {
                                            ui.small_button(format!(
                                                "{} · {:.1}s",
                                                marker.label, seconds
                                            ))
                                        },
                                    )
                                    .clicked()
                                    {
                                        state.session_replay_seconds = seconds;
                                    }
                                }
                            });
                        }
                    }
                    ui.weak(metrics.session_recovery_status);
                    ui.weak(
                        "Restore starts a fresh journal and applies recovered mixer, output, effect and modulation state.",
                    );
                    ui.weak(
                        "Load the matching project first so recovered clip launches resolve against the same media slots.",
                    );
                });
            super::video_input::draw_video_input(ui, state, mixer.selected(), metrics.cameras, metrics.camera_status, actions);
            ui.weak("Audio input, the 8-band spectrum and band mappings are in the AUDIO SPECTRUM strip below the clip grid.");
            draw_midi(ui, state, &mut metrics.midi, actions);
            egui::CollapsingHeader::new("OSC input")
                .default_open(false)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("UDP bind");
                        ui.add_enabled(
                            !metrics.osc.connected,
                            egui::TextEdit::singleline(&mut state.osc_bind_address)
                                .desired_width(170.0),
                        );
                        if metrics.osc.connected {
                            if buttons::midi_button(ui, "setup.osc.disconnect", "OSC · Disconnect input", |ui| ui.button("Disconnect")).clicked() {
                                actions.push(UiAction::DisconnectOscInput);
                            }
                        } else if buttons::midi_button(ui, "setup.osc.listen", "OSC · Listen", |ui| ui.button("Listen")).clicked() {
                            actions.push(UiAction::ConnectOscInput);
                        }
                        ui.weak(metrics.osc.status);
                    });
                    ui.weak(format!(
                        "packets {} · messages {} · malformed {} · dropped {} · scheduled {} · schedule drops {}",
                        metrics.osc.stats.packets,
                        metrics.osc.stats.messages,
                        metrics.osc.stats.malformed,
                        metrics.osc.stats.dropped,
                        metrics.osc.pending,
                        metrics.osc.schedule_dropped
                    ));
                    ui.horizontal(|ui| {
                        ui.label("Feedback target");
                        ui.add_enabled(
                            !metrics.osc.output_connected,
                            egui::TextEdit::singleline(&mut state.osc_feedback_address)
                                .desired_width(170.0),
                        );
                        if metrics.osc.output_connected {
                            if buttons::midi_button(ui, "setup.osc.stop_feedback", "OSC · Stop feedback", |ui| ui.button("Stop feedback")).clicked() {
                                actions.push(UiAction::DisconnectOscOutput);
                            }
                        } else if buttons::midi_button(ui, "setup.osc.send_feedback", "OSC · Send feedback", |ui| ui.button("Send feedback")).clicked() {
                            actions.push(UiAction::ConnectOscOutput);
                        }
                        ui.weak(metrics.osc.output_status);
                    });
                    ui.weak(format!(
                        "feedback sent {} · dropped {} · errors {}",
                        metrics.osc.output_stats.sent,
                        metrics.osc.output_stats.dropped,
                        metrics.osc.output_stats.errors
                    ));
                    ui.weak("Routes use /virtual (the older /vjx prefix is still accepted); deck and clip numbers are 1-based.");
                });
            ui.separator();

            draw_runtime_summary(ui, state, metrics);

            ui.separator();
            ui.horizontal(|ui| {
                // While an external clock is driving the show, the tempo
                // controls are read-only: anything typed here would be
                // overwritten by the next pulse.
                let external = state.midi_clock_source == ClockSource::MidiInput
                    && metrics.midi.clock.locked;
                ui.add_enabled(
                    !external,
                    egui::DragValue::new(&mut state.bpm)
                        .range(20.0..=400.0)
                        .speed(0.25)
                        .suffix(" BPM"),
                );
                if external {
                    ui.colored_label(
                        palette.accent,
                        format!(
                            "MIDI CLOCK · {}",
                            metrics.midi.clock.following.unwrap_or("external")
                        ),
                    )
                    .on_hover_text("Tempo and beat phase follow the incoming clock");
                }
                let tap = ui.add_enabled_ui(!external, |ui| {
                    mappable(ui, midi_map, ControlTarget::TapTempo, actions, |ui| {
                        ui.button("Tap")
                    })
                })
                .inner;
                if tap.clicked() {
                    actions.push(UiAction::TapTempo);
                }
                if buttons::midi_button(ui, "tempo.half", "Tempo · Half", |ui| {
                    ui.add_enabled(!external, egui::Button::new("½"))
                        .on_hover_text("Half tempo")
                })
                .clicked()
                {
                    actions.push(UiAction::HalfTempo);
                }
                if buttons::midi_button(ui, "tempo.double", "Tempo · Double", |ui| {
                    ui.add_enabled(!external, egui::Button::new("×2"))
                        .on_hover_text("Double tempo")
                })
                .clicked()
                {
                    actions.push(UiAction::DoubleTempo);
                }
                buttons::midi_select(
                    ui,
                    "tempo.quantize.immediate",
                    "Quantize · Immediate",
                    &mut state.quantization,
                    Quantization::Immediate,
                    "Immediate",
                );
                buttons::midi_select(
                    ui,
                    "tempo.quantize.beat",
                    "Quantize · Next beat",
                    &mut state.quantization,
                    Quantization::Beat,
                    "Next beat",
                );
                buttons::midi_select(
                    ui,
                    "tempo.quantize.bar",
                    "Quantize · Next bar",
                    &mut state.quantization,
                    Quantization::Bar,
                    "Next bar",
                );
                ui.separator();
                ui.label(format!(
                    "beat {:.2} · phase {:.2} · bar {:.2}",
                    metrics.tempo.beat_at(metrics.now_seconds),
                    metrics.tempo.beat_phase(metrics.now_seconds),
                    metrics.tempo.bar_phase(metrics.now_seconds)
                ));
                draw_pipeline_health(ui, metrics);
            });
                });
        });
    }
}
