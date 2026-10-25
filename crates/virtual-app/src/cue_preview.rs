//! Muted moving source cue, completely independent of live deck transports.
use crate::State;
use std::path::PathBuf;
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use virtual_core::MediaTime;
use virtual_media::{ClipAddress, ClipPlayback, FfmpegVideoDecoder, RgbaFrame};

#[derive(Clone, PartialEq)]
struct Source {
    epoch: u64,
    address: ClipAddress,
    path: PathBuf,
    playback: ClipPlayback,
    bpm: f64,
}
struct Request {
    generation: u64,
    path: PathBuf,
    start: f64,
    end: Option<f64>,
    position: f64,
    playing: bool,
}
struct CueFrame {
    generation: u64,
    frame: Result<(RgbaFrame, f64), String>,
}
#[derive(Default)]
struct Shared {
    request: Mutex<Option<Request>>,
    wake: Condvar,
    latest: Mutex<Option<CueFrame>>,
    generation: AtomicU64,
    stop: AtomicBool,
}
pub struct CuePreview {
    shared: Arc<Shared>,
    worker: Option<JoinHandle<()>>,
    source: Option<Source>,
    generation: u64,
}
impl CuePreview {
    pub fn new() -> Self {
        let shared = Arc::new(Shared::default());
        let worker_shared = shared.clone();
        let worker = thread::Builder::new()
            .name("virtual-cue-preview".into())
            .spawn(move || worker_loop(&worker_shared))
            .expect("spawn cue preview");
        Self {
            shared,
            worker: Some(worker),
            source: None,
            generation: 0,
        }
    }
    fn clear(&mut self) {
        self.source = None;
        self.generation = self.generation.wrapping_add(1);
        self.shared
            .generation
            .store(self.generation, Ordering::Release);
        *self.shared.request.lock().expect("cue request") = None;
        self.shared.wake.notify_one();
    }
    fn request(
        &mut self,
        source: Source,
        start: f64,
        end: Option<f64>,
        position: f64,
        playing: bool,
    ) {
        self.generation = self.generation.wrapping_add(1);
        self.shared
            .generation
            .store(self.generation, Ordering::Release);
        *self.shared.request.lock().expect("cue request") = Some(Request {
            generation: self.generation,
            path: source.path.clone(),
            start,
            end,
            position,
            playing,
        });
        self.source = Some(source);
        self.shared.wake.notify_one();
    }
}
impl Drop for CuePreview {
    fn drop(&mut self) {
        {
            let _guard = self.shared.request.lock().expect("cue request");
            self.shared.stop.store(true, Ordering::Release);
            self.shared.wake.notify_one();
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
fn seconds_time(seconds: f64) -> MediaTime {
    MediaTime::new((seconds.max(0.0) * 1e6).round() as i64, 1_000_000).expect("positive timescale")
}
fn current(shared: &Shared, generation: u64) -> bool {
    !shared.stop.load(Ordering::Acquire) && shared.generation.load(Ordering::Acquire) == generation
}
fn publish(shared: &Shared, generation: u64, frame: Result<(RgbaFrame, f64), String>) {
    if current(shared, generation) {
        *shared.latest.lock().expect("cue result") = Some(CueFrame { generation, frame });
    }
}
fn worker_loop(shared: &Shared) {
    loop {
        let request = {
            let mut pending = shared.request.lock().expect("cue request");
            while pending.is_none() && !shared.stop.load(Ordering::Acquire) {
                pending = shared.wake.wait(pending).expect("cue request");
            }
            if shared.stop.load(Ordering::Acquire) {
                return;
            }
            pending.take().expect("request ready")
        };
        if let Err(error) = play_source(shared, &request) {
            publish(shared, request.generation, Err(error));
        }
    }
}
fn play_source(shared: &Shared, request: &Request) -> Result<(), String> {
    let mut position = request.position;
    'play: loop {
        if !current(shared, request.generation) {
            return Ok(());
        }
        let mut decoder =
            FfmpegVideoDecoder::open_for_preview(&request.path, [480, 270], seconds_time(position))
                .map_err(|error| error.to_string())?;
        let anchor = Instant::now();
        let mut sent = false;
        let mut last_frame = f64::NEG_INFINITY;
        let mut count = 0usize;
        let mut last_end = position;
        while current(shared, request.generation) {
            let Some(frame) = decoder.next_frame().map_err(|error| error.to_string())? else {
                break;
            };
            count += 1;
            let at = frame.pts.as_seconds();
            last_end = at + frame.duration.map_or(1.0 / 30.0, MediaTime::as_seconds);
            if last_end <= position + 1e-9 {
                continue;
            }
            if request.end.is_some_and(|end| at >= end) {
                break;
            }
            let due = Duration::from_secs_f64((at - position).max(0.0));
            while current(shared, request.generation) && anchor.elapsed() < due {
                let wait = (due - anchor.elapsed().min(due)).min(Duration::from_millis(20));
                let guard = shared.request.lock().expect("cue request");
                if guard.is_some() {
                    break 'play;
                }
                let _ = shared.wake.wait_timeout(guard, wait).expect("cue wake");
            }
            if !current(shared, request.generation) {
                return Ok(());
            }
            if !sent || at - last_frame >= 1.0 / 30.0 - 1e-6 {
                publish(
                    shared,
                    request.generation,
                    Ok((frame.pixels, at.max(request.start))),
                );
                sent = true;
                last_frame = at;
            }
            if !request.playing {
                return Ok(());
            }
        }
        if !sent || (count <= 1 && request.end.is_none()) {
            return Ok(());
        }
        let remaining =
            Duration::from_secs_f64((request.end.unwrap_or(last_end) - position).max(0.0));
        while current(shared, request.generation) && anchor.elapsed() < remaining {
            let wait = (remaining - anchor.elapsed().min(remaining)).min(Duration::from_millis(20));
            let guard = shared.request.lock().expect("cue request");
            if guard.is_some() {
                return Ok(());
            }
            let _ = shared.wake.wait_timeout(guard, wait).expect("cue wake");
        }
        position = request.start;
    }
    Ok(())
}

impl State {
    pub(crate) fn update_cue_preview(&mut self) {
        let address = ClipAddress {
            deck: self.mixer.selected(),
            slot: self.clips.selected(self.mixer.selected()),
        };
        let movie = self.clips.movie(address).cloned();
        let Some(movie) = movie else {
            if self.cue.source.is_some() {
                self.cue.clear();
            }
            self.ui.cue_status = "Select a media clip to cue".into();
            return;
        };
        let playback = self.clips.playback(address).unwrap_or_default();
        let duration = movie.duration.map(MediaTime::as_seconds);
        let (start, end) = playback.range(duration, self.ui.bpm);
        let source = Source {
            epoch: self.project_epoch,
            address,
            path: movie.path,
            playback,
            bpm: self.ui.bpm,
        };
        self.ui.cue_range = [start, end.unwrap_or(start)];
        if self.cue.source.as_ref() != Some(&source) {
            self.ui.cue_position = start;
            self.ui.cue_status = "Loading source cue…".into();
            self.cue
                .request(source.clone(), start, end, start, self.ui.cue_playing);
        }
        let latest = self.cue.shared.latest.lock().expect("cue result").take();
        if let Some(result) = latest
            && result.generation == self.cue.generation
        {
            match result.frame {
                Ok((frame, position)) => {
                    let image = egui::ColorImage::from_rgba_unmultiplied(
                        frame.extent.map(|size| size as usize),
                        &frame.data,
                    );
                    if let Some((target, path, texture)) = &mut self.ui.cue_preview
                        && *target == address
                        && *path == source.path
                    {
                        texture.set(image, egui::TextureOptions::LINEAR);
                    } else {
                        let texture = self.egui_state.egui_ctx().load_texture(
                            "moving-cue",
                            image,
                            egui::TextureOptions::LINEAR,
                        );
                        self.ui.cue_preview = Some((address, source.path.clone(), texture));
                    }
                    self.ui.cue_position = position;
                    self.ui.cue_status = if self.ui.cue_playing {
                        "Playing source · muted"
                    } else {
                        "Paused source · muted"
                    }
                    .into();
                }
                Err(error) => self.ui.cue_status = format!("Cue unavailable: {error}"),
            }
        }
    }
    pub(crate) fn seek_cue_preview(&mut self, position: f64) {
        if !position.is_finite() {
            return;
        }
        let Some(source) = self.cue.source.clone() else {
            return;
        };
        let [start, end] = self.ui.cue_range;
        let last = if end > start {
            (end - 1e-6).max(start)
        } else {
            start
        };
        let position = position.clamp(start, last);
        self.ui.cue_position = position;
        self.cue.request(
            source,
            start,
            (end > start).then_some(end),
            position,
            self.ui.cue_playing,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use virtual_media::{CameraRecorder, DeckId};

    #[test]
    fn moving_cue_is_scaled_and_new_pause_requests_reject_stale_frames() {
        let path =
            std::env::temp_dir().join(format!("virtual-cue-{}.mov", virtual_io::new_project_id()));
        let mut writer = CameraRecorder::start(path.clone(), 10).unwrap();
        for frame in 0..4 {
            let pixels = RgbaFrame {
                extent: [960, 540],
                data: [frame as u8 * 60, 30, 50, 255].repeat(960 * 540).into(),
            };
            writer.try_push(
                &pixels,
                MediaTime::new(frame, 10).unwrap(),
                Some(MediaTime::new(1, 10).unwrap()),
            );
        }
        writer.finish().unwrap().result.unwrap();
        let mut cue = CuePreview::new();
        let source = Source {
            epoch: 0,
            address: ClipAddress {
                deck: DeckId::A,
                slot: 0,
            },
            path: path.clone(),
            playback: ClipPlayback::default(),
            bpm: 120.0,
        };
        cue.request(source.clone(), 0.0, Some(0.4), 0.0, true);
        let deadline = Instant::now() + Duration::from_secs(4);
        let mut moving = false;
        while Instant::now() < deadline {
            if let Some(frame) = cue.shared.latest.lock().unwrap().take() {
                let (pixels, at) = frame.frame.unwrap();
                assert_eq!(pixels.extent, [480, 270]);
                if at >= 0.1 && pixels.data[0] >= 60 {
                    moving = true;
                    break;
                }
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert!(moving, "cue must advance without a live transport");
        cue.request(source.clone(), 0.0, Some(0.4), 0.2, false);
        let generation = cue.generation;
        let deadline = Instant::now() + Duration::from_secs(4);
        let mut paused = false;
        while Instant::now() < deadline {
            if let Some(frame) = cue.shared.latest.lock().unwrap().take()
                && frame.generation == generation
            {
                let (pixels, at) = frame.frame.unwrap();
                assert!((at - 0.2).abs() < 0.01);
                assert_eq!(pixels.data[0], 120);
                paused = true;
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert!(paused);
        thread::sleep(Duration::from_millis(120));
        assert!(
            cue.shared.latest.lock().unwrap().take().is_none(),
            "paused cue stays still"
        );
        cue.request(source, 0.0, Some(0.4), 0.399_999, false);
        let generation = cue.generation;
        let deadline = Instant::now() + Duration::from_secs(4);
        let mut at_end = false;
        while Instant::now() < deadline {
            if let Some(frame) = cue.shared.latest.lock().unwrap().take()
                && frame.generation == generation
            {
                let (pixels, at) = frame.frame.unwrap();
                assert!((at - 0.3).abs() < 0.01);
                assert_eq!(pixels.data[0], 180);
                at_end = true;
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert!(at_end, "the Out point cues the last included source frame");
        drop(cue);
        std::fs::remove_file(path).unwrap();
    }
}
