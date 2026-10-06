//! Per-deck transport, seeking and playhead updates.

use std::path::PathBuf;
use std::time::Instant;

use virtual_core::MediaTime;
use virtual_media::{
    ClipAddress, DeckId, DeckState, DecoderEvent, DiscontinuityPolicy, FrameScheduler,
    FrameSelection, TransportEvent,
};

use super::State;

impl State {
    pub(crate) fn reset_playback(&mut self, deck: DeckId, generation: u64) {
        let index = deck.index();
        self.decoders[index].stop();
        self.compositor.clear_deck(index);
        self.schedulers[index] = FrameScheduler::new(4, generation, DiscontinuityPolicy::Blank)
            .expect("non-zero frame queue");
        let duration = match &self.mixer.deck(deck).state {
            DeckState::Ready(movie) => movie.duration.map(MediaTime::as_seconds),
            DeckState::Live(_)
            | DeckState::Generator(_)
            | DeckState::Empty
            | DeckState::Loading { .. }
            | DeckState::Error { .. } => None,
        };
        self.transports[index].reset(duration);
        self.last_transport_updates[index] = Instant::now();
        self.media_origins[index] = None;
        self.playback_generations[index] = generation;
    }

    pub(crate) fn seek_deck(&mut self, deck: DeckId) {
        let index = deck.index();
        let DeckState::Ready(movie) = &self.mixer.deck(deck).state else {
            return;
        };
        let path = movie.path.clone();
        let decode_path = movie.decode_path;
        let epoch = self.playback_generations[index].wrapping_add(1);
        self.playback_generations[index] = epoch;
        self.schedulers[index] = FrameScheduler::new(4, epoch, DiscontinuityPolicy::HoldLastFrame)
            .expect("non-zero frame queue");
        let target = self.media_origins[index].and_then(|origin| {
            let micros =
                (self.transports[index].position * 1_000_000.0).clamp(0.0, i64::MAX as f64) as i64;
            origin
                .checked_add(MediaTime::new(micros, 1_000_000).ok()?)
                .ok()
        });
        let seek_to = if decode_path == virtual_media::DecodePath::FfmpegVideo {
            target.and_then(|target| movie.keyframes.nearest_preceding(target))
        } else {
            None
        };
        self.decoders[index].load_indexed(path, decode_path, epoch, target, seek_to);
        self.last_transport_updates[index] = Instant::now();
    }

    pub(crate) fn update_playback(&mut self, now: Instant) {
        for deck in DeckId::ALL {
            let index = deck.index();
            let media_generation = self.mixer.deck(deck).generation;
            if media_generation != self.playback_generations[index]
                && !matches!(self.mixer.deck(deck).state, DeckState::Ready(_))
            {
                self.reset_playback(deck, media_generation);
            }
            self.sync_clip_range(deck);
            let delta = now
                .saturating_duration_since(self.last_transport_updates[index])
                .as_secs_f64();
            self.last_transport_updates[index] = now;
            if !self.ui.master_freeze
                && matches!(
                    self.transports[index].advance(delta),
                    TransportEvent::Loop { .. }
                )
            {
                self.seek_deck(deck);
            }
            self.sync_generator(deck);
            let generation = self.playback_generations[index];
            while let Ok(event) = self.decoders[index].try_event() {
                match event {
                    DecoderEvent::Loaded {
                        generation: loaded_generation,
                    } if loaded_generation == generation && self.live_configs[index].is_some() => {
                        self.camera_status = format!("Deck {} video input is live", deck.label());
                    }
                    DecoderEvent::Error {
                        generation: failed_generation,
                        message,
                    } if failed_generation == generation => {
                        let path = match &self.mixer.deck(deck).state {
                            DeckState::Ready(movie) => movie.path.clone(),
                            DeckState::Live(config) => config.virtual_path(),
                            DeckState::Generator(settings) => {
                                PathBuf::from(format!("generator://{}", settings.pattern.id()))
                            }
                            DeckState::Loading { path } | DeckState::Error { path, .. } => {
                                path.clone()
                            }
                            DeckState::Empty => PathBuf::new(),
                        };
                        if self.live_configs[index].is_some() {
                            self.camera_status =
                                format!("Deck {} video input error: {message}", deck.label());
                        }
                        self.mixer.deck_mut(deck).state = DeckState::Error { path, message };
                        self.compositor.clear_deck(index);
                    }
                    DecoderEvent::Ended {
                        generation: ended_generation,
                    } if ended_generation == generation && self.live_configs[index].is_some() => {
                        self.camera_status =
                            format!("Deck {} video input disconnected", deck.label());
                    }
                    DecoderEvent::Loaded { .. }
                    | DecoderEvent::Ended { .. }
                    | DecoderEvent::Error { .. } => {}
                }
            }

            let generated = matches!(self.mixer.deck(deck).state, DeckState::Generator(_));
            // Leave future movie frames in the bounded worker channel until
            // the scheduler has room. Reading first used to discard one frame
            // per UI tick while full, racing through movies (especially at
            // slow speeds or while paused) and leaving long frozen gaps.
            while self.schedulers[index].has_capacity() {
                let Ok(frame) = self.decoders[index].try_frame() else {
                    break;
                };
                if frame.generation != generation {
                    continue;
                }
                if (self.live_configs[index].is_some() || generated)
                    && let virtual_media::VideoFramePayload::Rgba8(rgba) = &frame.payload
                    && let Some(recording) = &mut self.camera_recordings[index]
                    && !recording.finalizing
                {
                    recording.recorder.try_push(rgba, frame.pts, frame.duration);
                }
                self.media_origins[index].get_or_insert(frame.pts);
                if generated && !self.transports[index].frozen && !self.ui.master_freeze {
                    // A generator has no timeline to seek. Keep its newest
                    // frame due now, so releasing a deck or master freeze
                    // resumes at once instead of waiting out the pause.
                    let micros = (self.transports[index].position * 1_000_000.0)
                        .clamp(0.0, i64::MAX as f64) as i64;
                    if let Ok(position) = MediaTime::new(micros, 1_000_000)
                        && let Ok(due) = frame.pts.checked_sub(position)
                        && self.media_origins[index].is_some_and(|origin| origin < due)
                    {
                        self.media_origins[index] = Some(due);
                    }
                }
                if self.schedulers[index].enqueue(frame).is_err() {
                    break;
                }
            }

            let Some(origin) = self.media_origins[index] else {
                continue;
            };
            let elapsed =
                (self.transports[index].position * 1_000_000.0).clamp(0.0, i64::MAX as f64) as i64;
            let Ok(target) =
                origin.checked_add(MediaTime::new(elapsed, 1_000_000).expect("positive timescale"))
            else {
                continue;
            };
            if let FrameSelection::Advanced(frame) = self.schedulers[index].select(target)
                && let Err(error) =
                    self.compositor
                        .upload(&self.gpu.device, &self.gpu.queue, index, &frame.payload)
            {
                log::error!("deck {} upload failed: {error}", deck.label());
            }
        }
    }

    /// Forwards in-place UI edits and the live audio bands to a generator
    /// deck's worker. Both are non-blocking.
    fn sync_generator(&mut self, deck: DeckId) {
        let index = deck.index();
        let DeckState::Generator(settings) = &self.mixer.deck(deck).state else {
            self.generator_sent[index] = None;
            return;
        };
        if self.generator_sent[index].as_ref() != Some(settings) {
            self.decoders[index].update_generator(settings.clone());
            self.generator_sent[index] = Some(settings.clone());
        }
        if settings.audio_amount > 0.0 {
            let analysis = self.audio_snapshot.analysis;
            self.decoders[index].set_generator_audio(virtual_generate::AudioBands {
                bass: analysis.bass,
                mid: analysis.mid,
                high: analysis.high,
            });
        }
    }

    pub(crate) fn sync_clip_range(&mut self, deck: DeckId) {
        let Some(slot) = self.clips.active(deck) else {
            return;
        };
        let address = ClipAddress { deck, slot };
        let Some(movie) = self.clips.movie(address) else {
            return;
        };
        let playback = self.clips.playback(address).unwrap_or_default();
        let media_duration = movie.duration.map(MediaTime::as_seconds);
        let (in_point, out_point) = playback.range(media_duration, self.ui.bpm);
        let transport = &mut self.transports[deck.index()];
        transport.in_point = in_point;
        transport.duration = out_point;
        if transport.position < in_point {
            transport.position = in_point;
            self.seek_deck(deck);
        }
    }
}
