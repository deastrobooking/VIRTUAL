//! Sequence playback uses the existing generation-safe deck launch/seek path.
use crate::State;
use std::time::Instant;
use virtual_media::{ClipAddress, DeckId, EndMode};
use virtual_session::{CommandOperation, CommandOrigin};

impl State {
    pub(crate) fn process_sequence(&mut self, now: Instant) {
        let editor = &mut self.ui.session_editor;
        let fps = self.ui.sequence.fps;
        let elapsed = editor.last_tick.replace(now).map_or(0.0, |last| {
            now.saturating_duration_since(last).as_secs_f64()
        });
        if editor.playing {
            let frames = elapsed * f64::from(fps) + editor.fractional;
            editor.frame = editor.frame.saturating_add(frames.floor() as u64);
            editor.fractional = frames.fract();
            let end = self.ui.sequence.duration();
            if editor.frame >= end {
                editor.frame = end;
                editor.playing = false;
            }
        }
        if !editor.armed {
            if let Some(saved) = editor.saved_bypass.take() {
                self.ui.bypassed = saved;
                // Release ownership without starting paused layers unexpectedly.
                for transport in &mut self.transports {
                    transport.playing = false;
                }
            }
            editor.active = [None; 4];
            editor.was_playing = false;
            return;
        }
        if editor.saved_bypass.is_none() {
            editor.saved_bypass = Some(self.ui.bypassed);
            self.launches = Default::default();
            editor.seek_requested = true;
        }
        let frame = editor.frame;
        let playing = editor.playing;
        let seek = std::mem::take(&mut editor.seek_requested);
        let transport_changed = playing != editor.was_playing;
        editor.was_playing = playing;
        let desired: [Option<virtual_core::sequence::SequenceClip>; 4] =
            std::array::from_fn(|i| self.ui.sequence.at(i as u8, frame).cloned());
        for (i, clip) in desired.into_iter().enumerate() {
            let deck = DeckId::ALL[i];
            let Some(clip) = clip else {
                self.ui.bypassed[i] = true;
                self.transports[i].playing = false;
                self.ui.session_editor.active[i] = None;
                continue;
            };
            let address = ClipAddress {
                deck,
                slot: usize::from(clip.slot),
            };
            let position =
                (clip.source_in + frame.saturating_sub(clip.start)) as f64 / f64::from(fps);
            let source_end = (clip.source_in + clip.duration) as f64 / f64::from(fps);
            let available = self.clips.movie(address).is_some_and(|movie| {
                movie
                    .duration
                    .is_none_or(|d| source_end <= d.as_seconds() + 1.0 / f64::from(fps))
            });
            if !available {
                self.ui.bypassed[i] = true;
                self.transports[i].playing = false;
                self.ui.session_editor.active[i] = None;
                self.ui.session_editor.status = format!(
                    "V{}: missing, live, or too-short source in slot {}",
                    i + 1,
                    clip.slot + 1
                );
                continue;
            }
            let changed = self.ui.session_editor.active[i] != Some(clip.id)
                || self.clips.active(deck) != Some(address.slot);
            if changed {
                self.record_show_operation(
                    CommandOrigin::Score,
                    now,
                    CommandOperation::LaunchClip {
                        deck: i as u8,
                        slot: clip.slot,
                    },
                );
                self.launch_clip(address);
                self.ui.session_editor.active[i] = Some(clip.id);
            }
            self.ui.bypassed[i] = false;
            self.transports[i].speed = 1.0;
            self.transports[i].frozen = false;
            self.transports[i].in_point = clip.source_in as f64 / f64::from(fps);
            self.transports[i].duration = Some(source_end);
            self.transports[i].end_mode = EndMode::OneShot;
            self.transports[i].playing = playing;
            if changed || seek || transport_changed {
                self.transports[i].position = position;
                self.record_show_operation(
                    CommandOrigin::Score,
                    now,
                    CommandOperation::SeekDeck {
                        deck: i as u8,
                        position_seconds: position,
                    },
                );
                self.seek_deck(deck);
            }
        }
    }

    pub(crate) fn export_sequence(&mut self) {
        if let Err(error) = self.ui.sequence.validate() {
            self.ui.session_editor.status = error.into();
            return;
        }
        let Some(path) = rfd::FileDialog::new()
            .set_file_name("show.otio")
            .add_filter("OpenTimelineIO", &["otio"])
            .save_file()
        else {
            return;
        };
        let sources: Vec<_> = self
            .ui
            .sequence
            .clips
            .iter()
            .filter_map(|clip| {
                let deck = DeckId::ALL[usize::from(clip.track)];
                let address = ClipAddress {
                    deck,
                    slot: usize::from(clip.slot),
                };
                self.clips
                    .path(address)
                    .map(|path| (clip.id, path.to_path_buf()))
            })
            .collect();
        if sources.len() != self.ui.sequence.clips.len() {
            self.ui.session_editor.status =
                "Export needs a resolved file-backed source for every timeline clip".into();
            return;
        }
        let sequence = self.ui.sequence.clone();
        // Bounded timeline export never writes on the render thread.
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        if self.sequence_export.is_some() {
            self.ui.session_editor.status = "A timeline export is already running".into();
            return;
        }
        std::thread::spawn(move || {
            let result =
                virtual_io::export_sequence_otio(&path, &sequence, &sources).map(|()| path);
            let _ = tx.send(result.map_err(|e| e.to_string()));
        });
        self.sequence_export = Some(rx);
        self.ui.session_editor.status = "Exporting source-edit timeline…".into();
    }

    pub(crate) fn poll_sequence_export(&mut self) {
        if let Some(receiver) = &self.sequence_export {
            match receiver.try_recv() {
                Ok(result) => {
                    self.ui.session_editor.status = match result {
                        Ok(path) => format!("Timeline exported: {}", path.display()),
                        Err(error) => format!("Timeline export failed: {error}"),
                    };
                    self.sequence_export = None;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.ui.session_editor.status = "Timeline export worker stopped".into();
                    self.sequence_export = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
    }
}
