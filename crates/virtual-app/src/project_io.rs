//! Project snapshot, save/open, restore polling and autosave.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use virtual_core::MediaTime;
use virtual_io::{
    ProjectFile, autosave_path, load_project, recovery_is_newer, resolve_media_paths,
    save_project_portable,
};
use virtual_media::{ClipAddress, ClipBank, ClipRestoreRequest, DeckId, LaunchQueue};
use virtual_session::{CommandOperation, CommandOrigin};

use super::project_save::{SaveCompletion, SaveKind, SaveRequest};
use super::{State, display_path, paths, project};

const DIRTY_CHECK_INTERVAL: Duration = Duration::from_millis(250);

pub(crate) struct PendingProjectOpen {
    path: PathBuf,
    recovered: bool,
    project: ProjectFile,
}

impl State {
    pub(crate) fn project_snapshot(&self) -> ProjectFile {
        let mut takes = self.project_takes.clone();
        if let Some(active) = self.performance_runtime.take_metadata() {
            if let Some(existing) = takes.iter_mut().find(|take| take.take_id == active.take_id) {
                *existing = active;
            } else {
                takes.push(active);
            }
        }
        if takes.len() > 256 {
            takes.sort_by_key(|take| take.created_unix_ms);
            let remove = takes.len() - 256;
            takes.drain(..remove);
        }
        let mut snapshot = project::snapshot(
            &self.ui,
            &self.mixer,
            &self.clips,
            &self.transports,
            &self.midi,
            &self.live_configs,
            project::ProjectSessionMetadata {
                project_id: &self.project_id,
                takes,
                graph: self.performance_runtime.active_graph().clone(),
                random_seeds: self.performance_runtime.random_seeds().clone(),
            },
        );
        snapshot.settings.midi_devices = self.midi_wanted.iter().cloned().collect();
        if self.audio_wanted {
            snapshot.settings.audio_input.device = self.ui.audio_device_id.clone();
        }
        snapshot
    }

    pub(crate) fn project_dirty(&self) -> bool {
        project::is_dirty(&self.project_snapshot(), self.last_saved_project.as_ref())
    }

    /// Dirty state for the status display, recomputed at most four times a
    /// second. Save, autosave and close paths call [`Self::project_dirty`].
    pub(crate) fn project_dirty_throttled(&mut self, now: Instant) -> bool {
        if self
            .last_dirty_check
            .is_none_or(|last| now.saturating_duration_since(last) >= DIRTY_CHECK_INTERVAL)
        {
            self.project_dirty_cached = self.project_dirty();
            self.last_dirty_check = Some(now);
        }
        self.project_dirty_cached
    }

    pub(crate) fn path_from_ui(&self) -> Option<PathBuf> {
        let value = self.ui.project_path.trim();
        if value.is_empty() {
            return None;
        }
        let path = PathBuf::from(value);
        Some(if path.is_absolute() {
            path
        } else {
            self.workspace.join(path)
        })
    }

    pub(crate) fn save_project_from_ui(&mut self) {
        if self.pending_project_open.is_some() {
            self.project_status =
                "A save-before-open is pending; wait for it to finish.".to_owned();
            return;
        }
        let Some(path) = self.path_from_ui() else {
            self.project_status = "Enter a project path first.".to_owned();
            return;
        };
        let request = SaveRequest {
            epoch: self.project_epoch,
            supersedes: vec![autosave_path(self.project_path.as_deref(), &self.workspace)],
            path,
            snapshot: self.project_snapshot(),
            kind: SaveKind::Project,
        };
        self.project_status = match self.submit_project_save(request) {
            Ok(()) => "Saving project…".to_owned(),
            Err(error) => error.to_owned(),
        };
    }

    fn submit_project_save(&mut self, request: SaveRequest) -> Result<(), &'static str> {
        let path = request.path.clone();
        let result = self.project_saver.submit(request);
        self.ui.save_status.submitted(&path, &result);
        result
    }

    pub(crate) fn poll_project_saves(&mut self) {
        while let Some(completion) = self.project_saver.try_recv() {
            self.apply_save_completion(completion);
        }
    }

    fn apply_save_completion(&mut self, completion: SaveCompletion) {
        if !completion.belongs_to(self.project_epoch) {
            // A replaced show's save can still fail after the switch. Surface
            // it so nobody believes the old show was saved; the pending count
            // already belongs to the new show and is left alone.
            if let Err(error) = &completion.result
                && completion.request.kind != SaveKind::Recovery
            {
                self.ui.save_status.error = Some((completion.request.path.clone(), error.clone()));
            }
            return;
        }
        let request = completion.request;
        self.ui
            .save_status
            .completed(&request.path, &completion.result);
        let then_open = request.kind == SaveKind::ProjectThenOpen;
        let succeeded = completion.result.is_ok();
        // Save As may have changed the recovery destination while this older
        // autosave was in flight. It must not advertise the old recovery file.
        if request.kind == SaveKind::Recovery
            && request.path != autosave_path(self.project_path.as_deref(), &self.workspace)
        {
            return;
        }
        match completion.result {
            Ok(()) if matches!(request.kind, SaveKind::Project | SaveKind::ProjectThenOpen) => {
                self.project_path = Some(request.path.clone());
                self.ui.project_path = request.path.to_string_lossy().into_owned();
                self.last_saved_project = Some(request.snapshot);
                self.last_dirty_check = None;
                self.recovery_path = None;
                self.project_status = format!("Saved {}", display_path(&request.path));
            }
            // The worker discards a snapshot equal to the saved project.
            Ok(()) if !project::is_dirty(&request.snapshot, self.last_saved_project.as_ref()) => {}
            Ok(()) => {
                self.recovery_path = Some(request.path);
                self.project_status = "Autosaved recovery snapshot.".to_owned();
            }
            Err(error) => self.project_status = format!("Save failed: {error}"),
        }
        if then_open && let Some(pending) = self.pending_project_open.take() {
            if can_replace_after_save(
                &self.project_snapshot(),
                self.last_saved_project.as_ref(),
                succeeded,
                self.ui.show_mode,
            ) {
                self.finish_open_project(pending);
            } else if succeeded {
                self.project_status = "Saved, but opening was cancelled because the show changed or Show Mode was enabled. Open again when ready.".to_owned();
            } else {
                self.project_status.push_str(" · Current show kept open.");
            }
        }
    }

    pub(crate) fn finish_project_saves(&mut self) {
        // Closing must never open a different show as a side effect of draining saves.
        self.pending_project_open = None;
        // Closing has ended presentation: finish accepted Save/Save As requests
        // before choosing the final recovery path, then durably save the latest
        // state even when the background queue had been full.
        for completion in self.project_saver.finish() {
            self.apply_save_completion(completion);
        }
        if self.project_dirty() {
            let path = autosave_path(self.project_path.as_deref(), &self.workspace);
            if let Err(error) = save_project_portable(&path, &self.project_snapshot()) {
                log::error!("close-time recovery save failed: {error}");
            }
        }
    }

    /// Native Save As. Like media relink, the modal dialog runs on this
    /// thread, so it is refused in Show Mode where output must keep moving.
    pub(crate) fn save_project_as_dialog(&mut self) {
        if self.pending_project_open.is_some() {
            self.project_status =
                "A save-before-open is pending; wait for it to finish.".to_owned();
            return;
        }
        if self.ui.show_mode {
            self.project_status = "Save As is unavailable in Show Mode.".to_owned();
            return;
        }
        let current = self.path_from_ui();
        let mut dialog = rfd::FileDialog::new()
            .set_title("Save Show As")
            .add_filter("VIRTUAL project", &["virtual"]);
        if let Some(parent) = current.as_deref().and_then(Path::parent)
            && parent.is_dir()
        {
            dialog = dialog.set_directory(parent);
        }
        let name = current
            .as_deref()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .unwrap_or("show.virtual");
        let Some(path) = dialog.set_file_name(name).save_file() else {
            self.project_status = "Save As cancelled".to_owned();
            return;
        };
        self.ui.project_path = with_project_extension(path).to_string_lossy().into_owned();
        self.save_project_from_ui();
    }

    pub(crate) fn open_project_dialog(&mut self) {
        if self.ui.show_mode {
            self.project_status = "Leave Show Mode to open another show.".to_owned();
            return;
        }
        let mut dialog = rfd::FileDialog::new()
            .set_title("Open Show")
            .add_filter("VIRTUAL project", &["virtual", "oneiroi"]);
        if let Some(parent) = self.path_from_ui().as_deref().and_then(Path::parent)
            && parent.is_dir()
        {
            dialog = dialog.set_directory(parent);
        }
        match dialog.pick_file() {
            Some(path) => self.open_project(path, false),
            None => self.project_status = "Open cancelled".to_owned(),
        }
    }

    pub(crate) fn open_project_from_ui(&mut self) {
        if self.ui.show_mode {
            self.project_status = "Leave Show Mode to open another show.".to_owned();
            return;
        }
        let Some(path) = self.path_from_ui() else {
            self.project_status = "Enter a project path first.".to_owned();
            return;
        };
        self.open_project(path, false);
    }

    pub(crate) fn open_project(&mut self, path: PathBuf, recovered: bool) {
        if self.ui.show_mode || self.pending_project_open.is_some() {
            self.project_status =
                "Leave Show Mode and wait for any pending project switch before opening a show."
                    .to_owned();
            return;
        }
        match load_project(&path) {
            Ok(mut project_file) => {
                let base = path.parent().unwrap_or(&self.workspace);
                resolve_media_paths(&mut project_file, base);
                let pending = PendingProjectOpen {
                    path,
                    recovered,
                    project: project_file,
                };
                if self.project_dirty() {
                    let decision = rfd::MessageDialog::new()
                        .set_title("Unsaved show changes")
                        .set_description("Save the current show before opening another? Changes since the last save will be discarded if you choose Discard.")
                        .set_buttons(rfd::MessageButtons::YesNoCancelCustom(
                            "Save".to_owned(), "Discard".to_owned(), "Cancel".to_owned()))
                        .show();
                    match decision {
                        rfd::MessageDialogResult::Yes => self.save_before_open(pending),
                        rfd::MessageDialogResult::No => self.finish_open_project(pending),
                        rfd::MessageDialogResult::Custom(label) if label == "Save" => {
                            self.save_before_open(pending)
                        }
                        rfd::MessageDialogResult::Custom(label) if label == "Discard" => {
                            self.finish_open_project(pending)
                        }
                        _ => {
                            self.project_status =
                                "Open cancelled · Current show kept open.".to_owned()
                        }
                    }
                } else {
                    self.finish_open_project(pending);
                }
            }
            Err(error) => self.project_status = format!("Open failed: {error}"),
        }
    }

    /// At startup there is no outgoing user show to save or discard.
    pub(crate) fn open_initial_project(&mut self, path: PathBuf) {
        match load_project(&path) {
            Ok(mut project) => {
                resolve_media_paths(&mut project, path.parent().unwrap_or(&self.workspace));
                self.finish_open_project(PendingProjectOpen {
                    path,
                    recovered: false,
                    project,
                });
            }
            Err(error) => self.project_status = format!("Open failed: {error}"),
        }
    }

    fn save_before_open(&mut self, pending: PendingProjectOpen) {
        // The editable path may already contain the incoming show's name.
        // Always save to the outgoing show's actual path, or explicitly choose one.
        let path = match &self.project_path {
            Some(path) => path.clone(),
            None => {
                let Some(path) = rfd::FileDialog::new()
                    .set_title("Save Current Show Before Opening")
                    .add_filter("VIRTUAL project", &["virtual"])
                    .set_directory(&self.workspace)
                    .set_file_name("untitled.virtual")
                    .save_file()
                else {
                    self.project_status = "Open cancelled · Current show kept open.".to_owned();
                    return;
                };
                with_project_extension(path)
            }
        };
        let request = SaveRequest {
            epoch: self.project_epoch,
            supersedes: vec![autosave_path(self.project_path.as_deref(), &self.workspace)],
            path,
            snapshot: self.project_snapshot(),
            kind: SaveKind::ProjectThenOpen,
        };
        match self.submit_project_save(request) {
            Ok(()) => {
                self.pending_project_open = Some(pending);
                self.project_status = "Saving current show before opening…".to_owned();
            }
            Err(error) => {
                self.project_status = format!("Open cancelled: {error} · Current show kept open.")
            }
        }
    }

    fn finish_open_project(&mut self, pending: PendingProjectOpen) {
        let PendingProjectOpen {
            path,
            recovered,
            project: project_file,
        } = pending;
        let graph = project_file
            .graph
            .clone()
            .unwrap_or_else(virtual_graph::four_deck_performance_graph);
        if let Err(error) = self
            .performance_runtime
            .set_project_graph(graph, project_file.settings.output.composition_extent)
        {
            self.project_status = format!("Project graph rejected: {error:#}");
            return;
        }
        self.apply_project(project_file, recovered);
        if recovered {
            self.project_path = None;
            self.recovery_path = None;
            let destination = paths::recovered_save_path(&path, &self.workspace);
            self.project_status = format!(
                "Recovered autosave from {} · Save writes {}",
                display_path(&path),
                display_path(&destination)
            );
            self.ui.project_path = destination.to_string_lossy().into_owned();
        } else {
            self.project_path = Some(path.clone());
            self.ui.project_path = path.to_string_lossy().into_owned();
            let recovery = autosave_path(Some(&path), &self.workspace);
            self.recovery_path = recovery_is_newer(&path, &recovery).then_some(recovery);
            self.project_status = format!("Opened {}", display_path(&path));
        }
    }

    pub(crate) fn apply_project(&mut self, project_file: ProjectFile, recovered: bool) {
        for recording in self.camera_recordings.iter_mut().flatten() {
            recording.canceled = true;
            recording.finalizing = true;
            recording.recorder.stop();
        }
        self.master_effect_processor.reset_history();
        self.project_id.clone_from(&project_file.project_id);
        self.project_takes.clone_from(&project_file.takes);
        self.project_epoch = self.project_epoch.wrapping_add(1);
        self.pending_project_open = None;
        self.ui.save_status = Default::default();
        self.clips = ClipBank::default();
        self.ui.clear_thumbnails();
        self.thumbnail_requests.clear();
        self.folder_pending.clear();
        self.relink_pending.clear();
        self.relink_active.clear();
        self.recording_pending.clear();
        self.folder_status.clear();
        self.live_configs = std::array::from_fn(|_| None);
        self.launches = LaunchQueue::default();
        self.restore_active = [None; 4];
        self.restore_selected = [0; 4];
        self.restore_transport = [None; 4];
        project::apply_master(&project_file, &mut self.ui);
        let _ = self.apply_output_settings();
        self.midi = project::apply_midi(&project_file);
        self.midi_wanted = project_file.settings.midi_devices.iter().cloned().collect();
        // A project owns its controller rig. Do not let a controller retained
        // from the previous project keep driving the freshly loaded mappings.
        self.midi_connections
            .retain(|connection| self.midi_wanted.contains(connection.device_id()));
        // Reconnect whatever hardware from the saved rig is present right now;
        // the rest reconnects automatically when it appears.
        self.refresh_midi_inputs();
        self.restore_midi_clock_output();
        self.restore_audio_input(&project_file.settings.audio_input.device);

        for deck in DeckId::ALL {
            let index = deck.index();
            self.mixer.eject(deck);
            let generation = self.mixer.deck(deck).generation;
            self.reset_playback(deck, generation);
            let deck_project = &project_file.decks[index];
            let transport = project::apply_deck(deck, deck_project, &mut self.mixer, &mut self.ui);
            self.transports[index] = transport;
            self.clips.select(ClipAddress {
                deck,
                slot: deck_project.selected_slot,
            });
            self.restore_selected[index] = deck_project.selected_slot;
            self.restore_active[index] = deck_project.active_slot;
            self.clips.restore_active(deck, deck_project.active_slot);
            self.restore_transport[index] = deck_project.active_slot.map(|_| transport);

            for (slot, path) in deck_project.clips.iter().enumerate() {
                let address = ClipAddress { deck, slot };
                let path = path.clone();
                if let Some(path) = &path {
                    self.clips.begin_restore(address, path.clone());
                }
                if let Some(playback) = deck_project.clip_playback.get(slot) {
                    self.clips
                        .set_playback(address, project::clip_playback_from_project(*playback));
                }
                let Some(path) = path else {
                    continue;
                };
                if let Err(request) = self.restorer.submit(ClipRestoreRequest {
                    address,
                    path,
                    project_epoch: self.project_epoch,
                }) {
                    self.clips.fail_restore(
                        request.address,
                        request.path,
                        "Restore queue is full.".to_owned(),
                    );
                }
            }
            if let Some(camera) = &deck_project.camera {
                let config = project::camera_from_project(camera);
                let generation = self.mixer.connect_camera(deck, config.clone());
                self.live_configs[index] = Some(config.clone());
                self.reset_playback(deck, generation);
                self.transports[index] = transport;
                self.transports[index].end_mode = virtual_media::EndMode::OneShot;
                self.decoders[index].connect_camera(config, generation);
            }
            if let Some(generator_stack) = &deck_project.generator_stack {
                let settings = project::generator_stack_from_project(generator_stack);
                let generation = self.mixer.connect_generator(deck, settings.clone());
                self.reset_playback(deck, generation);
                self.transports[index] = transport;
                self.transports[index].end_mode = virtual_media::EndMode::OneShot;
                self.generator_sent[index] = Some(settings.clone());
                self.decoders[index].connect_generator(settings, generation);
            } else if let Some(generator) = &deck_project.generator {
                let settings = virtual_generate::GeneratorStack::new(
                    project::generator_from_project(generator),
                );
                let generation = self.mixer.connect_generator(deck, settings.clone());
                self.reset_playback(deck, generation);
                self.transports[index] = transport;
                self.transports[index].end_mode = virtual_media::EndMode::OneShot;
                self.generator_sent[index] = Some(settings.clone());
                self.decoders[index].connect_generator(settings, generation);
            }
        }

        let mut baseline = self.session_state_snapshot();
        baseline.random_seeds.clone_from(&project_file.random_seeds);
        if let Err(error) = self.performance_runtime.start_project_baseline(
            baseline,
            &self.project_id,
            self.show_time_at(Instant::now()),
        ) {
            log::error!("start project-linked take: {error:#}");
        }

        self.last_saved_project = (!recovered).then_some(project_file);
        self.last_dirty_check = None;
        self.performance_started = Instant::now();
        self.last_autosave = Instant::now();
    }

    pub(crate) fn poll_restores(&mut self) {
        while let Ok(result) = self.restorer.try_recv() {
            if result.project_epoch != self.project_epoch {
                continue;
            }
            let folder_result = self.folder_pending.remove(&result.address);
            let recording_result = self.recording_pending.remove(&result.address);
            if self.clips.path(result.address) != Some(result.path.as_path()) {
                if folder_result && self.folder_pending.is_empty() {
                    self.folder_status = "Folder import complete".to_owned();
                }
                continue;
            }
            let relink_result = self.relink_pending.remove(&result.address);
            let relink_active = self.relink_active.remove(&result.address);
            match result.metadata {
                Ok(movie) => {
                    let address = result.address;
                    let duration = movie.duration.map(MediaTime::as_seconds);
                    if folder_result || relink_result || recording_result {
                        self.record_show_operation(
                            CommandOrigin::Operator,
                            Instant::now(),
                            CommandOperation::SetParameter {
                                path: format!(
                                    "deck.{}.clip.{}.media",
                                    address.deck.index(),
                                    address.slot
                                ),
                                value: virtual_graph::ParameterValue::Text(
                                    result.path.to_string_lossy().into_owned(),
                                ),
                            },
                        );
                    }
                    self.clips.restore(address, movie);
                    self.request_thumbnail(address, result.path.clone());
                    if relink_active
                        || self.restore_active[address.deck.index()] == Some(address.slot)
                    {
                        let desired = self.restore_transport[address.deck.index()].take();
                        self.launch_clip(address);
                        self.clips.select(ClipAddress {
                            deck: address.deck,
                            slot: self.restore_selected[address.deck.index()],
                        });
                        if let Some(mut transport) = desired {
                            transport.duration = duration;
                            self.transports[address.deck.index()] = transport;
                            if transport.position > 0.0 {
                                self.seek_deck(address.deck);
                            }
                        }
                    }
                    if relink_result {
                        self.project_status = format!(
                            "Relinked Deck {} slot {}",
                            address.deck.label(),
                            address.slot + 1
                        );
                    } else if recording_result {
                        self.project_status = format!(
                            "Recorded Deck {} clip {}",
                            address.deck.label(),
                            address.slot + 1
                        );
                    }
                }
                Err(error) => {
                    let message = error.to_string();
                    self.clips
                        .fail_restore(result.address, result.path, message.clone());
                    if relink_result {
                        self.project_status = format!("Relink failed: {message}");
                    } else if recording_result {
                        self.project_status = format!("Recording import failed: {message}");
                    }
                }
            }
            if folder_result && self.folder_pending.is_empty() {
                self.folder_status = "Folder import complete".to_owned();
            } else if folder_result {
                self.folder_status = format!(
                    "Folder import · {} file(s) remaining",
                    self.folder_pending.len()
                );
            }
        }
    }

    pub(crate) fn maybe_autosave(&mut self, now: Instant) {
        if now.saturating_duration_since(self.last_autosave) < Duration::from_secs(5) {
            return;
        }
        self.last_autosave = now;
        self.autosave_recovery();
    }

    pub(crate) fn autosave_recovery(&mut self) {
        if !self.project_dirty() {
            return;
        }
        let path = autosave_path(self.project_path.as_deref(), &self.workspace);
        let request = SaveRequest {
            epoch: self.project_epoch,
            path,
            snapshot: self.project_snapshot(),
            kind: SaveKind::Recovery,
            supersedes: Vec::new(),
        };
        if let Err(error) = self.submit_project_save(request) {
            self.project_status = format!("Autosave deferred: {error}");
        }
    }
}

fn can_replace_after_save(
    current: &ProjectFile,
    saved: Option<&ProjectFile>,
    succeeded: bool,
    locked: bool,
) -> bool {
    succeeded && !locked && saved.is_some() && !project::is_dirty(current, saved)
}

/// Keep saves recognizable as shows: `set` and `set.show` both become
/// `….virtual`, while an existing project extension is left alone.
fn with_project_extension(path: PathBuf) -> PathBuf {
    if paths::is_project_path(&path) {
        return path;
    }
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".virtual");
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_waits_for_success_and_preserves_edits_made_during_save() {
        let saved = ProjectFile::default();
        let mut live = saved.clone();
        assert!(!can_replace_after_save(&live, None, true, false));
        assert!(!can_replace_after_save(&live, Some(&saved), false, false));
        assert!(!can_replace_after_save(&live, Some(&saved), true, true));
        live.decks[0].transport.position = 20.0;
        assert!(can_replace_after_save(&live, Some(&saved), true, false));
        live.settings.bpm = 135.0;
        assert!(!can_replace_after_save(&live, Some(&saved), true, false));
    }

    #[test]
    fn save_as_appends_the_project_extension_only_when_missing() {
        assert_eq!(
            with_project_extension("/shows/set".into()),
            PathBuf::from("/shows/set.virtual")
        );
        assert_eq!(
            with_project_extension("/shows/set.v2".into()),
            PathBuf::from("/shows/set.v2.virtual")
        );
        assert_eq!(
            with_project_extension("/shows/set.virtual".into()),
            PathBuf::from("/shows/set.virtual")
        );
    }
}
