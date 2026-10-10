//! Clip grid: slots, health, launch and thumbnail rendering.

use super::*;

const CLIP_CELL: egui::Vec2 = egui::vec2(132.0, 50.0);
const SCENE_CELL: egui::Vec2 = egui::vec2(132.0, 28.0);
const CLIP_THUMBNAIL: egui::Vec2 = egui::vec2(48.0, 27.0);

/// Gives a grid cell an exact rect. A child scope placed straight into the
/// grid (drag sources, MIDI-map overlays) gets a max rect reaching the bottom
/// of the panel and the row's centred layout, which drops each button lower
/// than the one before and grows the row.
fn fixed_cell<R>(ui: &mut egui::Ui, size: egui::Vec2, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.allocate_ui_with_layout(
        size,
        egui::Layout::centered_and_justified(egui::Direction::TopDown),
        |ui| {
            ui.set_min_size(size);
            ui.set_max_size(size);
            add(ui)
        },
    )
    .inner
}

pub(super) struct ClipGridContext<'a> {
    pub launches: &'a LaunchQueue,
    pub midi_map: &'a MidiMapUi,
    pub cameras: &'a [CameraDevice],
    pub camera_status: &'a str,
    pub camera_recordings: [CameraRecordingStatus; 4],
}

pub(super) fn draw_clip_grid(
    ui: &mut egui::Ui,
    state: &mut UiState,
    mixer: &mut FourDeckMixer,
    clips: &mut ClipBank,
    context: ClipGridContext<'_>,
    actions: &mut Vec<UiAction>,
) {
    let launches = context.launches;
    let midi_map = context.midi_map;
    let cameras = context.cameras;
    let camera_status = context.camera_status;
    let camera_recordings = context.camera_recordings;
    let palette = state.theme.palette();
    ui.horizontal(|ui| {
        for (bank, label, range) in [
            (0, "A", "1–8"),
            (1, "B", "9–16"),
            (2, "C", "17–24"),
            (3, "D", "25–32"),
        ] {
            let response = ui.selectable_label(state.scene_bank == bank, label);
            if response.clicked() {
                state.scene_bank = bank;
            }
            response.on_hover_text(format!("Scenes {range}"));
        }
        ui.weak(format!(
            "Scenes {}–{}",
            state.scene_bank * 8 + 1,
            state.scene_bank * 8 + 8
        ));
    });
    egui::Grid::new("clip-grid")
        .num_columns(CLIPS_PER_DECK + 1)
        .spacing([5.0, 5.0])
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new("SCENE")
                    .small()
                    .strong()
                    .color(palette.grid_text),
            );
            for slot in 0..CLIPS_PER_DECK {
                let scene_index = state.scene_bank * CLIPS_PER_DECK + slot;
                let scene = fixed_cell(ui, SCENE_CELL, |ui| {
                    mappable(
                        ui,
                        midi_map,
                        ControlTarget::SceneLaunch(scene_index as u8),
                        actions,
                        |ui| {
                            ui.add_sized(
                                SCENE_CELL,
                                egui::Button::new(
                                    egui::RichText::new(format!("{}", scene_index + 1))
                                        .small()
                                        .color(palette.grid_text),
                                )
                                .fill(palette.control_tint(palette.secondary, 0.22)),
                            )
                        },
                    )
                });
                if scene
                    .on_hover_text(format!(
                        "Launch scene {} on the next quantized boundary",
                        scene_index + 1
                    ))
                    .clicked()
                {
                    actions.push(UiAction::LaunchScene(scene_index));
                }
            }
            ui.end_row();

            for deck in DeckId::ALL {
                if mappable(
                    ui,
                    midi_map,
                    ControlTarget::DeckSelect(deck.index() as u8),
                    actions,
                    |ui| {
                        ui.selectable_label(
                            mixer.selected() == deck,
                            egui::RichText::new(format!("DECK {}", deck.label()))
                                .strong()
                                .color(palette.grid_text),
                        )
                        .on_hover_text("Select this deck's performance controls")
                    },
                )
                .clicked()
                {
                    mixer.select(deck);
                }
                for slot in 0..CLIPS_PER_DECK {
                    let address = ClipAddress { deck, slot };
                    let selected = clips.selected(deck) == slot && mixer.selected() == deck;
                    let active = clips.active(deck) == Some(slot);
                    let queued = launches.queued(address);
                    let slot_state = clips
                        .slot(address)
                        .cloned()
                        .expect("valid clip-grid address");
                    let first_frame_ready = state
                        .preloaded_frame(address, clips.path(address))
                        .is_some();
                    let label = if let Some(movie) = &slot_state.movie {
                        let name = movie
                            .display_name
                            .split('.')
                            .next()
                            .unwrap_or(&movie.display_name);
                        let short: String = name.chars().take(14).collect();
                        if queued {
                            format!("◷ {short}")
                        } else if active {
                            format!("▶ {short}")
                        } else if first_frame_ready {
                            format!("● {short}")
                        } else {
                            format!("○ {short}")
                        }
                    } else if slot_state.error.is_some() {
                        format!("⚠ {}{}", deck.label(), slot + 1)
                    } else if let Some(path) = &slot_state.pending_path {
                        format!(
                            "… {}",
                            path.file_stem()
                                .and_then(|name| name.to_str())
                                .unwrap_or("loading")
                        )
                    } else {
                        format!("{}{}", deck.label(), slot + 1)
                    };
                    let label = egui::RichText::new(label).color(palette.grid_text);
                    let button =
                        if let Some(thumbnail) = state.thumbnail(address, clips.path(address)) {
                            egui::Button::image_and_text(
                                egui::Image::new(thumbnail).fit_to_exact_size(CLIP_THUMBNAIL),
                                label,
                            )
                        } else {
                            let label = if state
                                .thumbnail_failure(address, clips.path(address))
                                .is_some()
                            {
                                egui::RichText::new(format!("□ {}", label.text()))
                                    .color(palette.grid_text)
                            } else {
                                label
                            };
                            egui::Button::new(label)
                        }
                        .truncate()
                        .selected(selected || active)
                        .fill(if active {
                            palette.control_tint(palette.success, 0.24)
                        } else if queued {
                            palette.control_tint(palette.warning, 0.22)
                        } else if selected {
                            palette.control_tint(palette.accent, 0.24)
                        } else {
                            palette.control
                        })
                        .stroke(egui::Stroke::new(
                            if active || queued {
                                palette.outline_width + 1.0
                            } else {
                                palette.outline_width
                            },
                            if active {
                                palette.success
                            } else if queued {
                                palette.warning
                            } else {
                                palette.stroke
                            },
                        ));
                    let draggable = !state.show_mode
                        && !midi_map.active
                        && (slot_state.movie.is_some()
                            || slot_state.pending_path.is_some()
                            || slot_state.error.is_some());
                    let response = fixed_cell(ui, CLIP_CELL, |ui| {
                        if midi_map.active {
                            mappable(
                                ui,
                                midi_map,
                                ControlTarget::ClipLaunch {
                                    deck: deck.index() as u8,
                                    slot: slot as u8,
                                },
                                actions,
                                |ui| ui.add_sized(CLIP_CELL, button),
                            )
                        } else if draggable {
                            // Drag moves the clip to another slot; a plain click
                            // still selects and launches because the drag only
                            // starts past egui's drag threshold.
                            ui.dnd_drag_source(
                                egui::Id::new(("clip-slot-drag", deck.index(), slot)),
                                address,
                                |ui| ui.add_sized(CLIP_CELL, button),
                            )
                            .inner
                        } else {
                            ui.add_sized(CLIP_CELL, button)
                        }
                    });
                    let play_rect = egui::Rect::from_min_size(
                        response.rect.left_top() + egui::vec2(3.0, 3.0),
                        egui::vec2(22.0, 20.0),
                    );
                    let play = ui.put(
                        play_rect,
                        egui::Button::new(egui::RichText::new("▶").small())
                            .fill(palette.control_tint(palette.success, 0.72)),
                    );
                    if play
                        .on_hover_text(if clips.movie(address).is_some() {
                            "Play this clip independently of scene launching"
                        } else {
                            "Open this clip slot"
                        })
                        .clicked()
                    {
                        clips.select(address);
                        mixer.select(deck);
                        if clips.movie(address).is_some() {
                            actions.push(UiAction::Launch(address));
                        } else {
                            state.clip_editor = Some(address);
                        }
                    }
                    drop_targets::register(
                        ui,
                        response.rect,
                        drop_targets::DropTarget::Clip(address),
                    );
                    let response = if draggable {
                        response.on_hover_cursor(egui::CursorIcon::Grab)
                    } else {
                        response
                    };
                    if let Some(source) = response.dnd_hover_payload::<ClipAddress>()
                        && *source != address
                    {
                        ui.painter().rect_stroke(
                            response.rect.expand(2.0),
                            6.0,
                            egui::Stroke::new(2.0, palette.accent),
                            egui::StrokeKind::Outside,
                        );
                    }
                    if let Some(source) = response.dnd_release_payload::<ClipAddress>()
                        && *source != address
                    {
                        actions.push(UiAction::MoveClip {
                            from: *source,
                            to: address,
                        });
                    }
                    if response.clicked() {
                        clips.select(address);
                        mixer.select(deck);
                        if clips.movie(address).is_some() {
                            actions.push(UiAction::Launch(address));
                        }
                    }
                    if response.double_clicked() {
                        clips.select(address);
                        mixer.select(deck);
                        state.clip_editor = Some(address);
                    }
                    response
                        .on_hover_text(if let Some(movie) = clips.movie(address) {
                            let mut details = format!(
                                "{}\n{}×{} · {}",
                                movie.display_name,
                                movie.visible_extent[0],
                                movie.visible_extent[1],
                                movie.codec
                            );
                            if movie.decode_path == virtual_media::DecodePath::FfmpegVideo {
                                details.push_str(&format!(
                                    "\n{} keyframe(s) indexed{}",
                                    movie.keyframes.len(),
                                    if movie.keyframes.is_complete() {
                                        ""
                                    } else {
                                        " · capped"
                                    }
                                ));
                            }
                            if let Some(error) =
                                state.thumbnail_failure(address, clips.path(address))
                            {
                                details.push_str(&format!("\nThumbnail unavailable: {error}"));
                            } else if first_frame_ready {
                                details.push_str("\nFirst frame ready for immediate launch");
                            } else {
                                details.push_str("\nFirst frame is still preloading");
                            }
                            if !state.show_mode {
                                details.push_str(
                                    "\nDrag onto another slot to move; occupied slots swap",
                                );
                            }
                            details
                        } else if let Some(error) = &slot_state.error {
                            format!(
                                "{}\n{error}",
                                slot_state.pending_path.as_deref().map_or_else(
                                    || "Missing media".to_owned(),
                                    |path| path.display().to_string()
                                )
                            )
                        } else if let Some(path) = &slot_state.pending_path {
                            format!("Restoring {}", path.display())
                        } else {
                            "Empty slot · drop a movie or folder here".to_owned()
                        })
                        .context_menu(|ui| {
                            if state.show_mode {
                                ui.weak("Show Mode locks media management");
                                return;
                            }
                            if clips.path(address).is_some() && ui.button("Relink media…").clicked()
                            {
                                actions.push(UiAction::BrowseRelink(address));
                                ui.close();
                            }
                            if clips.path(address).is_some() && ui.button("Delete clip").clicked() {
                                actions.push(UiAction::ClearSlot(address));
                                ui.close();
                            }
                        });
                }
                ui.end_row();
            }
        });

    let deck = mixer.selected();
    let address = ClipAddress {
        deck,
        slot: clips.selected(deck),
    };
    let selected_occupied = clips.slot(address).is_some_and(|slot| {
        slot.movie.is_some() || slot.pending_path.is_some() || slot.error.is_some()
    });
    ui.horizontal(|ui| {
        ui.strong(format!(
            "Selected · Deck {} clip {}",
            deck.label(),
            address.slot + 1
        ));
        let delete = buttons::midi_button(
            ui,
            "clips.delete_selected",
            "Clips · Delete selected clip",
            |ui| {
                ui.add_enabled(
                    selected_occupied && !state.show_mode,
                    egui::Button::new("Delete selected clip")
                        .fill(palette.control_tint(palette.danger, 0.28)),
                )
            },
        );
        if delete
            .on_hover_text(if state.show_mode {
                "Exit Show Mode to manage clips"
            } else {
                "Delete this slot · keyboard: Delete or Backspace"
            })
            .clicked()
        {
            actions.push(UiAction::ClearSlot(address));
        }
    });

    let recording = camera_recordings[deck.index()];
    super::video_input::draw_video_input(ui, state, deck, cameras, camera_status, actions);
    super::generator::draw_generator_source(ui, state, deck, actions);
    ui.horizontal_wrapped(|ui| {
        if recording.address.is_some() {
            let label = if recording.finalizing {
                "Finalizing…".to_owned()
            } else {
                format!("■ Stop · {:.1}s", recording.elapsed_seconds)
            };
            if buttons::midi_button(
                ui,
                &format!("deck.{}.record", deck.index()),
                &format!("Deck {} · Record / stop clip", deck.label()),
                |ui| ui.add_enabled(!recording.finalizing, egui::Button::new(label)),
            )
            .clicked()
            {
                actions.push(UiAction::StopCameraRecording(deck));
            }
            if recording.dropped_frames > 0 {
                ui.colored_label(
                    palette.warning,
                    format!("{} dropped", recording.dropped_frames),
                );
            }
        } else {
            let live = matches!(
                mixer.deck(deck).state,
                DeckState::Live(_) | DeckState::Generator(_)
            );
            let can_record = live && !selected_occupied;
            if buttons::midi_button(
                ui,
                &format!("deck.{}.record", deck.index()),
                &format!("Deck {} · Record / stop clip", deck.label()),
                |ui| {
                    ui.add_enabled(
                        can_record,
                        egui::Button::new("● Record clip")
                            .fill(palette.control_tint(palette.danger, 0.28)),
                    )
                    .on_hover_text(if !live {
                        "Connect this deck to a video input or generator first"
                    } else if selected_occupied {
                        "Select an empty clip slot to record into"
                    } else {
                        "Record this deck's live source into the selected clip slot"
                    })
                },
            )
            .clicked()
            {
                actions.push(UiAction::StartCameraRecording(address));
            }
        }
        if let DeckState::Live(config) = &mixer.deck(deck).state
            && let (Some([width, height]), Some(fps)) = (config.requested_extent, config.requested_fps)
        {
            let megabytes = f64::from(width) * f64::from(height) * 4.0 * f64::from(fps) / f64::from(config.fps_denominator.max(1)) / 1_000_000.0;
            ui.weak(format!("Raw recording ≈ {megabytes:.0} MB/s · {:.1} GB/min", megabytes * 60.0 / 1000.0))
                .on_hover_text("Storage estimate from the requested camera format. Actual capture rate may differ.");
        }
    });

    if let Some(editor_address) = state.clip_editor {
        draw_clip_editor(ui.ctx(), state, clips, editor_address);
    }

    if state.show_mode {
        return;
    }
    if let Some(movie) = clips.movie(address) {
        let name = movie.display_name.clone();
        let media_duration = movie.duration.map(virtual_core::MediaTime::as_seconds);
        let mut playback = clips.playback(address).unwrap_or_default();
        let mut changed = false;
        egui::CollapsingHeader::new(format!("Selected clip playback · {name}"))
            .default_open(false)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Launch");
                    changed |= buttons::midi_select(
                        ui,
                        "clips.selected.launch_restart",
                        "Selected clip · Launch restarts at In",
                        &mut playback.launch_mode,
                        ClipLaunchMode::Restart,
                        "Restart at In",
                    )
                    .changed();
                    changed |= buttons::midi_select(
                        ui,
                        "clips.selected.launch_resume",
                        "Selected clip · Launch resumes",
                        &mut playback.launch_mode,
                        ClipLaunchMode::Resume,
                        "Resume last position",
                    )
                    .changed();
                });
                let maximum = media_duration.unwrap_or(86_400.0).max(0.001);
                ui.horizontal(|ui| {
                    changed |= ui
                        .add(
                            egui::DragValue::new(&mut playback.in_point)
                                .range(0.0..=maximum)
                                .speed(0.05)
                                .suffix(" s")
                                .prefix("In "),
                        )
                        .changed();
                    let mut out_enabled = playback.out_point.is_some();
                    if buttons::midi_toggle(
                        ui,
                        "clips.selected.out_point",
                        "Selected clip · Out point",
                        &mut out_enabled,
                        |ui, value| ui.checkbox(value, "Out"),
                    )
                    .changed()
                    {
                        playback.out_point = out_enabled.then_some(maximum);
                        changed = true;
                    }
                    if let Some(out_point) = &mut playback.out_point {
                        changed |= ui
                            .add(
                                egui::DragValue::new(out_point)
                                    .range(0.001..=maximum)
                                    .speed(0.05)
                                    .suffix(" s"),
                            )
                            .changed();
                    }
                });
                ui.horizontal(|ui| {
                    let mut beat_enabled = playback.beat_duration.is_some();
                    if buttons::midi_toggle(
                        ui,
                        "clips.selected.beat_duration",
                        "Selected clip · BPM-relative duration",
                        &mut beat_enabled,
                        |ui, value| ui.checkbox(value, "BPM-relative duration"),
                    )
                    .changed()
                    {
                        playback.beat_duration = beat_enabled.then_some(4.0);
                        changed = true;
                    }
                    if let Some(beats) = &mut playback.beat_duration {
                        changed |= ui
                            .add(
                                egui::DragValue::new(beats)
                                    .range(0.0625..=256.0)
                                    .speed(0.25)
                                    .suffix(" beats"),
                            )
                            .changed();
                    }
                    if let Some(beats) = playback.beat_duration {
                        ui.weak(format!(
                            "{:.3} s at {:.1} BPM",
                            beats * 60.0 / state.bpm,
                            state.bpm
                        ));
                    }
                });
                let (start, end) = playback.range(media_duration, state.bpm);
                ui.weak(match end {
                    Some(end) => format!("Effective range {start:.3}–{end:.3} s"),
                    None => format!("Effective range starts at {start:.3} s"),
                });
            });
        if changed {
            clips.set_playback(address, playback);
        }
    } else if let Some(path) = clips.path(address) {
        let path = path.to_path_buf();
        egui::CollapsingHeader::new(format!(
            "Missing media · Deck {} slot {}",
            deck.label(),
            address.slot + 1
        ))
        .default_open(true)
        .show(ui, |ui| {
            ui.colored_label(palette.danger, path.display().to_string());
            if ui.button("Browse and relink…").clicked() {
                actions.push(UiAction::BrowseRelink(address));
            }
            ui.weak("Trim, launch mode and beat-duration settings will be preserved.");
        });
    }
}

fn draw_clip_editor(
    ctx: &egui::Context,
    state: &mut UiState,
    clips: &mut ClipBank,
    address: ClipAddress,
) {
    let Some(movie) = clips.movie(address) else {
        state.clip_editor = None;
        return;
    };
    let title = format!("Clip Editor · {}", movie.display_name);
    let media_duration = movie.duration.map(virtual_core::MediaTime::as_seconds);
    let playback = clips.playback(address).unwrap_or_default();
    let mut edited = playback;
    let mut changed = false;
    let mut open = true;
    egui::Window::new(title)
        .id(egui::Id::new(("clip-editor", address.deck.index(), address.slot)))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(560.0)
        .show(ctx, |ui| {
            ui.label(format!("Deck {} · Clip {}", address.deck.label(), address.slot + 1));
            ui.weak(format!(
                "{}×{} · {} · {}",
                movie.visible_extent[0], movie.visible_extent[1], movie.codec,
                media_duration.map_or_else(|| "unknown duration".to_owned(), |seconds| format!("{seconds:.3} s source"))
            ));
            ui.separator();
            ui.horizontal(|ui| {
                ui.strong("Source range");
                let maximum = media_duration.unwrap_or(86_400.0).max(0.001);
                changed |= ui.add(egui::DragValue::new(&mut edited.in_point).range(0.0..=maximum).speed(0.05).suffix(" s").prefix("In ")).changed();
                let mut out_enabled = edited.out_point.is_some();
                if ui.checkbox(&mut out_enabled, "Out").changed() {
                    edited.out_point = out_enabled.then_some(maximum);
                    changed = true;
                }
                if let Some(out) = &mut edited.out_point {
                    changed |= ui.add(egui::DragValue::new(out).range(0.001..=maximum).speed(0.05).suffix(" s")).changed();
                }
            });
            let (start, end) = edited.range(media_duration, state.bpm);
            let effective_end = end.unwrap_or_else(|| media_duration.unwrap_or(start + 1.0));
            let span = (effective_end - start).max(0.001);
            ui.add(egui::Slider::new(&mut edited.in_point, 0.0..=media_duration.unwrap_or(86_400.0).max(0.001)).text("Crop start (In)").suffix(" s"));
            if edited.out_point.is_none() && edited.beat_duration.is_none() {
                ui.weak("Set an Out point or beat length to crop the clip's end. Otherwise it plays to source end.");
            }
            let source_duration = media_duration.unwrap_or(effective_end).max(0.001);
            let fraction = (span / source_duration).clamp(0.0, 1.0) as f32;
            let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 30.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 4.0, ui.visuals().widgets.inactive.bg_fill);
            let selected = egui::Rect::from_min_max(
                egui::pos2(rect.left() + rect.width() * (start / source_duration) as f32, rect.top()),
                egui::pos2(rect.left() + rect.width() * ((start / source_duration) as f32 + fraction).clamp(0.0, 1.0), rect.bottom()),
            );
            ui.painter().rect_filled(selected, 3.0, state.theme.palette().accent);
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, format!("{start:.2}–{effective_end:.2} s · {span:.2} s clip"), egui::FontId::proportional(12.0), ui.visuals().text_color());
            ui.separator();
            ui.horizontal(|ui| {
                ui.strong("Length");
                let mut beat_enabled = edited.beat_duration.is_some();
                if ui.checkbox(&mut beat_enabled, "Tempo synced").changed() {
                    edited.beat_duration = beat_enabled.then_some(4.0);
                    changed = true;
                }
                if let Some(beats) = &mut edited.beat_duration {
                    changed |= ui.add(egui::DragValue::new(beats).range(0.0625..=256.0).speed(0.25).suffix(" beats")).changed();
                    ui.weak(format!("{:.2} s at {:.1} BPM", *beats * 60.0 / state.bpm, state.bpm));
                } else if let Some(end) = edited.out_point {
                    ui.weak(format!("{:.3} s", (end - edited.in_point).max(0.0)));
                } else if let Some(duration) = media_duration {
                    ui.weak(format!("{:.3} s", (duration - edited.in_point).max(0.0)));
                }
            });
            ui.horizontal(|ui| {
                ui.strong("Launch");
                changed |= ui.selectable_value(&mut edited.launch_mode, ClipLaunchMode::Restart, "Restart at In").changed();
                changed |= ui.selectable_value(&mut edited.launch_mode, ClipLaunchMode::Resume, "Resume position").changed();
            });
            ui.separator();
            ui.strong("Automation");
            ui.weak("Clip automation lanes are not yet attached to clip playback. The curve editor will be enabled when that playback path is connected.");
            ui.horizontal(|ui| {
                if ui.button("Close").clicked() {
                    state.clip_editor = None;
                }
                if ui.button("Reset crop").clicked() {
                    edited.in_point = 0.0;
                    edited.out_point = None;
                    edited.beat_duration = None;
                    changed = true;
                }
            });
        });
    if changed {
        clips.set_playback(address, edited);
    }
    if !open {
        state.clip_editor = None;
    }
}
