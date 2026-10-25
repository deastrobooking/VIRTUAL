//! Clip grid: slots, health, launch and thumbnail rendering, in two
//! arrangements: Session (decks as condensed horizontal lanes of filmstrip
//! clips, like a video editor's tracks) and Launcher (decks as columns with
//! scene launchers and stop buttons, after Ableton Live).

use super::theme::{ClipSize, ClipView};
use super::*;

/// Width of the per-deck controls at the left of each Session lane.
const LANE_HEADER_WIDTH: f32 = 176.0;
/// Width of the scene launch column in Session view.
const SCENE_LAUNCH_WIDTH: f32 = 64.0;
const HEADER_HEIGHT: f32 = 22.0;

/// Gives a grid cell an exact rect. A child scope placed straight into the
/// grid (drag sources, MIDI-map overlays) gets a max rect reaching the bottom
/// of the panel and the row's centred layout, which drops each button lower
/// than the one before and grows the row.
fn fixed_cell<R>(ui: &mut egui::Ui, size: egui::Vec2, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    fixed_layout(
        ui,
        size,
        egui::Layout::centered_and_justified(egui::Direction::TopDown),
        add,
    )
}

/// Like [`fixed_cell`] but stacking its contents from the top left.
fn fixed_column<R>(ui: &mut egui::Ui, size: egui::Vec2, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    fixed_layout(ui, size, egui::Layout::top_down(egui::Align::Min), add)
}

fn fixed_layout<R>(
    ui: &mut egui::Ui,
    size: egui::Vec2,
    layout: egui::Layout,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    ui.allocate_ui_with_layout(size, layout, |ui| {
        ui.set_min_size(size);
        ui.set_max_size(size);
        add(ui)
    })
    .inner
}

pub(super) struct ClipGridContext<'a> {
    pub launches: &'a LaunchQueue,
    pub midi_map: &'a MidiMapUi,
    pub cameras: &'a [CameraDevice],
    pub camera_status: &'a str,
    pub camera_recordings: [CameraRecordingStatus; 4],
    pub transports: &'a [DeckTransport; 4],
    pub beat_position: f64,
}

/// What a clip tile shows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TileState {
    Empty,
    Loading,
    Missing,
    /// Loaded, first frame not yet preloaded.
    Idle,
    /// First frame preloaded: launches instantly.
    Ready,
    Queued,
    Playing,
}

struct Tile<'a> {
    thumbnail: Option<&'a egui::TextureHandle>,
    thumbnail_failed: bool,
    name: String,
    state: TileState,
    selected: bool,
    deck_color: egui::Color32,
    /// Playback position through the clip, while it plays.
    progress: Option<f32>,
    /// Media length in seconds.
    duration: Option<f64>,
}

/// How a clip slot is drawn.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TileStyle {
    /// Launcher: thumbnail over a name strip.
    Card,
    /// Session lanes: the thumbnail repeated as a filmstrip with overlaid
    /// labels and a playhead.
    Filmstrip,
}

/// `m:ss` (or `h:mm:ss`) for clip lengths.
fn format_duration(seconds: f64) -> String {
    let total = seconds.max(0.0).round() as u64;
    let (hours, minutes, seconds) = (total / 3600, total / 60 % 60, total % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// The border every clip style uses for its state.
fn tile_stroke(
    ui: &egui::Ui,
    tile: &Tile<'_>,
    hovered: bool,
    palette: ThemePalette,
) -> egui::Stroke {
    match tile.state {
        TileState::Playing => egui::Stroke::new(2.0, palette.success),
        TileState::Queued => {
            let pulse = (ui.input(|input| input.time) * 6.0).sin() as f32 * 0.5 + 0.5;
            ui.ctx().request_repaint();
            egui::Stroke::new(2.0, palette.warning.gamma_multiply(0.35 + 0.65 * pulse))
        }
        _ if tile.selected => egui::Stroke::new(1.5, palette.accent),
        _ if hovered => egui::Stroke::new(1.0, palette.text.gamma_multiply(0.6)),
        _ => egui::Stroke::new(palette.outline_width.min(1.0), palette.stroke),
    }
}

/// UV rectangle that crops a texture to `aspect` (width / height), centred.
fn cover_uv(texture: &egui::TextureHandle, aspect: f32) -> egui::Rect {
    let [width, height] = texture.size().map(|v| v.max(1) as f32);
    let texture_aspect = width / height;
    if texture_aspect > aspect {
        let u = aspect / texture_aspect;
        egui::Rect::from_min_max(
            egui::pos2(0.5 - u / 2.0, 0.0),
            egui::pos2(0.5 + u / 2.0, 1.0),
        )
    } else {
        let v = texture_aspect / aspect;
        egui::Rect::from_min_max(
            egui::pos2(0.0, 0.5 - v / 2.0),
            egui::pos2(1.0, 0.5 + v / 2.0),
        )
    }
}

/// A Session-lane clip: the thumbnail tiled as a filmstrip, the name and
/// length in small dark labels, a deck-colour bar along the bottom and a
/// playhead across the playing clip, as in a video editor's timeline.
fn filmstrip_widget(
    ui: &mut egui::Ui,
    size: egui::Vec2,
    tile: &Tile<'_>,
    palette: ThemePalette,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let hovered = response.hovered();
    let radius = egui::CornerRadius::same(ui.visuals().widgets.inactive.corner_radius.nw.min(4));
    let painter = ui.painter();
    if tile.state == TileState::Empty {
        painter.rect_filled(rect, radius, palette.faint);
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "+",
            egui::FontId::proportional((rect.height() * 0.4).clamp(10.0, 20.0)),
            palette.idle.gamma_multiply(if hovered { 1.0 } else { 0.6 }),
        );
        painter.rect_stroke(
            rect,
            radius,
            tile_stroke(ui, tile, hovered, palette),
            egui::StrokeKind::Inside,
        );
        return response;
    }
    painter.rect_filled(rect, radius, palette.extreme);
    let inner = rect.shrink(1.0);
    if let Some(texture) = tile.thumbnail {
        let frame_width = inner.height() * 16.0 / 9.0;
        let uv = cover_uv(texture, 16.0 / 9.0);
        let tint = if tile.state == TileState::Playing || hovered {
            egui::Color32::WHITE
        } else {
            egui::Color32::from_gray(190)
        };
        let clipped = painter.with_clip_rect(inner);
        let frames = (inner.width() / frame_width).ceil() as usize;
        for index in 0..frames {
            let left = inner.min.x + index as f32 * frame_width;
            let frame = egui::Rect::from_min_size(
                egui::pos2(left, inner.min.y),
                egui::vec2(frame_width, inner.height()),
            );
            clipped.image(texture.id(), frame, uv, tint);
            if index > 0 {
                clipped.line_segment(
                    [frame.left_top(), frame.left_bottom()],
                    egui::Stroke::new(1.0, egui::Color32::from_black_alpha(140)),
                );
            }
        }
    } else {
        let (glyph, color) = match tile.state {
            TileState::Loading => ("…", palette.muted_text),
            TileState::Missing => ("⚠", palette.danger),
            _ if tile.thumbnail_failed => ("▢", palette.muted_text),
            _ => ("▶", palette.idle),
        };
        painter.text(
            inner.center(),
            egui::Align2::CENTER_CENTER,
            glyph,
            egui::FontId::proportional((inner.height() * 0.45).clamp(10.0, 24.0)),
            color,
        );
    }
    let bar = egui::Rect::from_min_max(egui::pos2(inner.min.x, inner.max.y - 3.0), inner.max);
    painter.rect_filled(bar, 0.0, tile.deck_color);
    if let Some(progress) = tile.progress {
        let x = inner.min.x + inner.width() * progress.clamp(0.0, 1.0);
        painter.rect_filled(
            egui::Rect::from_min_max(inner.min, egui::pos2(x, inner.max.y)),
            0.0,
            palette.success.gamma_multiply(0.18),
        );
        painter.line_segment(
            [egui::pos2(x, inner.min.y), egui::pos2(x, inner.max.y)],
            egui::Stroke::new(2.0, palette.strong_text),
        );
        ui.ctx().request_repaint();
    }
    let font = egui::FontId::proportional((rect.height() * 0.24).clamp(9.0, 12.0));
    let label = |text: &str, anchor: egui::Pos2, align: egui::Align2, color: egui::Color32| {
        let galley = painter.layout_no_wrap(text.to_owned(), font.clone(), color);
        let size = galley.size() + egui::vec2(6.0, 2.0);
        let pill = align.anchor_size(anchor, size).intersect(inner);
        painter.rect_filled(pill, 2.0, egui::Color32::from_black_alpha(165));
        painter
            .with_clip_rect(pill)
            .galley(pill.min + egui::vec2(3.0, 1.0), galley, color);
    };
    let name_color = match tile.state {
        TileState::Missing => palette.danger,
        TileState::Queued => palette.warning,
        _ => egui::Color32::WHITE,
    };
    // Leave room for the play button in the top-left corner.
    label(
        &tile.name,
        inner.left_top() + egui::vec2(22.0, 2.0),
        egui::Align2::LEFT_TOP,
        name_color,
    );
    let mut info = String::new();
    match tile.state {
        TileState::Ready => info.push('●'),
        TileState::Idle => info.push('○'),
        TileState::Queued => info.push('◷'),
        _ => {}
    }
    if let Some(duration) = tile.duration {
        if !info.is_empty() {
            info.push(' ');
        }
        info.push_str(&format_duration(duration));
    }
    if !info.is_empty() && rect.height() >= 30.0 {
        let color = if tile.state == TileState::Ready {
            palette.success
        } else {
            egui::Color32::from_gray(220)
        };
        label(
            &info,
            inner.right_bottom() - egui::vec2(2.0, 5.0),
            egui::Align2::RIGHT_BOTTOM,
            color,
        );
    }
    painter.rect_stroke(
        rect,
        radius,
        tile_stroke(ui, tile, hovered, palette),
        egui::StrokeKind::Inside,
    );
    response
}

/// A clip tile: the thumbnail fills the tile above a name strip, a thin
/// bar in the deck colour marks filled slots, and the border and strip show
/// playing (play colour), queued (pulsing warning) and selected (accent).
fn tile_widget(
    ui: &mut egui::Ui,
    size: egui::Vec2,
    tile: &Tile<'_>,
    palette: ThemePalette,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let hovered = response.hovered();
    let radius = ui.visuals().widgets.inactive.corner_radius;
    let strip_height = (size.y - size.x * 9.0 / 16.0).max(12.0);
    let image =
        egui::Rect::from_min_max(rect.min, egui::pos2(rect.max.x, rect.max.y - strip_height));
    let strip = egui::Rect::from_min_max(egui::pos2(rect.min.x, image.max.y), rect.max);
    let painter = ui.painter();
    let empty = tile.state == TileState::Empty;
    painter.rect_filled(
        rect,
        radius,
        if empty {
            palette.faint
        } else {
            palette.extreme
        },
    );
    if let Some(texture) = tile.thumbnail {
        // Cover-fit: crop the thumbnail to the tile's aspect.
        let uv = cover_uv(texture, image.width() / image.height());
        let tint = if tile.state == TileState::Playing || hovered {
            egui::Color32::WHITE
        } else {
            egui::Color32::from_gray(200)
        };
        painter.image(texture.id(), image, uv, tint);
    } else {
        let (glyph, color) = match tile.state {
            TileState::Empty => ("+", palette.idle),
            TileState::Loading => ("…", palette.muted_text),
            TileState::Missing => ("⚠", palette.danger),
            _ if tile.thumbnail_failed => ("▢", palette.muted_text),
            _ => ("▶", palette.idle),
        };
        painter.text(
            image.center(),
            egui::Align2::CENTER_CENTER,
            glyph,
            egui::FontId::proportional((image.height() * 0.42).max(10.0)),
            if empty && !hovered {
                color.gamma_multiply(0.6)
            } else {
                color
            },
        );
    }
    if !empty {
        painter.rect_filled(
            egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), 2.5)),
            egui::CornerRadius {
                nw: radius.nw,
                ne: radius.ne,
                sw: 0,
                se: 0,
            },
            tile.deck_color,
        );
    }
    let strip_fill = match tile.state {
        TileState::Playing => palette.control_tint(palette.success, 0.55),
        TileState::Queued => palette.control_tint(palette.warning, 0.4),
        TileState::Missing => palette.control_tint(palette.danger, 0.3),
        _ if tile.selected => palette.control_tint(palette.accent, 0.35),
        _ => palette.control,
    };
    painter.rect_filled(
        strip,
        egui::CornerRadius {
            nw: 0,
            ne: 0,
            sw: radius.sw,
            se: radius.se,
        },
        strip_fill,
    );
    let font = egui::FontId::proportional((strip_height * 0.68).clamp(9.0, 13.0));
    let marker = match tile.state {
        TileState::Playing => "▶",
        TileState::Queued => "◷",
        TileState::Ready => "●",
        TileState::Idle => "○",
        _ => "",
    };
    let text_color = match tile.state {
        TileState::Playing | TileState::Queued => palette.selected_text,
        TileState::Empty => palette.muted_text,
        _ => palette.grid_text,
    };
    let text_area = strip.shrink2(egui::vec2(4.0, 0.0));
    painter.with_clip_rect(text_area).text(
        text_area.left_center(),
        egui::Align2::LEFT_CENTER,
        &tile.name,
        font.clone(),
        text_color,
    );
    if !marker.is_empty() {
        let marker_color = match tile.state {
            TileState::Ready => palette.success,
            TileState::Idle => palette.muted_text,
            _ => palette.selected_text,
        };
        painter.text(
            egui::pos2(strip.max.x - 4.0, strip.center().y),
            egui::Align2::RIGHT_CENTER,
            marker,
            font,
            marker_color,
        );
    }
    let stroke = tile_stroke(ui, tile, hovered, palette);
    painter.rect_stroke(rect, radius, stroke, egui::StrokeKind::Inside);
    response
}

pub(super) fn draw_clip_grid(
    ui: &mut egui::Ui,
    state: &mut UiState,
    mixer: &mut FourDeckMixer,
    clips: &mut ClipBank,
    context: ClipGridContext<'_>,
    actions: &mut Vec<UiAction>,
) {
    let cameras = context.cameras;
    let camera_status = context.camera_status;
    let camera_recordings = context.camera_recordings;
    let transports = context.transports;
    let palette = state.theme.palette();
    ui.horizontal_wrapped(|ui| {
        for view in ClipView::ALL {
            ui.selectable_value(&mut state.theme.clip_view, view, view.label())
                .on_hover_text(match view {
                    ClipView::Session => {
                        "Decks as condensed lanes of filmstrip clips, like editing tracks"
                    }
                    ClipView::Launcher => {
                        "Decks as columns; scenes launch from the right, stop buttons below"
                    }
                });
        }
        ui.separator();
        for size in ClipSize::ALL {
            ui.selectable_value(&mut state.theme.clip_size, size, size.label())
                .on_hover_text("Clip size");
        }
        ui.separator();
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
    let size = state.theme.clip_size;
    match state.theme.clip_view {
        ClipView::Session => lanes_view(ui, state, mixer, clips, &context, actions, size),
        ClipView::Launcher => {
            launcher_grid(ui, state, mixer, clips, &context, actions, size.tile());
        }
    }

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
        automation_switch(ui, state, deck);
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
        let transport = transports[editor_address.deck.index()];
        let playhead_phase = clips
            .active_automation(editor_address.deck)
            .filter(|(slot, _)| {
                *slot == editor_address.slot
                    && transport.playing
                    && !transport.frozen
                    && !state.master_freeze
            })
            .and_then(|(_, start)| {
                clips.automation(editor_address).map(|automation| {
                    ((context.beat_position - start).rem_euclid(automation.loop_beats)
                        / automation.loop_beats)
                        .clamp(0.0, 1.0)
                })
            });
        draw_clip_editor(ui.ctx(), state, clips, editor_address, playhead_phase);
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

/// Video-editor style: one condensed lane per deck with its controls in the
/// lane header, clips as filmstrips, and scene triggers along the top.
fn lanes_view(
    ui: &mut egui::Ui,
    state: &mut UiState,
    mixer: &mut FourDeckMixer,
    clips: &mut ClipBank,
    context: &ClipGridContext<'_>,
    actions: &mut Vec<UiAction>,
    size: ClipSize,
) {
    let palette = state.theme.palette();
    let height = size.lane_height();
    let block = egui::vec2((height * 16.0 / 9.0 * 1.6).round(), height);
    egui::Grid::new("clip-lanes")
        .num_columns(CLIPS_PER_DECK + 1)
        .spacing([2.0, 3.0])
        .show(ui, |ui| {
            fixed_cell(ui, egui::vec2(LANE_HEADER_WIDTH, 18.0), |ui| {
                ui.label(
                    egui::RichText::new("SCENES")
                        .small()
                        .strong()
                        .color(palette.muted_text),
                )
            });
            for slot in 0..CLIPS_PER_DECK {
                let scene_index = state.scene_bank * CLIPS_PER_DECK + slot;
                scene_cell(
                    ui,
                    state,
                    clips,
                    context.midi_map,
                    actions,
                    scene_index,
                    egui::vec2(block.x, 18.0),
                    &format!("▼ {}", scene_index + 1),
                );
            }
            ui.end_row();
            for deck in DeckId::ALL {
                fixed_column(ui, egui::vec2(LANE_HEADER_WIDTH, height), |ui| {
                    lane_header(
                        ui,
                        state,
                        mixer,
                        clips,
                        context.midi_map,
                        actions,
                        deck,
                        height,
                    );
                });
                for slot in 0..CLIPS_PER_DECK {
                    clip_cell(
                        ui,
                        state,
                        mixer,
                        clips,
                        context,
                        actions,
                        ClipAddress { deck, slot },
                        block,
                        TileStyle::Filmstrip,
                    );
                }
                ui.end_row();
            }
        });
}

/// Ableton-style: one column per deck under its title bar, one row per
/// scene with its launcher on the right, stop buttons along the bottom.
fn launcher_grid(
    ui: &mut egui::Ui,
    state: &mut UiState,
    mixer: &mut FourDeckMixer,
    clips: &mut ClipBank,
    context: &ClipGridContext<'_>,
    actions: &mut Vec<UiAction>,
    tile: egui::Vec2,
) {
    let palette = state.theme.palette();
    let header = egui::vec2(tile.x, 44.0);
    egui::Grid::new("clip-grid-session")
        .num_columns(DeckId::ALL.len() + 1)
        .spacing([4.0, 4.0])
        .show(ui, |ui| {
            for deck in DeckId::ALL {
                fixed_column(ui, header, |ui| {
                    track_header(ui, state, mixer, context.midi_map, actions, deck, tile.x);
                });
            }
            fixed_cell(ui, egui::vec2(SCENE_LAUNCH_WIDTH, header.y), |ui| {
                ui.label(
                    egui::RichText::new("SCENES")
                        .small()
                        .strong()
                        .color(palette.muted_text),
                )
            });
            ui.end_row();
            for slot in 0..CLIPS_PER_DECK {
                for deck in DeckId::ALL {
                    clip_cell(
                        ui,
                        state,
                        mixer,
                        clips,
                        context,
                        actions,
                        ClipAddress { deck, slot },
                        tile,
                        TileStyle::Card,
                    );
                }
                let scene_index = state.scene_bank * CLIPS_PER_DECK + slot;
                scene_cell(
                    ui,
                    state,
                    clips,
                    context.midi_map,
                    actions,
                    scene_index,
                    egui::vec2(SCENE_LAUNCH_WIDTH, tile.y),
                    &format!("▶ {}", scene_index + 1),
                );
                ui.end_row();
            }
            for deck in DeckId::ALL {
                let enabled =
                    !state.show_mode && !matches!(mixer.deck(deck).state, DeckState::Empty);
                let stop = fixed_cell(ui, egui::vec2(tile.x, HEADER_HEIGHT), |ui| {
                    buttons::midi_button(
                        ui,
                        &format!("clips.deck.{}.stop", deck.index()),
                        &format!("Clips · Stop deck {}", deck.label()),
                        |ui| ui.add_enabled(enabled, egui::Button::new("■")),
                    )
                });
                if stop
                    .on_hover_text(format!("Stop deck {}: eject its clip", deck.label()))
                    .clicked()
                {
                    actions.push(UiAction::Eject(deck));
                }
            }
            let any = !state.show_mode
                && DeckId::ALL
                    .iter()
                    .any(|deck| !matches!(mixer.deck(*deck).state, DeckState::Empty));
            let stop_all = fixed_cell(ui, egui::vec2(SCENE_LAUNCH_WIDTH, HEADER_HEIGHT), |ui| {
                buttons::midi_button(ui, "clips.stop_all", "Clips · Stop all decks", |ui| {
                    ui.add_enabled(
                        any,
                        egui::Button::new("■ All").fill(palette.control_tint(palette.danger, 0.2)),
                    )
                })
            });
            if stop_all
                .on_hover_text("Stop every deck: eject all clips")
                .clicked()
            {
                for deck in DeckId::ALL {
                    if !matches!(mixer.deck(deck).state, DeckState::Empty) {
                        actions.push(UiAction::Eject(deck));
                    }
                }
            }
            ui.end_row();
        });
}

/// X (eject), B (bypass) and S (solo), as on a Resolume layer.
fn deck_toggles(
    ui: &mut egui::Ui,
    state: &mut UiState,
    mixer: &FourDeckMixer,
    midi_map: &MidiMapUi,
    actions: &mut Vec<UiAction>,
    deck: DeckId,
) {
    let palette = state.theme.palette();
    let index = deck.index();
    let small = |text: &str| egui::RichText::new(text).small().strong();
    let size = egui::vec2(22.0, 18.0);
    let eject_enabled = !state.show_mode && !matches!(mixer.deck(deck).state, DeckState::Empty);
    if buttons::midi_button(
        ui,
        &format!("clips.deck.{index}.eject"),
        &format!("Clips · Deck {} eject", deck.label()),
        |ui| ui.add_enabled(eject_enabled, egui::Button::new(small("X")).min_size(size)),
    )
    .on_hover_text("Eject the playing clip")
    .clicked()
    {
        actions.push(UiAction::Eject(deck));
    }
    let bypassed = state.bypassed[index];
    if mappable(
        ui,
        midi_map,
        ControlTarget::DeckMute(index as u8),
        actions,
        |ui| {
            ui.add(
                egui::Button::new(small("B"))
                    .min_size(size)
                    .selected(bypassed)
                    .fill(if bypassed {
                        palette.control_tint(palette.warning, 0.6)
                    } else {
                        palette.control
                    }),
            )
        },
    )
    .on_hover_text("Bypass: hide this deck from the mix")
    .clicked()
    {
        state.bypassed[index] = !bypassed;
    }
    let solo = state.solo[index];
    if buttons::midi_button(
        ui,
        &format!("clips.deck.{index}.solo"),
        &format!("Clips · Deck {} solo", deck.label()),
        |ui| {
            ui.add(
                egui::Button::new(small("S"))
                    .min_size(size)
                    .selected(solo)
                    .fill(if solo {
                        palette.control_tint(palette.accent, 0.6)
                    } else {
                        palette.control
                    }),
            )
        },
    )
    .on_hover_text("Solo: show only soloed decks")
    .clicked()
    {
        state.solo[index] = !solo;
    }
}

fn deck_level(
    ui: &mut egui::Ui,
    mixer: &mut FourDeckMixer,
    midi_map: &MidiMapUi,
    actions: &mut Vec<UiAction>,
    deck: DeckId,
    width: f32,
) {
    let level = &mut mixer.deck_mut(deck).level;
    mappable(
        ui,
        midi_map,
        ControlTarget::DeckLevel(deck.index() as u8),
        actions,
        |ui| {
            ui.spacing_mut().slider_width = width;
            ui.add(
                egui::Slider::new(level, 0.0..=1.0)
                    .show_value(false)
                    .clamping(egui::SliderClamping::Always),
            )
            .on_hover_text("Deck level")
        },
    );
}

/// A square badge with the deck letter in its colour, like a video
/// editor's track label; click to select the deck.
fn deck_badge(
    ui: &mut egui::Ui,
    mixer: &mut FourDeckMixer,
    midi_map: &MidiMapUi,
    actions: &mut Vec<UiAction>,
    deck: DeckId,
    palette: ThemePalette,
) {
    let selected = mixer.selected() == deck;
    let color = palette.deck_color(deck);
    let badge = mappable(
        ui,
        midi_map,
        ControlTarget::DeckSelect(deck.index() as u8),
        actions,
        |ui| {
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(22.0, 18.0), egui::Sense::click());
            let fill = if selected {
                color
            } else {
                palette.surface_tint(color, if response.hovered() { 0.55 } else { 0.32 })
            };
            ui.painter().rect_filled(rect, 3.0, fill);
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                deck.label(),
                egui::FontId::proportional(12.0),
                if selected {
                    egui::Color32::BLACK
                } else {
                    palette.strong_text
                },
            );
            response.on_hover_text(format!("Select deck {}", deck.label()))
        },
    );
    if badge.clicked() {
        mixer.select(deck);
    }
}

/// The header of a Session lane: deck badge, what is playing, X/B/S and the
/// level. Short lanes put everything on one row.
#[allow(clippy::too_many_arguments)]
fn lane_header(
    ui: &mut egui::Ui,
    state: &mut UiState,
    mixer: &mut FourDeckMixer,
    clips: &ClipBank,
    midi_map: &MidiMapUi,
    actions: &mut Vec<UiAction>,
    deck: DeckId,
    height: f32,
) {
    let palette = state.theme.palette();
    let color = palette.deck_color(deck);
    let rect = ui.max_rect();
    ui.painter()
        .rect_filled(rect, 3.0, palette.surface_tint(color, 0.07));
    ui.painter().rect_filled(
        egui::Rect::from_min_size(rect.min, egui::vec2(3.0, rect.height())),
        1.0,
        color,
    );
    let tall = height >= 48.0;
    let rows = if tall { 2.0 } else { 1.0 };
    ui.add_space(((height - rows * 20.0 - (rows - 1.0) * 4.0) / 2.0).max(1.0));
    let toggles_width = 3.0 * 22.0 + 2.0 * ui.spacing().item_spacing.x;
    if tall {
        ui.horizontal(|ui| {
            ui.add_space(7.0);
            deck_badge(ui, mixer, midi_map, actions, deck, palette);
            let playing = clips
                .active(deck)
                .and_then(|slot| clips.movie(ClipAddress { deck, slot }))
                .map(|movie| movie.display_name.clone());
            let text = playing.unwrap_or_else(|| match mixer.deck(deck).state {
                DeckState::Empty => "empty".to_owned(),
                DeckState::Live(_) => "live input".to_owned(),
                DeckState::Generator(_) => "generator".to_owned(),
                _ => String::new(),
            });
            ui.add(
                egui::Label::new(egui::RichText::new(text).small().color(palette.muted_text))
                    .truncate(),
            );
        });
        ui.horizontal(|ui| {
            ui.add_space(7.0);
            deck_toggles(ui, state, mixer, midi_map, actions, deck);
            deck_level(
                ui,
                mixer,
                midi_map,
                actions,
                deck,
                (LANE_HEADER_WIDTH - toggles_width - 24.0).max(24.0),
            );
        });
    } else {
        ui.horizontal(|ui| {
            ui.add_space(7.0);
            deck_badge(ui, mixer, midi_map, actions, deck, palette);
            deck_toggles(ui, state, mixer, midi_map, actions, deck);
            deck_level(
                ui,
                mixer,
                midi_map,
                actions,
                deck,
                (LANE_HEADER_WIDTH - toggles_width - 52.0).max(20.0),
            );
        });
    }
}

/// An Ableton-style track header: a title bar in the deck colour, then
/// bypass/solo and the level.
fn track_header(
    ui: &mut egui::Ui,
    state: &mut UiState,
    mixer: &mut FourDeckMixer,
    midi_map: &MidiMapUi,
    actions: &mut Vec<UiAction>,
    deck: DeckId,
    width: f32,
) {
    let palette = state.theme.palette();
    let color = palette.deck_color(deck);
    let selected = mixer.selected() == deck;
    let title = mappable(
        ui,
        midi_map,
        ControlTarget::DeckSelect(deck.index() as u8),
        actions,
        |ui| {
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(width, 18.0), egui::Sense::click());
            let fill = if selected {
                color
            } else {
                palette.surface_tint(color, if response.hovered() { 0.6 } else { 0.4 })
            };
            ui.painter().rect_filled(rect, 2.0, fill);
            let luminance = 0.299 * f32::from(fill.r())
                + 0.587 * f32::from(fill.g())
                + 0.114 * f32::from(fill.b());
            ui.painter().text(
                rect.left_center() + egui::vec2(6.0, 0.0),
                egui::Align2::LEFT_CENTER,
                format!("DECK {}", deck.label()),
                egui::FontId::proportional(12.0),
                if luminance > 140.0 {
                    egui::Color32::BLACK
                } else {
                    egui::Color32::WHITE
                },
            );
            response.on_hover_text("Select this deck's performance controls")
        },
    );
    if title.clicked() {
        mixer.select(deck);
    }
    ui.horizontal(|ui| {
        deck_toggles(ui, state, mixer, midi_map, actions, deck);
        let used = 3.0 * 26.0;
        deck_level(
            ui,
            mixer,
            midi_map,
            actions,
            deck,
            (width - used - 8.0).max(24.0),
        );
    });
}

/// A scene (column) launcher with its assignment menu.
#[allow(clippy::too_many_arguments)]
fn scene_cell(
    ui: &mut egui::Ui,
    state: &mut UiState,
    clips: &ClipBank,
    midi_map: &MidiMapUi,
    actions: &mut Vec<UiAction>,
    scene_index: usize,
    size: egui::Vec2,
    label: &str,
) {
    let palette = state.theme.palette();
    let scene = fixed_cell(ui, size, |ui| {
        mappable(
            ui,
            midi_map,
            ControlTarget::SceneLaunch(scene_index as u8),
            actions,
            |ui| {
                ui.add_sized(
                    size,
                    egui::Button::new(egui::RichText::new(label).small().color(palette.grid_text))
                        .fill(palette.control_tint(palette.secondary, 0.18)),
                )
            },
        )
    });
    let scene = scene.on_hover_text(format!(
        "Launch scene {} on the next quantized boundary · right-click to edit",
        scene_index + 1
    ));
    scene.context_menu(|ui| {
        if state.show_mode {
            ui.weak("Scene assignments are locked in Show Mode");
            return;
        }
        if ui.button("Capture selected clips from all decks").clicked() {
            for deck in DeckId::ALL {
                state.scenes[scene_index].clip_slots[deck.index()] =
                    Some(clips.selected(deck) as u8);
                state.scenes[scene_index].stop_decks[deck.index()] = false;
            }
            ui.close();
        }
        ui.separator();
        for deck in DeckId::ALL {
            let index = deck.index();
            let scene_choice = if state.scenes[scene_index].stop_decks[index] {
                "Stop".to_owned()
            } else if let Some(slot) = state.scenes[scene_index].clip_slots[index] {
                format!("Clip {}", slot + 1)
            } else {
                "No change".to_owned()
            };
            ui.menu_button(format!("Deck {} · {scene_choice}", deck.label()), |ui| {
                if ui
                    .selectable_label(
                        !state.scenes[scene_index].stop_decks[index]
                            && state.scenes[scene_index].clip_slots[index].is_none(),
                        "No change",
                    )
                    .clicked()
                {
                    state.scenes[scene_index].clip_slots[index] = None;
                    state.scenes[scene_index].stop_decks[index] = false;
                    ui.close();
                }
                if ui
                    .selectable_label(state.scenes[scene_index].stop_decks[index], "Stop deck")
                    .clicked()
                {
                    state.scenes[scene_index].clip_slots[index] = None;
                    state.scenes[scene_index].stop_decks[index] = true;
                    ui.close();
                }
                ui.separator();
                for slot in 0..CLIPS_PER_DECK {
                    if ui
                        .selectable_label(
                            !state.scenes[scene_index].stop_decks[index]
                                && state.scenes[scene_index].clip_slots[index] == Some(slot as u8),
                            format!("Clip {}", slot + 1),
                        )
                        .clicked()
                    {
                        state.scenes[scene_index].clip_slots[index] = Some(slot as u8);
                        state.scenes[scene_index].stop_decks[index] = false;
                        ui.close();
                    }
                }
            });
        }
    });
    if scene.clicked() {
        actions.push(UiAction::LaunchScene(scene_index));
    }
}

/// One clip slot: tile, play button, drag and drop, launch, editor and
/// context menu. Shared by both grid arrangements.
#[allow(clippy::too_many_arguments)]
fn clip_cell(
    ui: &mut egui::Ui,
    state: &mut UiState,
    mixer: &mut FourDeckMixer,
    clips: &mut ClipBank,
    context: &ClipGridContext<'_>,
    actions: &mut Vec<UiAction>,
    address: ClipAddress,
    size: egui::Vec2,
    style: TileStyle,
) {
    let palette = state.theme.palette();
    let midi_map = context.midi_map;
    let ClipAddress { deck, slot } = address;
    let selected = clips.selected(deck) == slot && mixer.selected() == deck;
    let active = clips.active(deck) == Some(slot);
    let queued = context.launches.queued(address);
    let slot_state = clips
        .slot(address)
        .cloned()
        .expect("valid clip-grid address");
    let first_frame_ready = state
        .preloaded_frame(address, clips.path(address))
        .is_some();
    let automation_playing = clips
        .active_automation(deck)
        .is_some_and(|(active_slot, _)| active_slot == slot);
    let (name, tile_state) = if let Some(movie) = &slot_state.movie {
        let name = movie
            .display_name
            .rsplit_once('.')
            .map_or(movie.display_name.as_str(), |(stem, _)| stem)
            .to_owned();
        let tile_state = if queued {
            TileState::Queued
        } else if active {
            TileState::Playing
        } else if first_frame_ready {
            TileState::Ready
        } else {
            TileState::Idle
        };
        (name, tile_state)
    } else if slot_state.error.is_some() {
        (
            format!("{}{} missing", deck.label(), slot + 1),
            TileState::Missing,
        )
    } else if let Some(path) = &slot_state.pending_path {
        (
            path.file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or("loading")
                .to_owned(),
            TileState::Loading,
        )
    } else if automation_playing {
        (
            format!("{}{} automation", deck.label(), slot + 1),
            TileState::Playing,
        )
    } else {
        (format!("{}{}", deck.label(), slot + 1), TileState::Empty)
    };
    let tile = Tile {
        thumbnail: state.thumbnail(address, clips.path(address)),
        thumbnail_failed: state
            .thumbnail_failure(address, clips.path(address))
            .is_some(),
        name,
        state: tile_state,
        selected,
        deck_color: palette.deck_color(deck),
        progress: (tile_state == TileState::Playing && slot_state.movie.is_some())
            .then(|| {
                let transport = context.transports[deck.index()];
                transport
                    .duration
                    .filter(|duration| *duration > 0.0)
                    .map(|duration| (transport.position / duration) as f32)
            })
            .flatten(),
        duration: slot_state
            .movie
            .as_ref()
            .and_then(|movie| movie.duration)
            .map(virtual_core::MediaTime::as_seconds),
    };
    let draw = |ui: &mut egui::Ui| match style {
        TileStyle::Card => tile_widget(ui, size, &tile, palette),
        TileStyle::Filmstrip => filmstrip_widget(ui, size, &tile, palette),
    };
    let draggable = !state.show_mode
        && !midi_map.active
        && (slot_state.movie.is_some()
            || slot_state.pending_path.is_some()
            || slot_state.error.is_some());
    let (response, play) = fixed_cell(ui, size, |ui| {
        let response = if midi_map.active {
            mappable(
                ui,
                midi_map,
                ControlTarget::ClipLaunch {
                    deck: deck.index() as u8,
                    slot: slot as u8,
                },
                actions,
                draw,
            )
        } else if draggable {
            // Drag moves the clip to another slot; a plain click still
            // selects and launches because the drag only starts past egui's
            // drag threshold.
            ui.dnd_drag_source(
                egui::Id::new(("clip-slot-drag", deck.index(), slot)),
                address,
                draw,
            )
            .inner
        } else {
            draw(ui)
        };
        let play_rect = egui::Rect::from_min_size(
            response.rect.left_top()
                + if style == TileStyle::Card {
                    egui::vec2(4.0, 6.0)
                } else {
                    egui::vec2(3.0, 3.0)
                },
            egui::vec2(16.0, 16.0),
        );
        let play = ui.put(
            play_rect,
            egui::Button::new(
                egui::RichText::new(if automation_playing && clips.movie(address).is_none() {
                    "■"
                } else {
                    "▶"
                })
                .small(),
            )
            .fill(
                palette
                    .control_tint(
                        if automation_playing {
                            palette.success
                        } else {
                            palette.secondary
                        },
                        0.72,
                    )
                    .gamma_multiply(0.85),
            ),
        );
        (response, play)
    });
    if play
        .on_hover_text(if automation_playing && clips.movie(address).is_none() {
            "Stop this automation clip"
        } else if clips.movie(address).is_some() {
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
        } else if matches!(
            mixer.deck(deck).state,
            DeckState::Ready(_) | DeckState::Live(_) | DeckState::Generator(_)
        ) {
            actions.push(UiAction::LaunchAutomation(address));
        } else {
            state.clip_editor = Some(address);
        }
    }
    drop_targets::register(ui, response.rect, drop_targets::DropTarget::Clip(address));
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
            4.0,
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
    let preview = state.thumbnail(address, clips.path(address)).cloned();
    let details = if let Some(movie) = clips.movie(address) {
        let mut details = format!(
            "{}\n{}×{} · {}",
            movie.display_name, movie.visible_extent[0], movie.visible_extent[1], movie.codec
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
        if let Some(error) = state.thumbnail_failure(address, clips.path(address)) {
            details.push_str(&format!("\nThumbnail unavailable: {error}"));
        } else if first_frame_ready {
            details.push_str("\nFirst frame ready for immediate launch");
        } else {
            details.push_str("\nFirst frame is still preloading");
        }
        if !state.show_mode {
            details.push_str("\nDrag onto another slot to move; occupied slots swap");
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
    };
    response
        .on_hover_ui(|ui| {
            if let Some(texture) = &preview {
                ui.add(egui::Image::new(texture).fit_to_exact_size(egui::vec2(256.0, 144.0)));
            }
            ui.label(details);
        })
        .context_menu(|ui| {
            if state.show_mode {
                ui.weak("Show Mode locks media management");
                return;
            }
            if clips.path(address).is_some() && ui.button("Relink media…").clicked() {
                actions.push(UiAction::BrowseRelink(address));
                ui.close();
            }
            if clips.path(address).is_some() && ui.button("Delete clip").clicked() {
                actions.push(UiAction::ClearSlot(address));
                ui.close();
            }
        });
}

fn draw_clip_editor(
    ctx: &egui::Context,
    state: &mut UiState,
    clips: &mut ClipBank,
    address: ClipAddress,
    playhead_phase: Option<f64>,
) {
    let movie = clips.movie(address).cloned();
    let title = format!(
        "Clip Editor · {}",
        movie.as_ref().map_or_else(
            || format!("{}{} automation", address.deck.label(), address.slot + 1),
            |movie| movie.display_name.clone()
        )
    );
    let media_duration = movie
        .as_ref()
        .and_then(|movie| movie.duration)
        .map(virtual_core::MediaTime::as_seconds);
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
            if let Some(movie) = movie.as_ref() {
                ui.weak(format!("{}×{} · {} · {}", movie.visible_extent[0], movie.visible_extent[1], movie.codec,
                    media_duration.map_or_else(|| "unknown duration".to_owned(), |seconds| format!("{seconds:.3} s source"))));
            } else {
                ui.weak("Automation slot · runs over the active video, camera, or generator on this deck");
            }
            ui.separator();
            if movie.is_some() {
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
            }
            ui.separator();
            ui.strong("Automation");
            draw_automation_editor(ui, state, clips, address, playhead_phase);
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

fn draw_automation_editor(
    ui: &mut egui::Ui,
    state: &mut UiState,
    clips: &mut ClipBank,
    address: ClipAddress,
    playhead_phase: Option<f64>,
) {
    use virtual_core::{AutomationKeyframe, ClipAutomationLane, ControlTarget, CurveType};

    let mut targets = vec![
        ("Master · Opacity".to_owned(), ControlTarget::MasterOpacity),
        ("Mixer · Crossfader".to_owned(), ControlTarget::Crossfader),
        (
            format!("Deck {} · Level", address.deck.label()),
            ControlTarget::DeckLevel(address.deck.index() as u8),
        ),
        (
            format!("Deck {} · Speed", address.deck.label()),
            ControlTarget::DeckSpeed(address.deck.index() as u8),
        ),
    ];
    for (parameter, info) in virtual_generate::GENERATOR_PARAMETERS.iter().enumerate() {
        targets.push((
            format!("Deck {} · Generator · {}", address.deck.label(), info.name),
            ControlTarget::GeneratorParameter {
                deck: address.deck.index() as u8,
                parameter: parameter as u8,
            },
        ));
    }
    for effect in 0..virtual_core::FIXED_DECK_EFFECT_PARAMETER_COUNT {
        targets.push((
            format!(
                "Deck {} · Video FX · {}",
                address.deck.label(),
                effect_parameter_short_name(effect)
            ),
            ControlTarget::EffectParameter {
                deck: address.deck.index() as u8,
                effect,
                parameter: 0,
            },
        ));
    }
    for parameter in &state.deck_packages[address.deck.index()].parameters {
        targets.push((
            format!(
                "Deck {} · {} · {}",
                address.deck.label(),
                state.deck_packages[address.deck.index()].package_id,
                parameter.id
            ),
            ControlTarget::DeckEffectParameter {
                deck: address.deck.index() as u8,
                parameter_key: virtual_core::effect_parameter_key(
                    &state.deck_packages[address.deck.index()].package_id,
                    &parameter.id,
                ),
            },
        ));
    }
    for (slot, effect) in state.master_effects.slots.iter().enumerate() {
        for parameter in &effect.parameters {
            targets.push((
                format!("Master · {} · {}", effect.package_id, parameter.id),
                ControlTarget::MasterEffectParameter {
                    slot: slot as u8,
                    parameter_key: virtual_core::effect_parameter_key(
                        &effect.package_id,
                        &parameter.id,
                    ),
                },
            ));
        }
    }
    targets.retain(|(_, target)| target.is_clip_automatable());

    let mut automation = clips.automation(address).cloned().unwrap_or_default();
    ui.horizontal(|ui| automation_switch(ui, state, address.deck));
    ui.horizontal(|ui| {
        ui.label("Loop");
        ui.add(
            egui::DragValue::new(&mut automation.loop_beats)
                .range(1.0..=256.0)
                .speed(1.0)
                .suffix(" beats"),
        );
        ui.weak("Internal, MIDI, or Link tempo");
        ui.weak(format!("0 — {:.0} beats", automation.loop_beats));
    });
    let choice_id = egui::Id::new((
        "clip-automation-target-index",
        address.deck.index(),
        address.slot,
    ));
    let mut selected_target_index = ui
        .ctx()
        .data(|data| data.get_temp::<usize>(choice_id))
        .unwrap_or(0)
        .min(targets.len().saturating_sub(1));
    egui::ComboBox::from_id_salt(("clip-automation-target", address.deck.index(), address.slot))
        .selected_text(
            targets
                .get(selected_target_index)
                .map_or("Choose parameter", |(label, _)| label.as_str()),
        )
        .show_ui(ui, |ui| {
            for (index, (label, _target)) in targets.iter().enumerate() {
                ui.selectable_value(&mut selected_target_index, index, label);
            }
        });
    ui.ctx()
        .data_mut(|data| data.insert_temp(choice_id, selected_target_index));
    let new_target = targets
        .get(selected_target_index)
        .map(|(_, target)| *target)
        .unwrap_or(ControlTarget::MasterOpacity);
    if let Some((label, _)) = targets.get(selected_target_index) {
        ui.horizontal(|ui| {
            ui.weak(label);
            if automation.lanes.len() < virtual_core::MAX_AUTOMATION_LANES
                && ui.button("+ Add lane").clicked()
            {
                let initial = match new_target {
                    ControlTarget::MasterOpacity => state.master_opacity,
                    ControlTarget::Crossfader => state.crossfader,
                    ControlTarget::DeckLevel(_) => 1.0,
                    ControlTarget::DeckSpeed(_) => 1.0,
                    ControlTarget::GeneratorParameter { .. }
                    | ControlTarget::EffectParameter { .. } => 0.0,
                    ControlTarget::DeckEffectParameter {
                        deck,
                        parameter_key,
                    } => state.deck_packages[deck as usize]
                        .parameters
                        .iter()
                        .find(|p| {
                            virtual_core::effect_parameter_key(
                                &state.deck_packages[deck as usize].package_id,
                                &p.id,
                            ) == parameter_key
                        })
                        .map_or(0.0, |p| p.value),
                    ControlTarget::MasterEffectParameter {
                        slot,
                        parameter_key,
                    } => state
                        .master_effects
                        .slots
                        .get(slot as usize)
                        .and_then(|fx| {
                            fx.parameters.iter().find(|p| {
                                virtual_core::effect_parameter_key(&fx.package_id, &p.id)
                                    == parameter_key
                            })
                        })
                        .map_or(0.0, |p| p.value),
                    _ => 0.0,
                };
                if automation
                    .lanes
                    .iter()
                    .all(|lane| lane.target != new_target)
                {
                    automation
                        .lanes
                        .push(ClipAutomationLane::flat(new_target, initial));
                    state.clip_editor_lane = automation.lanes.len() - 1;
                }
            }
        });
    }
    ui.horizontal_wrapped(|ui| {
        if ui.button("Select").clicked() {
            state.clip_editor_draw_mode = false;
            state.clip_editor_draw_last = None;
        }
        if ui.button("Draw").clicked() {
            state.clip_editor_draw_mode = true;
            state.clip_editor_draw_last = None;
        }
        egui::ComboBox::from_id_salt(("automation-snap", address.deck.index(), address.slot))
            .selected_text(format!("Snap · {} beat", state.clip_editor_snap_beats))
            .show_ui(ui, |ui| {
                for (label, beats) in [
                    ("1/4 beat", 0.25),
                    ("1/2 beat", 0.5),
                    ("1 beat", 1.0),
                    ("2 beats", 2.0),
                    ("4 beats", 4.0),
                ] {
                    ui.selectable_value(&mut state.clip_editor_snap_beats, beats, label);
                }
            });
        ui.checkbox(&mut state.clip_editor_square_steps, "Square steps");
        if ui
            .button("Fill grid steps")
            .on_hover_text("Replace the selected lane with one held automation point per snapped tempo-grid step.")
            .clicked()
            && let Some(lane) = automation.lanes.get_mut(state.clip_editor_lane)
        {
            fill_automation_steps(
                lane,
                automation.loop_beats,
                state.clip_editor_snap_beats,
            );
        }
        ui.weak(if state.clip_editor_draw_mode {
            "Drag to draw points on the tempo grid · right-click removes"
        } else {
            "Click point to select · drag to edit"
        });
    });
    let effective_snap =
        automation_effective_snap(automation.loop_beats, state.clip_editor_snap_beats);
    if effective_snap > state.clip_editor_snap_beats {
        ui.weak(format!(
            "Long loop: grid widened to {effective_snap:.2} beats to stay within the 128-point lane limit."
        ));
    }

    for lane_index in 0..automation.lanes.len() {
        let mut remove_lane = false;
        ui.horizontal(|ui| {
            let lane = &mut automation.lanes[lane_index];
            ui.checkbox(&mut lane.enabled, "")
                .on_hover_text("Lane on/off, saved with the clip");
            let overrides = &mut state.automation_overrides[address.deck.index()];
            if let Some(held) = overrides.iter().position(|target| *target == lane.target) {
                ui.colored_label(state.theme.palette().warning, "✋ manual")
                    .on_hover_text("You moved this parameter, so the lane is paused");
                if ui
                    .small_button("Resume")
                    .on_hover_text("Hand this parameter back to its lane")
                    .clicked()
                {
                    overrides.remove(held);
                }
            }
            let lane_label = targets
                .iter()
                .find(|(_, target)| *target == lane.target)
                .map_or_else(|| format!("{:?}", lane.target), |(label, _)| label.clone());
            if ui
                .selectable_label(state.clip_editor_lane == lane_index, lane_label)
                .clicked()
            {
                state.clip_editor_lane = lane_index;
            }
            let selected_index = state
                .clip_editor_keyframe
                .min(lane.keyframes.len().saturating_sub(1));
            if state.clip_editor_lane == lane_index
                && let Some(selected) = lane.keyframes.get_mut(selected_index)
            {
                ui.add(
                    egui::DragValue::new(&mut selected.value)
                        .range(-100.0..=100.0)
                        .speed(0.01)
                        .prefix("Value "),
                );
                egui::ComboBox::from_id_salt((
                    "clip-curve-type",
                    address.deck.index(),
                    address.slot,
                    lane_index,
                ))
                .selected_text(selected.interpolation.label())
                .show_ui(ui, |ui| {
                    for curve in CurveType::ALL {
                        ui.selectable_value(&mut selected.interpolation, curve, curve.label());
                    }
                });
            }
            if ui.small_button("−").clicked() {
                remove_lane = true;
            }
        });
        if remove_lane {
            automation.remove_lane(lane_index);
            continue;
        }
        let lane = &mut automation.lanes[lane_index];
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(ui.available_width().max(200.0), 92.0),
            egui::Sense::click_and_drag(),
        );
        ui.painter()
            .rect_filled(rect, 3.0, ui.visuals().extreme_bg_color);
        let grid_steps = (automation.loop_beats / effective_snap).ceil().max(1.0) as usize;
        let line_stride = grid_steps
            .div_ceil(virtual_core::MAX_AUTOMATION_KEYFRAMES - 1)
            .max(1);
        for division in (0..=grid_steps).step_by(line_stride) {
            let beat = (division as f64 * effective_snap).min(automation.loop_beats);
            let x = rect.left() + rect.width() * (beat / automation.loop_beats) as f32;
            ui.painter().line_segment(
                [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                egui::Stroke::new(
                    if beat % 4.0 < 0.000_001 { 1.0 } else { 0.5 },
                    ui.visuals().widgets.noninteractive.bg_stroke.color,
                ),
            );
        }
        for beat in 0..=4 {
            let y = rect.top() + rect.height() * beat as f32 / 4.0;
            ui.painter().line_segment(
                [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
                egui::Stroke::new(0.5, ui.visuals().widgets.noninteractive.bg_stroke.color),
            );
        }
        if let Some(phase) = playhead_phase.filter(|phase| phase.is_finite()) {
            let phase = phase as f32;
            let x = rect.left() + rect.width() * phase;
            ui.painter().line_segment(
                [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                egui::Stroke::new(2.0, state.theme.palette().warning),
            );
        }
        let (minimum, maximum) = lane
            .keyframes
            .iter()
            .fold((0.0f32, 1.0f32), |(min, max), key| {
                (min.min(key.value), max.max(key.value))
            });
        let range = (maximum - minimum).max(0.001);
        let point_position = |key: AutomationKeyframe| {
            egui::pos2(
                rect.left() + rect.width() * key.position as f32,
                rect.bottom() - rect.height() * ((key.value - minimum) / range),
            )
        };
        for keyframes in lane.keyframes.windows(2) {
            let start = keyframes[0];
            let end = keyframes[1];
            let start_point = point_position(start);
            let end_point = point_position(end);
            if start.interpolation == CurveType::Step {
                let corner = egui::pos2(end_point.x, start_point.y);
                ui.painter().line_segment(
                    [start_point, corner],
                    egui::Stroke::new(2.0, state.theme.palette().accent),
                );
                ui.painter().line_segment(
                    [corner, end_point],
                    egui::Stroke::new(2.0, state.theme.palette().accent),
                );
            } else {
                let mut previous = start_point;
                for sample in 1..=16 {
                    let t = start.position + (end.position - start.position) * sample as f64 / 16.0;
                    let current = point_position(AutomationKeyframe::new(
                        t,
                        lane.value_at(t).unwrap_or(start.value),
                    ));
                    ui.painter().line_segment(
                        [previous, current],
                        egui::Stroke::new(2.0, state.theme.palette().accent),
                    );
                    previous = current;
                }
            }
        }
        for (index, key) in lane.keyframes.iter().copied().enumerate() {
            ui.painter().circle_filled(
                point_position(key),
                if lane_index == state.clip_editor_lane && index == state.clip_editor_keyframe {
                    5.0
                } else {
                    3.5
                },
                if lane_index == state.clip_editor_lane && index == state.clip_editor_keyframe {
                    state.theme.palette().warning
                } else {
                    state.theme.palette().accent
                },
            );
        }
        let position_from_pointer =
            |pos: egui::Pos2| ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0) as f64;
        let value_from_pointer = |pos: egui::Pos2| {
            minimum + range * (1.0 - (pos.y - rect.top()) / rect.height()).clamp(0.0, 1.0)
        };
        if let Some(pointer) = response.interact_pointer_pos() {
            if response.secondary_clicked() {
                state.clip_editor_draw_last = None;
                if let Some((index, _)) =
                    lane.keyframes.iter().enumerate().min_by(|(_, a), (_, b)| {
                        point_position(**a)
                            .distance(pointer)
                            .total_cmp(&point_position(**b).distance(pointer))
                    })
                {
                    lane.remove_keyframe(index);
                }
            } else if state.clip_editor_draw_mode {
                let current = [
                    position_from_pointer(pointer) as f32,
                    value_from_pointer(pointer),
                ];
                if response.drag_started() {
                    draw_automation_stroke(
                        lane,
                        current,
                        current,
                        automation.loop_beats,
                        effective_snap,
                        state.clip_editor_square_steps,
                    );
                    state.clip_editor_draw_last = Some(current);
                } else if response.dragged() || response.drag_stopped() {
                    let previous = state.clip_editor_draw_last.unwrap_or(current);
                    draw_automation_stroke(
                        lane,
                        previous,
                        current,
                        automation.loop_beats,
                        effective_snap,
                        state.clip_editor_square_steps,
                    );
                    state.clip_editor_draw_last = Some(current);
                } else if response.clicked() {
                    draw_automation_stroke(
                        lane,
                        current,
                        current,
                        automation.loop_beats,
                        effective_snap,
                        state.clip_editor_square_steps,
                    );
                }
                if response.drag_stopped() {
                    state.clip_editor_draw_last = None;
                }
            } else if response.clicked() || response.dragged() {
                let nearest = lane
                    .keyframes
                    .iter()
                    .enumerate()
                    .min_by(|(_, a), (_, b)| {
                        point_position(**a)
                            .distance(pointer)
                            .total_cmp(&point_position(**b).distance(pointer))
                    })
                    .map(|(index, _)| index);
                if let Some(index) = nearest {
                    state.clip_editor_lane = lane_index;
                    state.clip_editor_keyframe = index;
                    if response.dragged() {
                        let moved = AutomationKeyframe {
                            position: position_from_pointer(pointer),
                            value: value_from_pointer(pointer),
                            ..lane.keyframes[index]
                        };
                        lane.remove_keyframe(index);
                        lane.set_keyframe(moved);
                        state.clip_editor_keyframe = lane
                            .keyframes
                            .partition_point(|keyframe| keyframe.position <= moved.position)
                            .saturating_sub(1);
                    }
                }
            }
        }
    }
    clips.set_automation(address, automation);
}

fn draw_automation_stroke(
    lane: &mut virtual_core::ClipAutomationLane,
    from: [f32; 2],
    to: [f32; 2],
    loop_beats: f64,
    snap_beats: f64,
    square_steps: bool,
) {
    use virtual_core::{AutomationKeyframe, CurveType};

    if !loop_beats.is_finite() || loop_beats <= 0.0 || !snap_beats.is_finite() || snap_beats <= 0.0
    {
        return;
    }
    let snap_beats = automation_effective_snap(loop_beats, snap_beats);
    let snap = |position: f32| {
        let beat = position.clamp(0.0, 1.0) as f64 * loop_beats;
        if loop_beats - beat <= snap_beats * 0.5 {
            return 1.0;
        }
        ((beat / snap_beats).round() * snap_beats / loop_beats).clamp(0.0, 1.0)
    };
    let from_position = snap(from[0]);
    let to_position = snap(to[0]);
    let from_value = from[1];
    let to_value = to[1];
    let interpolation = if square_steps {
        CurveType::Step
    } else {
        CurveType::Linear
    };

    if square_steps {
        for keyframe in &mut lane.keyframes {
            keyframe.interpolation = CurveType::Step;
        }
    }

    if (to_position - from_position).abs() < f64::EPSILON {
        lane.set_keyframe(AutomationKeyframe {
            position: to_position,
            value: to_value,
            interpolation,
        });
        return;
    }

    let min_beat = from_position.min(to_position) * loop_beats;
    let max_beat = from_position.max(to_position) * loop_beats;
    let first_grid = (min_beat / snap_beats - 1e-9).ceil() as usize;
    let last_grid = (max_beat / snap_beats + 1e-9).floor() as usize;
    let max_grid = (loop_beats / snap_beats).ceil() as usize;
    for grid in first_grid..=last_grid.min(max_grid) {
        let position = (grid as f64 * snap_beats / loop_beats).clamp(0.0, 1.0);
        let amount = ((position - from_position) / (to_position - from_position)) as f32;
        lane.set_keyframe(AutomationKeyframe {
            position,
            value: from_value + (to_value - from_value) * amount.clamp(0.0, 1.0),
            interpolation,
        });
    }
    if from_position == 1.0 {
        lane.set_keyframe(AutomationKeyframe {
            position: 1.0,
            value: from_value,
            interpolation,
        });
    }
    if to_position == 1.0 {
        lane.set_keyframe(AutomationKeyframe {
            position: 1.0,
            value: to_value,
            interpolation,
        });
    }
}

fn fill_automation_steps(
    lane: &mut virtual_core::ClipAutomationLane,
    loop_beats: f64,
    snap_beats: f64,
) {
    use virtual_core::{AutomationKeyframe, CurveType, MAX_AUTOMATION_KEYFRAMES};

    if !loop_beats.is_finite() || loop_beats <= 0.0 || !snap_beats.is_finite() || snap_beats <= 0.0
    {
        return;
    }
    let snap_beats = automation_effective_snap(loop_beats, snap_beats);
    let source = lane.clone();
    let step_count = (loop_beats / snap_beats)
        .ceil()
        .clamp(1.0, (MAX_AUTOMATION_KEYFRAMES - 1) as f64) as usize;
    lane.keyframes.clear();
    for step in 0..step_count {
        let position = (step as f64 * snap_beats / loop_beats).min(1.0);
        lane.keyframes.push(AutomationKeyframe {
            position,
            value: source.value_at(position).unwrap_or(0.0),
            interpolation: CurveType::Step,
        });
    }
    lane.keyframes.push(AutomationKeyframe {
        position: 1.0,
        value: source.value_at(1.0).unwrap_or(0.0),
        interpolation: CurveType::Step,
    });
}

fn automation_effective_snap(loop_beats: f64, snap_beats: f64) -> f64 {
    use virtual_core::MAX_AUTOMATION_KEYFRAMES;

    if !loop_beats.is_finite() || loop_beats <= 0.0 || !snap_beats.is_finite() || snap_beats <= 0.0
    {
        return 1.0;
    }
    let steps = (loop_beats / snap_beats).ceil();
    let stride = (steps / (MAX_AUTOMATION_KEYFRAMES - 1) as f64)
        .ceil()
        .max(1.0);
    snap_beats * stride
}

fn effect_parameter_short_name(effect: u8) -> &'static str {
    [
        "Hue",
        "Contrast",
        "Saturation",
        "Black level",
        "White level",
        "Gamma",
        "Pixelate",
        "Luma key",
        "Neon",
        "Fractal",
        "Jitter",
        "Edges",
        "Bit reduction",
        "Blacklight",
        "Bloom",
        "Bloom threshold",
        "Bloom radius",
        "Bloom chroma",
        "Spiral fold",
        "Kali fold",
        "Koch fold",
        "Wave",
        "Vortex",
        "Block jitter",
        "RGB jitter",
        "Julia fold",
        "Polynomial fold",
    ]
    .get(effect as usize)
    .copied()
    .unwrap_or("Video FX")
}

/// Clip automation master switch for `deck`, plus a control that hands
/// parameters the performer grabbed back to their lanes.
fn automation_switch(ui: &mut egui::Ui, state: &mut UiState, deck: DeckId) {
    let index = deck.index();
    let palette = state.theme.palette();
    let on = state.automation_enabled[index];
    let clicked = buttons::mapped_target(ui, ControlTarget::DeckAutomation(index as u8), |ui| {
        ui.add(
            egui::Button::new(
                egui::RichText::new(if on {
                    "● AUTOMATION ON"
                } else {
                    "○ AUTOMATION OFF"
                })
                .strong(),
            )
            .selected(on)
            .fill(if on {
                palette.control_tint(palette.success, 0.35)
            } else {
                palette.control
            }),
        )
        .on_hover_text(
            "Clip automation for this deck. Off: every parameter is yours to tweak. \
             On: lanes run again, including parameters you grabbed. Moving a \
             parameter while automation runs takes over just that one.",
        )
    })
    .clicked();
    if clicked {
        state.automation_enabled[index] = !on;
    }
    let held = state.automation_overrides[index].len();
    if on
        && held > 0
        && buttons::midi_button(
            ui,
            &format!("deck.{index}.automation.resume"),
            &format!("Deck {} · Resume grabbed automation", deck.label()),
            |ui| {
                ui.button(format!("↺ Resume {held} manual"))
                    .on_hover_text("Hand the parameters you moved back to their automation lanes")
            },
        )
        .clicked()
    {
        state.automation_overrides[index].clear();
    }
}

#[cfg(test)]
mod automation_draw_tests {
    use super::{draw_automation_stroke, fill_automation_steps};
    use virtual_core::{ClipAutomationLane, ControlTarget, CurveType};

    #[test]
    fn square_draw_stroke_fills_every_tempo_grid_position() {
        let mut lane = ClipAutomationLane::flat(ControlTarget::MasterOpacity, 0.0);
        draw_automation_stroke(&mut lane, [0.0, 0.2], [1.0, 0.8], 16.0, 1.0, true);

        assert_eq!(lane.keyframes.len(), 17);
        for (index, keyframe) in lane.keyframes.iter().enumerate() {
            assert!((keyframe.position - index as f64 / 16.0).abs() < 1e-9);
            assert_eq!(keyframe.interpolation, CurveType::Step);
        }
        assert!((lane.keyframes[0].value - 0.2).abs() < 1e-6);
        assert!((lane.keyframes[16].value - 0.8).abs() < 1e-6);
        let midpoint = lane.value_at(0.5 / 16.0).unwrap();
        assert!((midpoint - lane.keyframes[0].value).abs() < 1e-6);
    }

    #[test]
    fn fill_grid_steps_replaces_a_curve_with_snapped_points_and_endpoint() {
        let mut lane = ClipAutomationLane::flat(ControlTarget::MasterOpacity, 0.0);
        lane.keyframes[1].value = 1.0;
        fill_automation_steps(&mut lane, 8.0, 0.5);

        assert_eq!(lane.keyframes.len(), 17);
        for (index, keyframe) in lane.keyframes.iter().enumerate() {
            assert_eq!(keyframe.interpolation, CurveType::Step);
            if index < 16 {
                assert!((keyframe.position - index as f64 / 16.0).abs() < 1e-9);
            }
        }
        assert_eq!(lane.keyframes.last().unwrap().position, 1.0);
        assert!((lane.keyframes.last().unwrap().value - 1.0).abs() < 1e-6);
    }

    #[test]
    fn long_automation_loops_coarsen_the_grid_instead_of_dropping_the_tail() {
        let mut lane = ClipAutomationLane::flat(ControlTarget::MasterOpacity, 0.0);
        draw_automation_stroke(&mut lane, [0.0, 0.1], [1.0, 0.9], 256.0, 0.25, true);
        assert!(lane.keyframes.len() <= virtual_core::MAX_AUTOMATION_KEYFRAMES);
        assert_eq!(lane.keyframes.last().unwrap().position, 1.0);
        assert!((lane.keyframes.last().unwrap().value - 0.9).abs() < 1e-6);

        fill_automation_steps(&mut lane, 256.0, 0.25);
        assert!(lane.keyframes.len() <= virtual_core::MAX_AUTOMATION_KEYFRAMES);
        assert!(lane.keyframes.len() > 100);
        assert_eq!(lane.keyframes.last().unwrap().position, 1.0);
    }
}
