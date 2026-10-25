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
        super::panel::section(
            ui,
            "setup",
            "⚙",
            "SETUP & DIAGNOSTICS",
            palette,
            false,
            |ui| {
                ui.weak("output · project · devices · sync");
            },
            |ui| {
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
            draw_projection_mapping(ui, state);
            draw_ndi_output(ui, state, &metrics.ndi, palette);
            draw_network_sync(ui, state, &metrics.sync, palette);
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
                                " · unlinked"
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
                    ui.weak("Routes use /virtual; deck and clip numbers are 1-based.");
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
            },
        );
    }
}

fn draw_projection_mapping(ui: &mut egui::Ui, state: &mut UiState) {
    egui::CollapsingHeader::new("Projection mapping · corner pin, crop, masks & blend")
        .show(ui, |ui| {
            ui.add_enabled_ui(!state.output_locked, |ui| {
                let mut candidate = state.output_projection;
                ui.checkbox(&mut candidate.enabled, "Map the external output");
                ui.weak("Drag corners to align the projector. The operator preview stays unchanged.");
                let width = ui.available_width().min(480.0);
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(width, width * 9.0 / 16.0),
                    egui::Sense::hover(),
                );
                let canvas = rect.shrink(10.0);
                let point = |p: [f32; 2]| {
                    egui::pos2(
                        canvas.left() + p[0] * canvas.width(),
                        canvas.top() + p[1] * canvas.height(),
                    )
                };
                let normalized = |p: egui::Pos2| {
                    [
                        ((p.x - canvas.left()) / canvas.width()).clamp(0.0, 1.0),
                        ((p.y - canvas.top()) / canvas.height()).clamp(0.0, 1.0),
                    ]
                };
                ui.painter().rect_filled(canvas, 0.0, egui::Color32::BLACK);

                let draw_mode = ui
                    .checkbox(&mut state.projection_mask_draw_mode, "Draw mask on canvas")
                    .on_hover_text("Drag across the projector preview to create a blackout mask.");
                let _ = draw_mode;
                let draw_response = ui.interact(
                    canvas,
                    ui.id().with("projection-mask-draw"),
                    egui::Sense::drag(),
                );
                if draw_response.drag_started() {
                    state.projection_mask_origin = draw_response.interact_pointer_pos().map(normalized);
                }
                if state.projection_mask_draw_mode
                    && (draw_response.dragged() || draw_response.drag_stopped())
                    && let (Some(start), Some(pointer)) = (
                        state.projection_mask_origin,
                        draw_response.interact_pointer_pos().map(normalized),
                    )
                {
                    let slot = state
                        .projection_selected_mask
                        .min(candidate.masks.len().saturating_sub(1));
                    candidate.masks[slot] = [
                        start[0].min(pointer[0]),
                        start[1].min(pointer[1]),
                        start[0].max(pointer[0]),
                        start[1].max(pointer[1]),
                    ];
                }
                if draw_response.drag_stopped() {
                    state.projection_mask_origin = None;
                    state.projection_mask_draw_mode = false;
                }

                for (index, mask) in candidate.masks.iter_mut().enumerate() {
                    if mask[2] <= mask[0] || mask[3] <= mask[1] {
                        continue;
                    }
                    let mask_rect = egui::Rect::from_min_max(
                        point([mask[0], mask[1]]),
                        point([mask[2], mask[3]]),
                    );
                    let selected = index == state.projection_selected_mask;
                    ui.painter().rect_filled(
                        mask_rect,
                        0.0,
                        egui::Color32::from_rgba_unmultiplied(
                            255,
                            40,
                            40,
                            if selected { 125 } else { 75 },
                        ),
                    );
                    ui.painter().rect_stroke(
                        mask_rect,
                        0.0,
                        egui::Stroke::new(
                            if selected { 2.0 } else { 1.0 },
                            egui::Color32::LIGHT_RED,
                        ),
                        egui::StrokeKind::Inside,
                    );
                    let response = ui.interact(
                        mask_rect,
                        ui.id().with(("projection-mask", index)),
                        if state.projection_mask_draw_mode {
                            egui::Sense::hover()
                        } else {
                            egui::Sense::drag()
                        },
                    );
                    if response.clicked() {
                        state.projection_selected_mask = index;
                    }
                    if response.dragged() && !state.projection_mask_draw_mode {
                        state.projection_selected_mask = index;
                        let delta = response.drag_motion();
                        let dx = delta.x / canvas.width();
                        let dy = delta.y / canvas.height();
                        let mask_width = mask[2] - mask[0];
                        let mask_height = mask[3] - mask[1];
                        mask[0] = (mask[0] + dx).clamp(0.0, 1.0 - mask_width);
                        mask[1] = (mask[1] + dy).clamp(0.0, 1.0 - mask_height);
                        mask[2] = mask[0] + mask_width;
                        mask[3] = mask[1] + mask_height;
                    }
                    let handle = egui::Rect::from_center_size(
                        mask_rect.right_bottom(),
                        egui::vec2(14.0, 14.0),
                    );
                    ui.painter()
                        .circle_filled(handle.center(), 4.0, egui::Color32::WHITE);
                    let resize = ui.interact(
                        handle,
                        ui.id().with(("projection-mask-resize", index)),
                        if state.projection_mask_draw_mode {
                            egui::Sense::hover()
                        } else {
                            egui::Sense::drag()
                        },
                    );
                    if resize.dragged() && !state.projection_mask_draw_mode {
                        state.projection_selected_mask = index;
                        if let Some(pointer) = resize.interact_pointer_pos() {
                            let p = normalized(pointer);
                            mask[2] = p[0].max(mask[0] + 0.005);
                            mask[3] = p[1].max(mask[1] + 0.005);
                        }
                    }
                }

                let labels = ["TL", "TR", "BR", "BL"];
                for (index, label) in labels.iter().enumerate() {
                    let pos = point(candidate.corners[index]);
                    let response = ui.interact(
                        egui::Rect::from_center_size(pos, egui::vec2(20.0, 20.0)),
                        ui.id().with(("projection-corner", index)),
                        if state.projection_mask_draw_mode {
                            egui::Sense::hover()
                        } else {
                            egui::Sense::drag()
                        },
                    );
                    if response.dragged()
                        && let Some(pointer) = response.interact_pointer_pos()
                    {
                        candidate.corners[index] = normalized(pointer);
                    }
                    ui.painter()
                        .circle_filled(pos, 5.0, egui::Color32::LIGHT_BLUE);
                    ui.painter().text(
                        pos + egui::vec2(7.0, 7.0),
                        egui::Align2::LEFT_TOP,
                        label,
                        egui::FontId::proportional(11.0),
                        egui::Color32::WHITE,
                    );
                }
                for index in 0..4 {
                    ui.painter().line_segment(
                        [
                            point(candidate.corners[index]),
                            point(candidate.corners[(index + 1) % 4]),
                        ],
                        egui::Stroke::new(1.5, egui::Color32::LIGHT_BLUE),
                    );
                }
                egui::CollapsingHeader::new("Exact corner coordinates").show(ui, |ui| {
                    for (index, label) in labels.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(*label);
                            for (axis, value) in candidate.corners[index].iter_mut().enumerate() {
                                ui.add(
                                    egui::DragValue::new(value)
                                        .range(0.0..=1.0)
                                        .speed(0.001)
                                        .prefix(if axis == 0 { "X " } else { "Y " }),
                                );
                            }
                        });
                    }
                });
                ui.label("Source crop · left / top / right / bottom");
                ui.horizontal(|ui| {
                    for value in &mut candidate.source {
                        ui.add(egui::DragValue::new(value).range(0.0..=1.0).speed(0.001));
                    }
                });
                egui::CollapsingHeader::new("Blackout masks").show(ui, |ui| {
                    ui.weak("Select a mask to edit it. Drag its body to move or its white corner to resize.");
                    for (index, mask) in candidate.masks.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            if ui
                                .selectable_label(
                                    state.projection_selected_mask == index,
                                    format!("Mask {}", index + 1),
                                )
                                .clicked()
                            {
                                state.projection_selected_mask = index;
                            }
                            for value in mask.iter_mut() {
                                ui.add(egui::DragValue::new(value).range(0.0..=1.0).speed(0.001));
                            }
                            if ui.small_button("Clear").clicked() {
                                *mask = [0.0; 4];
                            }
                        });
                    }
                });
                egui::CollapsingHeader::new("Edge feather & projector matching").show(ui, |ui| {
                    ui.weak("Feather normalized output edges. Gamma and black level help match projector calibration.");
                    for (index, label) in ["Left", "Top", "Right", "Bottom"].iter().enumerate() {
                        ui.add(
                            egui::Slider::new(&mut candidate.edge_blend[index], 0.0..=0.5)
                                .text(format!("{label} feather")),
                        );
                    }
                    ui.add(egui::Slider::new(&mut candidate.gamma, 0.25..=4.0).text("Output gamma"));
                    ui.add(
                        egui::Slider::new(&mut candidate.black_level, 0.0..=0.25)
                            .text("Black floor compensation"),
                    );
                });
                if ui.button("Reset calibration").clicked() {
                    candidate = virtual_core::ProjectionMapping {
                        enabled: candidate.enabled,
                        ..Default::default()
                    };
                }
                if candidate.is_valid() {
                    state.output_projection = candidate;
                } else {
                    ui.colored_label(egui::Color32::YELLOW, "Keep corners clockwise without crossing, and crop/mask edges ordered. Last valid calibration retained.");
                }
                ui.weak("Calibration is saved in the project. Use Test card while aligning; Lock output protects calibration.");
            });
        });
}

/// NDI network output of the program frame.
fn draw_ndi_output(
    ui: &mut egui::Ui,
    state: &mut UiState,
    status: &crate::ndi::NdiStatus,
    palette: super::theme::ThemePalette,
) {
    egui::CollapsingHeader::new("Network output · NDI").show(ui, |ui| {
        ui.horizontal(|ui| {
            buttons::midi_toggle(
                ui,
                "setup.ndi.enabled",
                "Output · NDI",
                &mut state.ndi_enabled,
                |ui, value| ui.checkbox(value, "Send program over NDI"),
            )
            .on_hover_text(
                "Publishes the program frame to OBS, Resolume, TouchDesigner, vMix and other \
                 NDI receivers on the local network. Requires the free NDI runtime (NDI Tools).",
            );
            ui.label("Source name");
            ui.add(
                egui::TextEdit::singleline(&mut state.ndi_name)
                    .desired_width(180.0)
                    .hint_text(crate::ndi::DEFAULT_SOURCE_NAME),
            )
            .on_hover_text(
                "Receivers list this as MACHINE (name). Changing it restarts the sender.",
            );
        });
        if let Some(error) = &status.error {
            ui.colored_label(palette.danger, error);
        } else if status.running {
            ui.label(format!(
                "Sending “{}” · {} receiver{} · {} sent · {} dropped",
                status.source_name,
                status.connections,
                if status.connections == 1 { "" } else { "s" },
                status.sent,
                status.dropped
            ));
            if status.connections == 0 {
                ui.weak("Idle until a receiver connects: no frames are read back until then.");
            }
        } else if state.ndi_enabled {
            ui.weak("Starting NDI…");
        } else {
            ui.weak(
                "Off. Frames are sent at the composition resolution with synthesized timecode.",
            );
        }
    });
}

/// VIRTUAL Sync: leader / follower networking with other machines and lights.
fn draw_network_sync(
    ui: &mut egui::Ui,
    state: &mut UiState,
    status: &crate::sync::SyncStatus,
    palette: super::theme::ThemePalette,
) {
    use virtual_io::SyncRoleProject as Role;
    egui::CollapsingHeader::new("Network sync · second rooms, TVs and lighting").show(ui, |ui| {
        let sync = &mut state.sync;
        ui.horizontal(|ui| {
            ui.label("Role");
            for (role, label, key) in [
                (Role::Off, "Off", "off"),
                (Role::Leader, "Leader", "leader"),
                (Role::Follower, "Follower", "follower"),
            ] {
                buttons::midi_select(
                    ui,
                    &format!("setup.sync.role.{key}"),
                    &format!("Network sync · {label}"),
                    &mut sync.role,
                    role,
                    label,
                );
            }
        });
        ui.horizontal(|ui| {
            ui.label("Name");
            ui.add(egui::TextEdit::singleline(&mut sync.name).desired_width(140.0))
                .on_hover_text("How this machine appears to other devices");
            ui.label("Show key");
            ui.add(
                egui::TextEdit::singleline(&mut sync.passphrase)
                    .password(true)
                    .desired_width(140.0)
                    .hint_text("none (unsigned)"),
            )
            .on_hover_text(
                "Type the same key on every device. Packets are signed with it, so other \
                 machines on the network cannot spoof or replay your show.",
            );
            ui.label("Port");
            ui.add(egui::DragValue::new(&mut sync.port).range(1024..=65535));
        });
        match sync.role {
            Role::Off => {
                ui.weak("Off. Choose Leader on the main machine and Follower on the others.");
            }
            Role::Leader => {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Send");
                    ui.checkbox(&mut sync.send_clock, "Tempo & beat");
                    ui.checkbox(&mut sync.send_controls, "Parameter changes");
                    ui.checkbox(&mut sync.send_audio, "Audio bands");
                    ui.checkbox(&mut sync.send_zones, "Light zones");
                    ui.add_enabled(
                        sync.send_zones,
                        egui::DragValue::new(&mut sync.zones).range(1..=128).suffix(" zones"),
                    )
                    .on_hover_text("The program image averaged into this many vertical colour strips");
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut sync.broadcast, "Broadcast for listen-only devices")
                        .on_hover_text(
                            "Devices that never say hello (simple light controllers) receive the \
                             broadcast. Followers that say hello also get unicast, which is more \
                             reliable on busy Wi-Fi.",
                        );
                });
                ui.horizontal(|ui| {
                    ui.label("Fixed receivers");
                    let mut text = sync.targets.join(", ");
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut text)
                                .desired_width(260.0)
                                .hint_text("192.168.1.50, 192.168.1.51:41641"),
                        )
                        .on_hover_text("Devices with static IPs on networks that block broadcast")
                        .changed()
                    {
                        sync.targets = text
                            .split(',')
                            .map(str::trim)
                            .filter(|target| !target.is_empty())
                            .map(str::to_owned)
                            .collect();
                    }
                });
            }
            Role::Follower => {
                ui.horizontal(|ui| {
                    ui.label("Follow leader");
                    ui.add(
                        egui::TextEdit::singleline(&mut sync.leader)
                            .desired_width(140.0)
                            .hint_text("first one heard"),
                    );
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label("Follow");
                    ui.checkbox(&mut sync.follow_tempo, "Tempo & beat");
                    ui.checkbox(&mut sync.follow_mixer, "Mixer & master");
                    ui.checkbox(&mut sync.follow_transport, "Transport, clips & scenes");
                    ui.checkbox(&mut sync.follow_effects, "Effects & generators");
                    ui.checkbox(&mut sync.follow_buttons, "Button presses")
                        .on_hover_text("Mirror every on-screen button the leader presses. Off by default.");
                });
                if sync.follow_tempo {
                    ui.weak("Following the leader's tempo replaces the local clock source while connected.");
                }
            }
        }
        if let Some(error) = &status.error {
            ui.colored_label(palette.danger, error);
        } else if status.active {
            match status.role {
                Role::Leader => ui.label(format!(
                    "{} follower(s) · {} datagrams sent · {} rejected",
                    status.peers.len(),
                    status.sent_or_received,
                    status.rejected
                )),
                _ => ui.label(format!(
                    "{} · {} received · {} applied · {} rejected{}",
                    status
                        .following
                        .as_ref()
                        .map_or("Searching for a leader…".to_owned(), |name| format!("Following {name}")),
                    status.sent_or_received,
                    status.applied,
                    status.rejected,
                    status
                        .round_trip_ms
                        .map_or(String::new(), |rtt| format!(" · round trip {rtt:.1} ms"))
                )),
            };
            for peer in &status.peers {
                ui.label(
                    egui::RichText::new(format!("• {} — {}", peer.name, peer.detail))
                        .small()
                        .color(palette.muted_text),
                );
            }
            if status.rejected > 0 && status.peers.is_empty() {
                ui.colored_label(
                    palette.warning,
                    "Packets are arriving but failing their signature: check the show key matches.",
                );
            }
        }
    });
}
