//! Finished-program capture, after native FX and before presentation/mapping.
//! GPU staging and the existing MOV writer are bounded and asynchronous.
use crate::State;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Instant;
use virtual_core::MediaTime;
use virtual_media::{CameraRecorder, CameraRecordingResult, FrameBufferPool, RgbaFrame};
use virtual_render::{
    PROGRAM_FORMAT, PresentationOptions, ProgramPresenter, ProgramReadback, ProgramTarget,
    ReadbackFrame,
};

#[derive(Clone, Default)]
pub struct RecordingStatus {
    pub active: bool,
    pub finalizing: bool,
    pub elapsed: f64,
    pub dropped: u64,
    pub message: String,
    pub completed: bool,
}
pub struct ProgramRecording {
    recorder: Option<CameraRecorder>,
    readback: ProgramReadback,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    presenter: ProgramPresenter,
    source_extent: [u32; 2],
    extent: [u32; 2],
    fps: u32,
    started: Instant,
    stopping: Option<MediaTime>,
    writer_stopped: bool,
    sequence: u64,
    last_tick: Option<u64>,
    cadence_dropped: u64,
    timestamps: BTreeMap<u64, MediaTime>,
    pool: FrameBufferPool,
    destination: PathBuf,
    staging: PathBuf,
    keep_staging: bool,
}
fn media_time(seconds: f64) -> MediaTime {
    MediaTime::new((seconds.max(0.0) * 1e6).round() as i64, 1_000_000).expect("positive timescale")
}
impl ProgramRecording {
    pub fn start(
        device: &wgpu::Device,
        program: &ProgramTarget,
        destination: PathBuf,
        extent: [u32; 2],
        fps: u32,
        now: Instant,
    ) -> Result<Self, String> {
        if !matches!(extent, [1280, 720] | [1920, 1080]) || ![24, 30, 60].contains(&fps) {
            return Err("Choose 720p or 1080p and 24/30/60 fps".into());
        }
        let destination = destination.with_extension("mov");
        if destination.exists() {
            return Err("Choose a new recording filename; existing movies are preserved".into());
        }
        let parent = destination.parent().ok_or("Choose a recording directory")?;
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Create recording directory: {error}"))?;
        let staging = parent.join(format!(".virtual-program-{}", virtual_io::new_project_id()));
        std::fs::create_dir(&staging)
            .map_err(|error| format!("Create recording workspace: {error}"))?;
        let recorder = match CameraRecorder::start(staging.join("program.mov"), fps) {
            Ok(recorder) => recorder,
            Err(error) => {
                let _ = std::fs::remove_dir_all(&staging);
                return Err(error);
            }
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("program-recording"),
            size: wgpu::Extent3d {
                width: extent[0],
                height: extent[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: PROGRAM_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        Ok(Self {
            recorder: Some(recorder),
            readback: ProgramReadback::new(device, extent),
            texture,
            view,
            presenter: ProgramPresenter::new(device, program, PROGRAM_FORMAT),
            source_extent: program.extent(),
            extent,
            fps,
            started: now,
            stopping: None,
            writer_stopped: false,
            sequence: 0,
            last_tick: None,
            cadence_dropped: 0,
            timestamps: BTreeMap::new(),
            pool: FrameBufferPool::new(10),
            destination,
            staging,
            keep_staging: false,
        })
    }
    pub fn capture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        program: &ProgramTarget,
        now: Instant,
    ) {
        if self.stopping.is_some() {
            return;
        }
        let elapsed = now.saturating_duration_since(self.started).as_secs_f64();
        let tick = (elapsed * f64::from(self.fps)).floor() as u64;
        if self.last_tick == Some(tick) {
            return;
        }
        if let Some(last) = self.last_tick {
            self.cadence_dropped += tick.saturating_sub(last + 1);
        }
        self.last_tick = Some(tick);
        // ProgramTarget may be rebuilt even at the same size. Rebinding is
        // handled by State when output resources change, via source_changed.
        if self.source_extent != program.extent() {
            self.source_changed(device, program);
        }
        self.presenter.draw(
            queue,
            encoder,
            &self.view,
            self.extent,
            PresentationOptions::default(),
        );
        if self.readback.capture(encoder, &self.texture) {
            self.timestamps.insert(self.sequence, media_time(elapsed));
            self.sequence += 1;
        }
    }
    pub fn source_changed(&mut self, device: &wgpu::Device, program: &ProgramTarget) {
        self.presenter = ProgramPresenter::new(device, program, PROGRAM_FORMAT);
        self.source_extent = program.extent();
    }
    fn deliver(
        recorder: &mut CameraRecorder,
        pool: &FrameBufferPool,
        timestamps: &mut BTreeMap<u64, MediaTime>,
        frame: ReadbackFrame<'_>,
    ) {
        // Failed mapping slots have no pixels; discard their timestamp entries
        // as later successful frames arrive. The queue stays bounded by staging.
        timestamps.retain(|sequence, _| *sequence >= frame.sequence);
        let Some(pts) = timestamps.remove(&frame.sequence) else {
            return;
        };
        let row_bytes = frame.extent[0] as usize * 4;
        let mut pixels = pool.acquire(row_bytes * frame.extent[1] as usize);
        for (to, from) in pixels
            .chunks_exact_mut(row_bytes)
            .zip(frame.data.chunks_exact(frame.stride as usize))
        {
            to.copy_from_slice(&from[..row_bytes]);
        }
        recorder.try_push(
            &RgbaFrame {
                extent: frame.extent,
                data: pixels,
            },
            pts,
            None,
        );
    }
    pub fn after_submit(&mut self, device: &wgpu::Device) {
        let Some(recorder) = &mut self.recorder else {
            return;
        };
        self.readback.poll(device, |frame| {
            Self::deliver(recorder, &self.pool, &mut self.timestamps, frame)
        });
        if let Some(oldest) = self.readback.oldest_sequence() {
            self.timestamps.retain(|sequence, _| *sequence >= oldest);
        } else {
            self.timestamps.clear();
        }
        if let Some(end) = self.stopping
            && self.readback.in_flight() == 0
            && !self.writer_stopped
        {
            recorder.note_end(end);
            recorder.stop();
            self.writer_stopped = true;
            self.timestamps.clear();
        }
    }
    pub fn stop(&mut self, now: Instant) {
        self.stopping.get_or_insert_with(|| {
            media_time(now.saturating_duration_since(self.started).as_secs_f64())
        });
    }
    pub fn status(&self, now: Instant) -> RecordingStatus {
        RecordingStatus {
            active: true,
            finalizing: self.stopping.is_some(),
            elapsed: self.stopping.map_or_else(
                || now.saturating_duration_since(self.started).as_secs_f64(),
                MediaTime::as_seconds,
            ),
            dropped: self.cadence_dropped
                + self.readback.skipped()
                + self.readback.failed()
                + self
                    .recorder
                    .as_ref()
                    .map_or(0, CameraRecorder::dropped_frames),
            message: String::new(),
            completed: false,
        }
    }
    pub fn try_finish(&mut self) -> Option<Result<PathBuf, String>> {
        let result = self.recorder.as_mut()?.try_finish()?;
        self.recorder.take();
        Some(self.publish(result))
    }
    fn publish(&mut self, result: CameraRecordingResult) -> Result<PathBuf, String> {
        result.result?;
        if let Err(error) = std::fs::hard_link(&result.path, &self.destination) {
            self.keep_staging = true;
            return Err(format!(
                "Could not publish recording: {error}. Completed movie kept at {}",
                result.path.display()
            ));
        }
        Ok(self.destination.clone())
    }
    pub fn finish(mut self, device: &wgpu::Device) -> Result<PathBuf, String> {
        self.stop(Instant::now());
        let mut recorder = self.recorder.take().ok_or("Recording already ended")?;
        self.readback.flush(device, |frame| {
            Self::deliver(&mut recorder, &self.pool, &mut self.timestamps, frame)
        });
        if let Some(end) = self.stopping {
            recorder.note_end(end);
        }
        let result = recorder.finish().ok_or("Recording worker stopped")?;
        self.publish(result)
    }
}
impl Drop for ProgramRecording {
    fn drop(&mut self) {
        // The worker must close the file before cleaning its private workspace.
        self.recorder.take();
        if !self.keep_staging {
            let _ = std::fs::remove_dir_all(&self.staging);
        }
    }
}
impl State {
    pub(crate) fn start_program_recording(&mut self, now: Instant) {
        if self.program_recording.is_some() {
            return;
        }
        let destination = if self.ui.record_path.trim().is_empty() {
            self.workspace
                .join("recordings")
                .join(format!("program-{}.mov", virtual_io::new_project_id()))
        } else {
            let path = PathBuf::from(self.ui.record_path.trim());
            if path.is_absolute() {
                path
            } else {
                self.workspace.join(path)
            }
        };
        match ProgramRecording::start(
            &self.gpu.device,
            &self.program,
            destination,
            self.ui.record_extent,
            self.ui.record_fps,
            now,
        ) {
            Ok(recording) => {
                self.program_recording = Some(recording);
                self.recording_status = "Recording processed PROGRAM · silent RGBA MOV".into();
            }
            Err(error) => self.recording_status = error,
        }
    }
    pub(crate) fn poll_program_recording(&mut self, now: Instant) {
        let summary = self
            .program_recording
            .as_ref()
            .map(|recording| recording.status(now));
        if let Some(result) = self
            .program_recording
            .as_mut()
            .and_then(ProgramRecording::try_finish)
        {
            self.program_recording = None;
            self.recording_summary = summary.unwrap_or_default();
            self.recording_summary.active = false;
            self.recording_summary.finalizing = false;
            match result {
                Ok(path) => {
                    self.recording_status = format!("Recorded {}", path.display());
                    self.recorded_program = Some(path);
                }
                Err(error) => self.recording_status = format!("Program recording failed: {error}"),
            }
        }
        self.ui.recording = self.program_recording.as_ref().map_or_else(
            || RecordingStatus {
                message: self.recording_status.clone(),
                completed: self.recorded_program.is_some(),
                ..self.recording_summary.clone()
            },
            |recording| {
                let mut status = recording.status(now);
                status.message = self.recording_status.clone();
                status
            },
        );
    }
    pub(crate) fn finish_program_recording(&mut self) {
        if let Some(recording) = self.program_recording.take() {
            match recording.finish(&self.gpu.device) {
                Ok(path) => self.recorded_program = Some(path),
                Err(error) => log::error!("finish program recording: {error}"),
            }
        }
    }
    pub(crate) fn add_program_recording_to_clips(&mut self) {
        let Some(path) = self.recorded_program.clone() else {
            return;
        };
        let start = virtual_media::ClipAddress {
            deck: self.mixer.selected(),
            slot: self.clips.selected(self.mixer.selected()),
        };
        let Some(address) = library_slot(&self.clips, &self.launches, start, |address| {
            self.clip_move_blocked(address)
        }) else {
            self.recording_status = "Choose a free clip slot to add this recording".into();
            return;
        };
        let request = virtual_media::ClipRestoreRequest {
            address,
            path: path.clone(),
            project_epoch: self.project_epoch,
        };
        if self.restorer.submit(request).is_err() {
            self.recording_status =
                "Media probe queue is busy; add the recording again when ready".into();
            return;
        }
        // Library import must never replace a live deck or resume a historical
        // project-open launch at an address that has since been cleared.
        if self.restore_active[address.deck.index()] == Some(address.slot) {
            self.restore_active[address.deck.index()] = None;
            self.restore_transport[address.deck.index()] = None;
        }
        self.clips.begin_restore(address, path);
        self.recording_pending.insert(address);
        self.clips.select(address);
        self.mixer.select(address.deck);
        self.recording_status = format!(
            "Adding recording to {}{} · PROGRAM keeps playing",
            address.deck.label(),
            address.slot + 1
        );
    }
}

fn library_slot(
    clips: &virtual_media::ClipBank,
    launches: &virtual_media::LaunchQueue,
    start: virtual_media::ClipAddress,
    pending: impl Fn(virtual_media::ClipAddress) -> bool,
) -> Option<virtual_media::ClipAddress> {
    clips
        .available_slots_from(start, 32)
        .into_iter()
        .find(|address| {
            !pending(*address)
                && !launches.queued(*address)
                && clips.active(address.deck) != Some(address.slot)
                && clips
                    .active_automation(address.deck)
                    .is_none_or(|(slot, _)| slot != address.slot)
                && clips
                    .automation(*address)
                    .is_none_or(|automation| automation.lanes.is_empty())
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use virtual_media::{FfmpegVideoDecoder, VideoFramePayload, probe_movie};
    use virtual_render::{FourDeckCompositor, MixerParams};

    #[test]
    fn adding_a_recording_skips_live_queued_importing_and_automation_slots() {
        use virtual_core::{ClipAutomationLane, ControlTarget, Quantization, TempoClock};
        use virtual_media::{ClipAddress, ClipBank, DeckId, LaunchQueue};
        let mut clips = ClipBank::default();
        let address = |slot| ClipAddress {
            deck: DeckId::A,
            slot,
        };
        clips.launch_automation(address(0), 0.0, true);
        let mut launches = LaunchQueue::default();
        launches.queue(address(1), Quantization::Bar, TempoClock::default(), 0.0);
        clips
            .automation_mut(address(3))
            .unwrap()
            .lanes
            .push(ClipAutomationLane::flat(ControlTarget::MasterOpacity, 0.5));
        assert_eq!(
            library_slot(&clips, &launches, address(0), |target| target == address(2)),
            Some(address(4))
        );
        assert!(launches.queued(address(1)));
        assert_eq!(clips.active_automation(DeckId::A), Some((0, 0.0)));
    }

    #[test]
    fn captures_native_effect_output_and_preserves_a_dropped_tail() {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter =
            pollster::block_on(instance.request_adapter(&Default::default())).expect("GPU adapter");
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let directory = std::env::temp_dir().join(format!(
            "virtual-program-record-{}",
            virtual_io::new_project_id()
        ));
        std::fs::create_dir(&directory).unwrap();
        let destination = directory.join("finished.mov");
        let program = ProgramTarget::new(&device, [64, 36]);
        let mut compositor = FourDeckCompositor::new(&device, &queue, PROGRAM_FORMAT);
        compositor.set_output_extent(&device, [64, 36]);
        let source = [20, 90, 180, 255];
        compositor
            .upload(
                &device,
                &queue,
                0,
                &VideoFramePayload::Rgba8(RgbaFrame {
                    extent: [4, 4],
                    data: source.repeat(16).into(),
                }),
            )
            .unwrap();
        let mut params = MixerParams {
            levels: [1.0, 0.0, 0.0, 0.0],
            output_aspect: 64.0 / 36.0,
            ..Default::default()
        };
        params.effects[0].blacklight = 1.0;
        let now = Instant::now();
        let mut recording =
            ProgramRecording::start(&device, &program, destination.clone(), [1280, 720], 30, now)
                .unwrap();
        let staging = recording.staging.clone();
        let mut expected = None;
        for offset in [0, 100] {
            let mut encoder = device.create_command_encoder(&Default::default());
            compositor.draw(&device, &queue, &mut encoder, &program.view, params);
            let mut reference = ProgramReadback::new(&device, [64, 36]);
            reference.capture(&mut encoder, program.texture());
            recording.capture(
                &device,
                &queue,
                &mut encoder,
                &program,
                now + Duration::from_millis(offset),
            );
            queue.submit([encoder.finish()]);
            reference.flush(&device, |frame| expected = Some(frame.data[..4].to_vec()));
            recording.after_submit(&device);
            assert!(recording.timestamps.len() <= 3);
        }
        assert_eq!(
            recording.status(now).dropped,
            2,
            "missed capture cadence is visible"
        );
        recording.stop(now + Duration::from_millis(200));
        assert_eq!(recording.finish(&device).unwrap(), destination);
        assert!(!staging.exists(), "finalized workspaces are cleaned up");
        let metadata = probe_movie(&destination).unwrap();
        assert_eq!(metadata.visible_extent, [1280, 720]);
        assert!((metadata.duration.unwrap().as_seconds() - 0.2).abs() < 0.034);
        let mut decoder = FfmpegVideoDecoder::open(&destination).unwrap();
        let pixels = decoder.next_frame().unwrap().unwrap().pixels;
        let at = (360 * 1280 + 640) * 4;
        let actual = &pixels.data[at..at + 4];
        let expected = expected.unwrap();
        assert_ne!(
            &expected[..3],
            &source[..3],
            "fixture must exercise native FX"
        );
        for channel in 0..4 {
            assert!(
                actual[channel].abs_diff(expected[channel]) <= 2,
                "recording {actual:?} != program {expected:?}"
            );
        }
        assert!(
            ProgramRecording::start(&device, &program, destination.clone(), [1280, 720], 30, now)
                .is_err(),
            "existing recordings stay intact"
        );
        drop(decoder);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
