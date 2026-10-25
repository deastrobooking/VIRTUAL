//! Compact non-destructive video and performance-take assembly.
use super::{PerformanceMetrics, UiAction, UiState};
use virtual_io::{EditSegmentProject, MAX_TIMELINE_SEGMENTS, VideoTimelineProject};

pub struct TimelineUi {
    pub open: bool,
    pub project: VideoTimelineProject,
    pub job: Option<crate::timeline_export::ExportJob>,
    pub status: String,
    takes: bool,
    selected: usize,
    cursor: f64,
    zoom: f32,
    clipboard: Option<(bool, EditSegmentProject)>,
    undo: Vec<VideoTimelineProject>,
    redo: Vec<VideoTimelineProject>,
}
impl Default for TimelineUi {
    fn default() -> Self {
        Self {
            open: false,
            project: Default::default(),
            job: None,
            status: String::new(),
            takes: false,
            selected: 0,
            cursor: 0.0,
            zoom: 16.0,
            clipboard: None,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }
}
impl TimelineUi {
    pub fn reset_history(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.clipboard = None;
        self.selected = 0;
        self.cursor = 0.0;
    }
    fn remember(&mut self, previous: VideoTimelineProject) {
        if previous == self.project {
            return;
        }
        if self.undo.len() == 64 {
            self.undo.remove(0);
        }
        self.undo.push(previous);
        self.redo.clear();
    }
    pub fn add_video(&mut self, segment: EditSegmentProject) {
        if !segment.valid() || self.project.videos.len() >= MAX_TIMELINE_SEGMENTS {
            self.status =
                "Choose a video with a finite duration; timeline limit is 128 cuts".into();
            return;
        }
        let previous = self.project.clone();
        self.project.videos.push(segment);
        self.selected = self.project.videos.len() - 1;
        self.takes = false;
        self.open = true;
        self.remember(previous);
    }
}

pub(super) fn draw(
    ctx: &egui::Context,
    state: &mut UiState,
    metrics: &PerformanceMetrics<'_>,
    actions: &mut Vec<UiAction>,
) {
    let timeline = &mut state.timeline;
    if let Some(job) = &timeline.job
        && let Some(result) = job.poll()
    {
        timeline.status = result.unwrap_or_else(|error| format!("Export failed: {error}"));
        timeline.job = None;
    }
    if !timeline.open {
        return;
    }
    let mut open = true;
    egui::Window::new("Timeline · Video & Takes").id(egui::Id::new("video-timeline"))
        .open(&mut open).default_size([820.0, 420.0]).min_width(480.0).vscroll(true)
        .show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.selectable_value(&mut timeline.takes, false, "▤ Video cuts").changed() { timeline.selected = 0; }
                if ui.selectable_value(&mut timeline.takes, true, "◷ Performance takes").changed() { timeline.selected = 0; }
                ui.separator();
                ui.add_enabled_ui(!state.show_mode, |ui| {
                    if ui.add_enabled(!timeline.undo.is_empty(), egui::Button::new("↶ Undo")).clicked()
                        && let Some(previous) = timeline.undo.pop() {
                        timeline.redo.push(std::mem::replace(&mut timeline.project, previous));
                    }
                    if ui.add_enabled(!timeline.redo.is_empty(), egui::Button::new("↷ Redo")).clicked()
                        && let Some(next) = timeline.redo.pop() {
                        timeline.undo.push(std::mem::replace(&mut timeline.project, next));
                    }
                });
                ui.add(egui::Slider::new(&mut timeline.zoom, 4.0..=80.0).text("Zoom").show_value(false));
            });
            let previous = timeline.project.clone();
            let takes = timeline.takes;
            if takes {
                ui.horizontal_wrapped(|ui| {
                    if ui.add_enabled(!state.show_mode, egui::Button::new("Scan recorded takes")).clicked() {
                        actions.push(UiAction::RefreshSessionRecoveries);
                    }
                    ui.menu_button("+ Add take segment", |ui| {
                        if metrics.session_recoveries.is_empty() { ui.weak("Scan recorded takes first"); }
                        for entry in metrics.session_recoveries {
                            if ui.add_enabled(!state.show_mode && timeline.project.takes.len() < MAX_TIMELINE_SEGMENTS,
                                egui::Button::new(&entry.take_name)).clicked() {
                                let duration = entry.latest_time.monotonic_ns as f64 / 1e9;
                                let start = entry.start_seconds();
                                let segment = EditSegmentProject { path: entry.journal_path.clone(), name: entry.take_name.clone(),
                                    in_seconds: start, out_seconds: duration, source_start: start, source_duration: duration };
                                if segment.valid() {
                                    timeline.project.takes.push(segment);
                                    timeline.selected = timeline.project.takes.len() - 1;
                                } else { timeline.status = "This take has no editable duration yet".into(); }
                                ui.close();
                            }
                        }
                    });
                });
                ui.weak("Trim and join recorded control performances. Export creates a new recoverable take.");
            } else {
                ui.weak("Append clips from any grid using + Timeline or the clip context menu. Save the project to keep edits.");
                ui.weak("Video export: source cuts, 30 fps, silent H.264. Mixer effects are stored separately in performance takes.");
            }
            let track = if takes { &mut timeline.project.takes } else { &mut timeline.project.videos };
            if !state.show_mode && !ctx.text_edit_focused() && ctx.top_layer_id() == Some(ui.layer_id()) {
                let (copy, paste) = ui.input(|input| (
                    input.events.iter().any(|event| matches!(event, egui::Event::Copy))
                        || (input.modifiers.command && input.key_pressed(egui::Key::C)),
                    input.events.iter().any(|event| matches!(event, egui::Event::Paste(_)))
                        || (input.modifiers.command && input.key_pressed(egui::Key::V)),
                ));
                if copy && let Some(segment) = track.get(timeline.selected) {
                    timeline.clipboard = Some((takes, segment.clone()));
                }
                if paste && track.len() < MAX_TIMELINE_SEGMENTS
                    && let Some((source_takes, segment)) = &timeline.clipboard && *source_takes == takes {
                    track.push(segment.clone()); timeline.selected = track.len() - 1;
                }
            }
            let total: f64 = track.iter().map(EditSegmentProject::duration).sum();
            timeline.selected = timeline.selected.min(track.len().saturating_sub(1));
            timeline.cursor = timeline.cursor.clamp(0.0, total.max(0.0));
            ui.horizontal(|ui| {
                ui.strong(format!("{} cuts · {total:.2} s", track.len()));
                ui.add(egui::Slider::new(&mut timeline.cursor, 0.0..=total.max(0.001)).text("Cursor").suffix(" s"));
            });
            let mut reorder = None;
            let mut offset = 0.0;
            egui::ScrollArea::horizontal().id_salt("edit-timeline-track").auto_shrink([false, true]).show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (index, segment) in track.iter().enumerate() {
                        let width = (segment.duration() as f32 * timeline.zoom).clamp(48.0, 20_000.0);
                        let text = format!("{}  {}\n{}\n{:.2}–{:.2} s", index + 1,
                            if takes { "TAKE" } else { "VIDEO" }, segment.name, offset, offset + segment.duration());
                        let draw = |ui: &mut egui::Ui| ui.add_sized([width, 72.0], egui::Button::new(&text).selected(timeline.selected == index));
                        let response = if state.show_mode { draw(ui) } else {
                            ui.dnd_drag_source(egui::Id::new(("edit-segment", takes, index)), (takes, index), draw).inner
                        };
                        if response.clicked() { timeline.selected = index; timeline.cursor = offset; }
                        if let Some(source) = response.dnd_release_payload::<(bool, usize)>()
                            && source.0 == takes && source.1 != index && !state.show_mode { reorder = Some((source.1, index)); }
                        offset += segment.duration();
                    }
                });
            });
            if let Some((from, to)) = reorder && from < track.len() && to < track.len() {
                let segment = track.remove(from); track.insert(to, segment); timeline.selected = to;
            }
            let selected = timeline.selected;
            let start: f64 = track.iter().take(selected).map(EditSegmentProject::duration).sum();
            let mut remove = false;
            let mut duplicate = false;
            let mut split = false;
            let mut shift = 0isize;
            if let Some(segment) = track.get_mut(selected) {
                ui.separator();
                ui.label(egui::RichText::new(&segment.name).strong());
                ui.add(egui::Label::new(segment.path.display().to_string()).truncate());
                ui.add_enabled_ui(!state.show_mode, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        let max_in = (segment.out_seconds - 0.001).max(0.0);
                        ui.add(egui::DragValue::new(&mut segment.in_seconds).range(segment.source_start..=max_in).speed(0.05).prefix("In ").suffix(" s"));
                        ui.add(egui::DragValue::new(&mut segment.out_seconds).range(segment.in_seconds + 0.001..=segment.source_duration).speed(0.05).prefix("Out ").suffix(" s"));
                        ui.weak(format!("{:.2} s", segment.duration()));
                    });
                    ui.horizontal_wrapped(|ui| {
                        if ui.small_button("Copy").clicked() { timeline.clipboard = Some((takes, segment.clone())); }
                        duplicate = ui.small_button("Duplicate").clicked();
                        split = ui.small_button("Split at cursor").clicked();
                        if ui.small_button("←").on_hover_text("Move earlier").clicked() { shift = -1; }
                        if ui.small_button("→").on_hover_text("Move later").clicked() { shift = 1; }
                        remove = ui.small_button("Remove").clicked();
                    });
                });
                if takes && ui.add_enabled(!state.show_mode, egui::Button::new("Restore take at cursor as branch")).clicked() {
                    if let Some(index) = metrics.session_recoveries.iter().position(|entry| entry.journal_path == segment.path) {
                        let local = (timeline.cursor - start).clamp(0.0, segment.duration());
                        actions.push(UiAction::RestoreSessionRecoveryAt { index, monotonic_ns: ((segment.in_seconds + local) * 1e9).round() as u64 });
                    } else { timeline.status = "Scan recorded takes to restore this segment".into(); }
                }
            }
            if !state.show_mode {
                if remove { track.remove(selected); }
                else if duplicate && track.len() < MAX_TIMELINE_SEGMENTS {
                    track.insert(selected + 1, track[selected].clone()); timeline.selected += 1;
                } else if split && track.len() < MAX_TIMELINE_SEGMENTS {
                    split_segment(track, selected, timeline.cursor - start);
                } else if shift != 0 {
                    let to = selected as isize + shift;
                    if to >= 0 && (to as usize) < track.len() { track.swap(selected, to as usize); timeline.selected = to as usize; }
                }
                if ui.add_enabled(timeline.clipboard.is_some() && track.len() < MAX_TIMELINE_SEGMENTS,
                    egui::Button::new("Paste segment at end")).clicked() && let Some(segment) = &timeline.clipboard {
                    // A take journal cannot be pasted into a source-video track.
                    if segment.0 == takes {
                        track.push(segment.1.clone()); timeline.selected = track.len() - 1;
                    } else { timeline.status = "Choose the matching video or take track to paste".into(); }
                }
            }
            let empty = track.is_empty();
            timeline.remember(previous);
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(!state.show_mode && !empty && timeline.job.is_none(),
                    egui::Button::new(if takes { "Export assembled take…" } else { "Render video…" })).clicked() {
                    actions.push(if takes { UiAction::ExportTimelineTake } else { UiAction::ExportTimelineVideo });
                }
                if let Some(job) = &timeline.job {
                    ui.spinner(); ui.label("Exporting…");
                    if ui.button("Cancel export").clicked() { job.cancel(); }
                }
            });
            if !timeline.status.is_empty() { ui.label(&timeline.status); }
        });
    timeline.open = open;
}

fn split_segment(track: &mut Vec<EditSegmentProject>, index: usize, local: f64) -> bool {
    let Some(segment) = track.get_mut(index) else {
        return false;
    };
    if !local.is_finite() || local < 0.001 || local > segment.duration() - 0.001 {
        return false;
    }
    let mut right = segment.clone();
    right.in_seconds += local;
    segment.out_seconds = right.in_seconds;
    track.insert(index + 1, right);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn split_preserves_source_and_total_duration() {
        let mut cuts = vec![EditSegmentProject {
            path: "clip.mov".into(),
            name: "Clip".into(),
            in_seconds: 3.0,
            out_seconds: 11.0,
            source_start: 0.0,
            source_duration: 12.0,
        }];
        assert!(!split_segment(&mut cuts, 0, 0.0));
        assert!(split_segment(&mut cuts, 0, 2.5));
        assert_eq!(cuts[0].out_seconds, cuts[1].in_seconds);
        assert_eq!(
            cuts.iter().map(EditSegmentProject::duration).sum::<f64>(),
            8.0
        );
        assert!(cuts.iter().all(EditSegmentProject::valid));
    }
}
