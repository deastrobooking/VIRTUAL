//! Planned-show workspace: bounded edit history and frame-based tracks.
use super::{UiAction, UiState};
use std::time::Instant;
use virtual_core::sequence::{
    MAX_SEQUENCE_CLIPS, MAX_SEQUENCE_MARKERS, SequenceClip, SequenceMarker, ShowSequence, timecode,
};
use virtual_media::{ClipAddress, ClipBank, DeckId, FourDeckMixer};

pub(crate) struct SessionEditor {
    pub frame: u64,
    pub playing: bool,
    pub armed: bool,
    pub seek_requested: bool,
    pub last_tick: Option<Instant>,
    pub fractional: f64,
    pub active: [Option<u64>; 4],
    pub saved_bypass: Option<[bool; 4]>,
    pub was_playing: bool,
    pub selected: Option<u64>,
    zoom: f32,
    snap: bool,
    marker_name: String,
    undo: Vec<ShowSequence>,
    redo: Vec<ShowSequence>,
    pub status: String,
    drag: Option<(u64, u64, f32)>,
}

impl Default for SessionEditor {
    fn default() -> Self {
        Self {
            frame: 0,
            playing: false,
            armed: false,
            seek_requested: false,
            last_tick: None,
            fractional: 0.0,
            active: [None; 4],
            saved_bypass: None,
            was_playing: false,
            selected: None,
            zoom: 70.0,
            snap: true,
            marker_name: "Cue".into(),
            undo: Vec::new(),
            redo: Vec::new(),
            status: String::new(),
            drag: None,
        }
    }
}

impl SessionEditor {
    fn commit(&mut self, sequence: &mut ShowSequence, next: ShowSequence) {
        if next == *sequence {
            return;
        }
        if let Err(error) = next.validate() {
            self.status = error.into();
            return;
        }
        if self.undo.len() == 32 {
            self.undo.remove(0);
        }
        self.undo.push(sequence.clone());
        self.redo.clear();
        *sequence = next;
        self.seek_requested = true;
        self.status.clear();
    }
}

pub(super) fn draw(
    ui: &mut egui::Ui,
    state: &mut UiState,
    mixer: &FourDeckMixer,
    bank: &ClipBank,
    actions: &mut Vec<UiAction>,
) {
    let palette = state.theme.palette();
    let seq = &mut state.sequence;
    let ed = &mut state.session_editor;
    ui.horizontal_wrapped(|ui| {
        ui.heading("SESSION");
        ui.add(egui::TextEdit::singleline(&mut seq.name).char_limit(128));
        ui.monospace(timecode(ed.frame, seq.fps));
        if ui
            .button(if ed.playing { "Pause" } else { "Play" })
            .clicked()
        {
            if ed.frame >= seq.duration() {
                ed.frame = 0;
                ed.seek_requested = true;
            }
            ed.playing = !ed.playing;
            ed.last_tick = None;
        }
        if ui.button("Stop / Home").clicked() {
            ed.playing = false;
            ed.frame = 0;
            ed.seek_requested = true;
        }
        if ui.checkbox(&mut ed.armed, "Drive live Program").changed() {
            ed.seek_requested = true;
        }
        if ui.button("Save project").clicked() {
            actions.push(UiAction::SaveProject);
        }
        if ui.button("Export timeline…").clicked() {
            actions.push(UiAction::ExportSequence);
        }
    });
    ui.horizontal_wrapped(|ui| {
        ui.weak(format!(
            "{} fps · {} · four video tracks",
            seq.fps,
            timecode(seq.duration(), seq.fps)
        ));
        // Changing rate reinterprets frames; avoid silently retiming an existing edit.
        ui.add_enabled_ui(seq.clips.is_empty() && !ed.playing, |ui| {
            egui::ComboBox::from_id_salt("sequence-fps")
                .selected_text(format!("{} fps", seq.fps))
                .show_ui(ui, |ui| {
                    for fps in [24, 25, 30, 50, 60] {
                        ui.selectable_value(&mut seq.fps, fps, fps.to_string());
                    }
                });
        });
        ui.checkbox(&mut ed.snap, "Snap to seconds");
        ui.add(egui::Slider::new(&mut ed.zoom, 12.0..=240.0).text("zoom px/s"));
        if ui
            .add_enabled(!ed.undo.is_empty() && !ed.armed, egui::Button::new("Undo"))
            .clicked()
        {
            ed.redo.push(seq.clone());
            *seq = ed.undo.pop().unwrap();
            ed.seek_requested = true;
        }
        if ui
            .add_enabled(!ed.redo.is_empty() && !ed.armed, egui::Button::new("Redo"))
            .clicked()
        {
            ed.undo.push(seq.clone());
            *seq = ed.redo.pop().unwrap();
            ed.seek_requested = true;
        }
    });
    if ed.armed {
        ui.colored_label(
            palette.warning,
            "LIVE · Timeline owns deck transport and gap bypass. Release to return to live mixing.",
        );
    } else {
        ui.weak("Planning transport only. Enable Drive live Program to audition the arrangement on the output.");
    }
    let mut next = seq.clone();
    ui.add_enabled_ui(!ed.armed && !ed.playing, |ui| {
        ui.horizontal_wrapped(|ui| {
            let deck = mixer.selected();
            let address = ClipAddress {
                deck,
                slot: bank.selected(deck),
            };
            if ui
                .add_enabled(
                    seq.clips.len() < MAX_SEQUENCE_CLIPS && bank.movie(address).is_some(),
                    egui::Button::new("Insert selected clip at playhead"),
                )
                .clicked()
                && let Some(movie) = bank.movie(address)
            {
                if movie.path.to_str().is_some_and(|p| p.contains("://")) {
                    ed.status = "Use a file-backed clip for a planned sequence".into();
                } else {
                    let playback = bank.playback(address).unwrap_or_default();
                    let (source_in, source_out) =
                        playback.range(movie.duration.map(|d| d.as_seconds()), state.bpm);
                    let duration = ((source_out.unwrap_or(source_in + 10.0) - source_in)
                        * f64::from(seq.fps))
                    .round()
                    .max(1.0) as u64;
                    next.clips.push(SequenceClip {
                        id: seq.next_id(),
                        track: deck.index() as u8,
                        slot: address.slot as u8,
                        name: movie.display_name.chars().take(128).collect(),
                        start: ed.frame,
                        duration,
                        source_in: (source_in * f64::from(seq.fps)).round() as u64,
                    });
                }
            }
            if ui
                .add_enabled(
                    ed.selected.is_some(),
                    egui::Button::new("Split at playhead"),
                )
                .clicked()
            {
                next.split(ed.selected.unwrap(), ed.frame);
            }
            if ui
                .add_enabled(ed.selected.is_some(), egui::Button::new("Delete clip"))
                .clicked()
            {
                next.clips.retain(|c| Some(c.id) != ed.selected);
            }
            ui.text_edit_singleline(&mut ed.marker_name);
            if ui
                .add_enabled(
                    seq.markers.len() < MAX_SEQUENCE_MARKERS,
                    egui::Button::new("Add marker"),
                )
                .clicked()
            {
                next.markers.push(SequenceMarker {
                    frame: ed.frame,
                    name: ed.marker_name.chars().take(128).collect(),
                });
            }
        });
    });
    ed.commit(seq, next);
    let header = 110.0;
    let seconds = (seq.duration() as f32 / seq.fps as f32 + 10.0).max(60.0);
    let width = (seconds * ed.zoom).max(ui.available_width() - header);
    let row_height = 62.0;
    let height = 34.0 + 4.0 * row_height;
    let mut proposed = None;
    egui::ScrollArea::horizontal()
        .id_salt("sequence-tracks")
        .show(ui, |ui| {
            let (rect, response) = ui.allocate_exact_size(
                egui::vec2(header + width, height),
                egui::Sense::click_and_drag(),
            );
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 0.0, palette.background);
            let origin = rect.left() + header;
            let step = (100.0 / ed.zoom).ceil().max(1.0) as u64;
            for second in (0..seconds.ceil() as u64).step_by(step as usize) {
                let x = origin + second as f32 * ed.zoom;
                painter.line_segment(
                    [
                        egui::pos2(x, rect.top() + 24.0),
                        egui::pos2(x, rect.bottom()),
                    ],
                    egui::Stroke::new(1.0, palette.stroke),
                );
                painter.text(
                    egui::pos2(x + 3.0, rect.top() + 3.0),
                    egui::Align2::LEFT_TOP,
                    timecode(second * u64::from(seq.fps), seq.fps),
                    egui::FontId::monospace(10.0),
                    palette.text,
                );
            }
            for track in 0..4_u8 {
                let top = rect.top() + 34.0 + f32::from(3 - track) * row_height;
                let color = palette.deck_color(DeckId::ALL[usize::from(track)]);
                painter.text(
                    egui::pos2(rect.left() + 8.0, top + 12.0),
                    egui::Align2::LEFT_TOP,
                    format!(
                        "V{} · Deck {}",
                        track + 1,
                        DeckId::ALL[usize::from(track)].label()
                    ),
                    egui::FontId::proportional(12.0),
                    color,
                );
                painter.line_segment(
                    [egui::pos2(rect.left(), top), egui::pos2(rect.right(), top)],
                    egui::Stroke::new(1.0, palette.stroke),
                );
            }
            for clip in &seq.clips {
                let top = rect.top() + 38.0 + f32::from(3 - clip.track) * row_height;
                let x = origin + clip.start as f32 / seq.fps as f32 * ed.zoom;
                let w = (clip.duration as f32 / seq.fps as f32 * ed.zoom).max(3.0);
                let tile =
                    egui::Rect::from_min_size(egui::pos2(x, top), egui::vec2(w, row_height - 8.0));
                let color = palette.deck_color(DeckId::ALL[usize::from(clip.track)]);
                painter.rect_filled(tile, 3.0, color.gamma_multiply(0.3));
                painter.rect_stroke(
                    tile,
                    3.0,
                    egui::Stroke::new(
                        if ed.selected == Some(clip.id) {
                            2.0
                        } else {
                            1.0
                        },
                        color,
                    ),
                    egui::StrokeKind::Inside,
                );
                let label = format!("{} · slot {}", clip.name, clip.slot + 1);
                painter.with_clip_rect(tile.intersect(rect)).text(
                    tile.min + egui::vec2(5.0, 6.0),
                    egui::Align2::LEFT_TOP,
                    label,
                    egui::FontId::proportional(12.0),
                    palette.text,
                );
                let hit = ui.interact(
                    tile,
                    ui.id().with(("sequence-clip", clip.id)),
                    egui::Sense::click_and_drag(),
                );
                if hit.clicked() {
                    ed.selected = Some(clip.id);
                }
                if hit.drag_started()
                    && let Some(pos) = hit.interact_pointer_pos()
                {
                    ed.drag = Some((clip.id, clip.start, pos.x));
                }
                if hit.drag_stopped()
                    && !ed.armed
                    && !ed.playing
                    && let Some((id, start, origin_x)) = ed.drag.take()
                    && id == clip.id
                    && let Some(pos) = hit.interact_pointer_pos()
                {
                    ed.selected = Some(clip.id);
                    let delta = ((pos.x - origin_x) / ed.zoom * seq.fps as f32).round() as i64;
                    let frame = start.saturating_add_signed(delta);
                    let frame = if ed.snap {
                        (frame + u64::from(seq.fps) / 2) / u64::from(seq.fps) * u64::from(seq.fps)
                    } else {
                        frame
                    };
                    proposed = Some((clip.id, frame));
                }
            }
            for marker in &seq.markers {
                let x = origin + marker.frame as f32 / seq.fps as f32 * ed.zoom;
                painter.text(
                    egui::pos2(x, rect.top() + 20.0),
                    egui::Align2::LEFT_TOP,
                    format!("◆ {}", marker.name),
                    egui::FontId::proportional(10.0),
                    palette.accent,
                );
            }
            let x = origin + ed.frame as f32 / seq.fps as f32 * ed.zoom;
            painter.line_segment(
                [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                egui::Stroke::new(2.0, palette.accent),
            );
            if (response.clicked() || response.dragged())
                && let Some(pos) = response.interact_pointer_pos()
                && pos.y < rect.top() + 34.0
            {
                ed.frame = (((pos.x - origin).max(0.0) / ed.zoom) * seq.fps as f32).round() as u64;
                ed.frame = ed.frame.min(u64::from(seq.fps) * 21_600);
                ed.seek_requested = true;
            }
        });
    if let Some((id, start)) = proposed {
        let mut next = seq.clone();
        if let Some(c) = next.clips.iter_mut().find(|c| c.id == id) {
            c.start = start;
        }
        ed.commit(seq, next);
    }
    let mut next = seq.clone();
    ui.add_enabled_ui(!ed.armed && !ed.playing, |ui| {
        if let Some(c) = next.clips.iter_mut().find(|c| Some(c.id) == ed.selected) {
            ui.group(|ui| {
                ui.strong("CLIP INSPECTOR · frames");
                ui.horizontal_wrapped(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut c.name).char_limit(128));
                    ui.label("Start");
                    ui.add(
                        egui::DragValue::new(&mut c.start).range(0..=u64::from(seq.fps) * 21_600),
                    );
                    ui.label("Duration");
                    ui.add(
                        egui::DragValue::new(&mut c.duration)
                            .range(1..=u64::from(seq.fps) * 21_600),
                    );
                    ui.label("Source In");
                    ui.add(
                        egui::DragValue::new(&mut c.source_in)
                            .range(0..=u64::from(seq.fps) * 21_600),
                    );
                });
            });
        }
    });
    ed.commit(seq, next);
    if !ed.status.is_empty() {
        ui.colored_label(palette.warning, &ed.status);
    }
    ui.weak("Timeline export contains source edits and gaps. Rendered movies, audio tracks and effect automation are separate upcoming stages.");
}

impl UiState {
    pub(crate) fn session_key(
        &mut self,
        code: winit::keyboard::KeyCode,
        modifiers: winit::keyboard::ModifiersState,
    ) -> bool {
        use winit::keyboard::KeyCode;
        if self.workspace != super::Workspace::Session || self.show_mode {
            return false;
        }
        let ed = &mut self.session_editor;
        match code {
            KeyCode::Space => {
                if ed.frame >= self.sequence.duration() {
                    ed.frame = 0;
                    ed.seek_requested = true;
                }
                ed.playing = !ed.playing;
                ed.last_tick = None;
            }
            KeyCode::Home => {
                ed.frame = 0;
                ed.seek_requested = true;
            }
            KeyCode::ArrowLeft => {
                ed.frame = ed.frame.saturating_sub(1);
                ed.seek_requested = true;
            }
            KeyCode::ArrowRight => {
                ed.frame = (ed.frame + 1).min(u64::from(self.sequence.fps) * 21_600);
                ed.seek_requested = true;
            }
            KeyCode::Delete | KeyCode::Backspace => {
                if !ed.armed && !ed.playing {
                    let mut next = self.sequence.clone();
                    next.clips.retain(|c| Some(c.id) != ed.selected);
                    ed.commit(&mut self.sequence, next);
                }
            }
            KeyCode::KeyZ
                if (modifiers.control_key() || modifiers.super_key())
                    && !ed.armed
                    && !ed.playing =>
            {
                if modifiers.shift_key() {
                    if let Some(next) = ed.redo.pop() {
                        ed.undo.push(self.sequence.clone());
                        self.sequence = next;
                    }
                } else if let Some(previous) = ed.undo.pop() {
                    ed.redo.push(self.sequence.clone());
                    self.sequence = previous;
                }
                ed.seek_requested = true;
            }
            _ => return false,
        }
        true
    }
}

pub(super) fn draw_bin(ui: &mut egui::Ui, mixer: &mut FourDeckMixer, bank: &mut ClipBank) {
    egui::CollapsingHeader::new("PROJECT MEDIA · selection never launches output")
        .default_open(true)
        .show(ui, |ui| {
            egui::ScrollArea::horizontal()
                .id_salt("session-source-bin")
                .show(ui, |ui| {
                    egui::Grid::new("session-bin-grid").show(ui, |ui| {
                        for deck in DeckId::ALL {
                            ui.strong(format!("V{}", deck.index() + 1));
                            for slot in 0..8 {
                                let address = ClipAddress { deck, slot };
                                let name = bank
                                    .movie(address)
                                    .map_or("Empty", |m| m.display_name.as_str());
                                let selected =
                                    mixer.selected() == deck && bank.selected(deck) == slot;
                                let button = egui::Button::new(format!(
                                    "{}{} · {}",
                                    deck.label(),
                                    slot + 1,
                                    name.chars().take(14).collect::<String>()
                                ))
                                .selected(selected);
                                if ui.add_sized([120.0, 32.0], button).clicked() {
                                    mixer.select(deck);
                                    bank.select(address);
                                }
                            }
                            ui.end_row();
                        }
                    });
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::keyboard::{KeyCode, ModifiersState};

    fn item(id: u64, start: u64) -> SequenceClip {
        SequenceClip {
            id,
            track: 0,
            slot: 0,
            name: "Shot".into(),
            start,
            duration: 30,
            source_in: 0,
        }
    }

    #[test]
    fn invalid_move_does_not_destroy_arrangement_or_history() {
        let mut sequence = ShowSequence {
            clips: vec![item(1, 0), item(2, 30)],
            ..ShowSequence::default()
        };
        let original = sequence.clone();
        let mut editor = SessionEditor::default();
        let mut overlapping = sequence.clone();
        overlapping.clips[1].start = 15;
        editor.commit(&mut sequence, overlapping);
        assert_eq!(sequence, original);
        assert!(editor.undo.is_empty());
        assert!(!editor.status.is_empty());
    }

    #[test]
    fn session_delete_undo_and_transport_are_distinct_from_performance_controls() {
        let mut state = UiState {
            workspace: super::super::Workspace::Session,
            ..UiState::default()
        };
        state.sequence.clips.push(item(1, 0));
        state.session_editor.selected = Some(1);
        assert!(state.session_key(KeyCode::Delete, ModifiersState::empty()));
        assert!(state.sequence.clips.is_empty());
        assert!(state.session_key(KeyCode::KeyZ, ModifiersState::SUPER));
        assert_eq!(state.sequence.clips.len(), 1);
        assert!(state.session_key(KeyCode::Space, ModifiersState::empty()));
        assert!(state.session_editor.playing);
        assert!(!state.master_freeze);
        state.show_mode = true;
        assert!(!state.session_key(KeyCode::Delete, ModifiersState::empty()));
        assert_eq!(state.sequence.clips.len(), 1);
    }
}
