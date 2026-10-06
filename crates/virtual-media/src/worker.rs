//! Bounded decoder actor used by each active mixer deck.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use virtual_core::MediaTime;
use virtual_generate::{AudioBands, Generator, GeneratorSettings, GeneratorStats};
use virtual_hap::Decoder as HapDecoder;

use crate::capture_interrupt::CaptureCancellation;

use crate::{
    CameraConfig, DecodePath, FfmpegVideoDecoder, FrameBufferPool, FramePoolStats, HapDemuxer,
    RgbaFrame, ScheduledFrame, VideoFramePayload,
};

#[derive(Debug)]
enum DecoderCommand {
    Load {
        path: PathBuf,
        decode_path: DecodePath,
        generation: u64,
        start_at: Option<MediaTime>,
        seek_to: Option<MediaTime>,
    },
    Camera {
        cancellation: CaptureCancellation,
        config: CameraConfig,
        generation: u64,
    },
    Generator {
        settings: GeneratorSettings,
        generation: u64,
        link: Arc<GeneratorLink>,
    },
    GeneratorSettings(GeneratorSettings),
    Stop,
    Shutdown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecoderEvent {
    Loaded { generation: u64 },
    Ended { generation: u64 },
    Error { generation: u64, message: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecoderFailureInjection {
    after_decoded_frames: u64,
    message: String,
}

impl DecoderFailureInjection {
    pub fn after_frames(after_decoded_frames: u64, message: impl Into<String>) -> Self {
        Self {
            after_decoded_frames,
            message: message.into(),
        }
    }
}

struct ArmedDecoderFailure {
    injection: DecoderFailureInjection,
    generation: Option<u64>,
    decoded_frames: u64,
}

impl ArmedDecoderFailure {
    fn new(injection: DecoderFailureInjection) -> Self {
        Self {
            injection,
            generation: None,
            decoded_frames: 0,
        }
    }

    fn arm(&mut self, generation: u64) {
        if self.generation.is_none() {
            self.generation = Some(generation);
        }
    }

    fn record_frame(&mut self, generation: u64) {
        if self.generation == Some(generation) {
            self.decoded_frames = self.decoded_frames.saturating_add(1);
        }
    }

    fn message_if_due(&self, generation: u64) -> Option<String> {
        (self.generation == Some(generation)
            && self.decoded_frames >= self.injection.after_decoded_frames)
            .then(|| self.injection.message.clone())
    }
}

/// Lock-free exchange between the render thread and a generator session:
/// live audio bands in, last-frame statistics out. Neither side waits.
#[derive(Debug, Default)]
struct GeneratorLink {
    audio: [AtomicU32; 3],
    segments: AtomicU32,
    drawn: AtomicU32,
    depths: AtomicU32,
    flags: AtomicU32,
    generate_micros: AtomicU32,
    render_micros: AtomicU32,
}

impl GeneratorLink {
    fn set_audio(&self, audio: AudioBands) {
        for (slot, value) in self.audio.iter().zip([audio.bass, audio.mid, audio.high]) {
            slot.store(value.to_bits(), Ordering::Relaxed);
        }
    }

    fn audio(&self) -> AudioBands {
        let band = |index: usize| f32::from_bits(self.audio[index].load(Ordering::Relaxed));
        AudioBands {
            bass: band(0),
            mid: band(1),
            high: band(2),
        }
    }

    fn publish(&self, stats: GeneratorStats) {
        self.segments.store(stats.segments, Ordering::Relaxed);
        self.drawn.store(stats.drawn, Ordering::Relaxed);
        self.depths.store(
            (stats.requested_depth.min(0xFFFF) << 16) | stats.depth.min(0xFFFF),
            Ordering::Relaxed,
        );
        self.flags.store(
            u32::from(stats.truncated) | (u32::from(stats.planar) << 1),
            Ordering::Relaxed,
        );
        self.generate_micros
            .store(stats.generate_micros, Ordering::Relaxed);
        self.render_micros
            .store(stats.render_micros, Ordering::Relaxed);
    }

    fn stats(&self) -> GeneratorStats {
        let depths = self.depths.load(Ordering::Relaxed);
        let flags = self.flags.load(Ordering::Relaxed);
        GeneratorStats {
            segments: self.segments.load(Ordering::Relaxed),
            drawn: self.drawn.load(Ordering::Relaxed),
            requested_depth: depths >> 16,
            depth: depths & 0xFFFF,
            truncated: flags & 1 != 0,
            planar: flags & 2 != 0,
            generate_micros: self.generate_micros.load(Ordering::Relaxed),
            render_micros: self.render_micros.load(Ordering::Relaxed),
        }
    }
}

pub struct DeckDecoder {
    capture_cancellation: CaptureCancellation,
    generator_link: Arc<GeneratorLink>,
    commands: mpsc::Sender<DecoderCommand>,
    frames: Receiver<ScheduledFrame<VideoFramePayload>>,
    events: Receiver<DecoderEvent>,
    frame_pool: FrameBufferPool,
    worker: Option<JoinHandle<()>>,
}

impl DeckDecoder {
    pub fn spawn(frame_capacity: usize) -> Self {
        Self::spawn_internal(frame_capacity, None)
    }

    /// Creates a worker with a deterministic, one-shot decode failure.
    ///
    /// This is intended for failure-recovery and soak tests. The fault arms
    /// against the first successfully opened generation and is consumed once
    /// its decoded-frame threshold is reached.
    pub fn spawn_with_failure(frame_capacity: usize, failure: DecoderFailureInjection) -> Self {
        Self::spawn_internal(frame_capacity, Some(failure))
    }

    fn spawn_internal(frame_capacity: usize, failure: Option<DecoderFailureInjection>) -> Self {
        let (commands_tx, commands_rx) = mpsc::channel();
        let (frames_tx, frames_rx) = mpsc::sync_channel(frame_capacity.max(1));
        let (events_tx, events_rx) = mpsc::channel();
        let frame_pool = FrameBufferPool::new(frame_capacity.saturating_add(4));
        let worker_pool = frame_pool.clone();
        let worker = thread::Builder::new()
            .name("virtual-deck-decoder".to_owned())
            .spawn(move || {
                decoder_loop(
                    commands_rx,
                    frames_tx,
                    events_tx,
                    worker_pool,
                    failure.map(ArmedDecoderFailure::new),
                )
            })
            .expect("spawn deck decoder");
        Self {
            capture_cancellation: CaptureCancellation::default(),
            generator_link: Arc::default(),
            commands: commands_tx,
            frames: frames_rx,
            events: events_rx,
            frame_pool,
            worker: Some(worker),
        }
    }

    pub fn load(&self, path: PathBuf, decode_path: DecodePath, generation: u64) {
        self.load_at(path, decode_path, generation, None);
    }

    /// Reopens the source and discards decoded frames before `start_at`.
    ///
    /// Reopening is intentionally performed on the worker. It provides a
    /// codec-independent seek path while container-specific random access is
    /// introduced, without ever blocking the render thread.
    pub fn load_at(
        &self,
        path: PathBuf,
        decode_path: DecodePath,
        generation: u64,
        start_at: Option<MediaTime>,
    ) {
        self.load_indexed(path, decode_path, generation, start_at, None);
    }

    pub fn load_indexed(
        &self,
        path: PathBuf,
        decode_path: DecodePath,
        generation: u64,
        start_at: Option<MediaTime>,
        seek_to: Option<MediaTime>,
    ) {
        let _ = self.capture_cancellation.next();
        let _ = self.commands.send(DecoderCommand::Load {
            path,
            decode_path,
            generation,
            start_at,
            seek_to,
        });
    }

    pub fn stop(&self) {
        let _ = self.capture_cancellation.next();
        let _ = self.commands.send(DecoderCommand::Stop);
    }

    pub fn connect_camera(&self, config: CameraConfig, generation: u64) {
        let cancellation = self.capture_cancellation.next();
        let _ = self.commands.send(DecoderCommand::Camera {
            config,
            generation,
            cancellation,
        });
    }

    /// Starts a procedural source. Frames are rendered on this deck's worker
    /// at the settings' frame rate and dropped, like camera frames, when the
    /// render loop falls behind.
    pub fn connect_generator(&self, settings: GeneratorSettings, generation: u64) {
        let _ = self.capture_cancellation.next();
        self.generator_link.publish(GeneratorStats::default());
        let _ = self.commands.send(DecoderCommand::Generator {
            settings,
            generation,
            link: self.generator_link.clone(),
        });
    }

    /// Applies live setting changes without restarting the generator, so
    /// rotation, growth and trails continue smoothly.
    pub fn update_generator(&self, settings: GeneratorSettings) {
        let _ = self
            .commands
            .send(DecoderCommand::GeneratorSettings(settings));
    }

    pub fn set_generator_audio(&self, audio: AudioBands) {
        self.generator_link.set_audio(audio);
    }

    pub fn generator_stats(&self) -> GeneratorStats {
        self.generator_link.stats()
    }

    pub fn try_frame(&self) -> Result<ScheduledFrame<VideoFramePayload>, TryRecvError> {
        self.frames.try_recv()
    }

    pub fn try_event(&self) -> Result<DecoderEvent, TryRecvError> {
        self.events.try_recv()
    }

    pub fn recv_frame_timeout(
        &self,
        timeout: Duration,
    ) -> Result<ScheduledFrame<VideoFramePayload>, mpsc::RecvTimeoutError> {
        self.frames.recv_timeout(timeout)
    }

    pub fn recv_event_timeout(
        &self,
        timeout: Duration,
    ) -> Result<DecoderEvent, mpsc::RecvTimeoutError> {
        self.events.recv_timeout(timeout)
    }

    pub fn frame_pool_stats(&self) -> FramePoolStats {
        self.frame_pool.stats()
    }
}

impl Drop for DeckDecoder {
    fn drop(&mut self) {
        let _ = self.capture_cancellation.next();
        let _ = self.commands.send(DecoderCommand::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

enum Session {
    Hap {
        demuxer: HapDemuxer,
        decoder: HapDecoder,
        generation: u64,
        skip_before: Option<MediaTime>,
    },
    Ffmpeg {
        decoder: FfmpegVideoDecoder,
        generation: u64,
        skip_before: Option<MediaTime>,
    },
    Generator {
        generator: Box<Generator>,
        generation: u64,
        sequence: u64,
        /// Seconds of generated time; also the frame PTS.
        clock: f64,
        next_due: Instant,
        pool: FrameBufferPool,
        link: Arc<GeneratorLink>,
    },
}

impl Session {
    fn generation(&self) -> u64 {
        match self {
            Self::Hap { generation, .. }
            | Self::Ffmpeg { generation, .. }
            | Self::Generator { generation, .. } => *generation,
        }
    }

    fn next_frame(&mut self) -> Result<Option<ScheduledFrame<VideoFramePayload>>, String> {
        match self {
            Self::Hap {
                demuxer,
                decoder,
                generation,
                skip_before,
            } => loop {
                let frame = demuxer
                    .next_scheduled(decoder, *generation)
                    .map_err(|error| error.to_string())?;
                let Some(frame) = frame else {
                    return Ok(None);
                };
                if skip_before.is_some_and(|target| frame.pts < target) {
                    continue;
                }
                *skip_before = None;
                return Ok(Some(ScheduledFrame {
                    pts: frame.pts,
                    duration: frame.duration,
                    generation: frame.generation,
                    sequence: frame.sequence,
                    payload: VideoFramePayload::BlockCompressed(frame.payload),
                }));
            },
            Self::Ffmpeg {
                decoder,
                generation,
                skip_before,
            } => loop {
                let frame = decoder.next_frame().map_err(|error| error.to_string())?;
                let Some(frame) = frame else {
                    return Ok(None);
                };
                if skip_before.is_some_and(|target| frame.pts < target) {
                    continue;
                }
                *skip_before = None;
                return Ok(Some(ScheduledFrame {
                    pts: frame.pts,
                    duration: frame.duration,
                    generation: *generation,
                    sequence: frame.sequence,
                    payload: VideoFramePayload::Rgba8(frame.pixels),
                }));
            },
            Self::Generator {
                generator,
                generation,
                sequence,
                clock,
                next_due,
                pool,
                link,
            } => {
                let now = Instant::now();
                if now.saturating_duration_since(*next_due) > Duration::from_millis(250) {
                    // Resynchronise after a stall instead of bursting frames.
                    *next_due = now;
                }
                let interval = 1.0 / f64::from(generator.settings().fps.max(1));
                let [width, height] = generator.extent();
                let mut data = pool.acquire(width as usize * height as usize * 4);
                generator.render(*clock, link.audio(), &mut data);
                link.publish(generator.stats());
                let micros = |seconds: f64| (seconds * 1_000_000.0).round() as i64;
                let pts = MediaTime::new(micros(*clock), 1_000_000).map_err(|e| e.to_string())?;
                let frame = ScheduledFrame {
                    pts,
                    duration: MediaTime::new(micros(interval), 1_000_000).ok(),
                    generation: *generation,
                    sequence: *sequence,
                    payload: VideoFramePayload::Rgba8(RgbaFrame {
                        extent: [width, height],
                        data,
                    }),
                };
                *sequence += 1;
                *clock += interval;
                *next_due += Duration::from_secs_f64(interval);
                Ok(Some(frame))
            }
        }
    }

    fn is_live(&self) -> bool {
        match self {
            Self::Ffmpeg { decoder, .. } => decoder.is_live(),
            Self::Generator { .. } => true,
            Self::Hap { .. } => false,
        }
    }

    /// Time until a paced source's next frame is due, if it is not yet.
    fn until_due(&self) -> Option<Duration> {
        match self {
            Self::Generator { next_due, .. } => {
                let wait = next_due.saturating_duration_since(Instant::now());
                (!wait.is_zero()).then_some(wait)
            }
            Self::Hap { .. } | Self::Ffmpeg { .. } => None,
        }
    }
}

fn decoder_loop(
    commands: Receiver<DecoderCommand>,
    frames: SyncSender<ScheduledFrame<VideoFramePayload>>,
    events: mpsc::Sender<DecoderEvent>,
    frame_pool: FrameBufferPool,
    mut failure: Option<ArmedDecoderFailure>,
) {
    let mut session = None;
    let mut pending = None;
    loop {
        match commands.try_recv() {
            Ok(command) => {
                if handle_command(
                    command,
                    &mut session,
                    &mut pending,
                    &events,
                    &frame_pool,
                    &mut failure,
                ) {
                    break;
                }
            }
            Err(TryRecvError::Disconnected) => break,
            Err(TryRecvError::Empty) => {}
        }

        // Paced sources wait on the command channel, so setting changes and
        // stops are handled immediately rather than after the next frame.
        if pending.is_none()
            && let Some(wait) = session.as_ref().and_then(Session::until_due)
        {
            match commands.recv_timeout(wait) {
                Ok(command) => {
                    if handle_command(
                        command,
                        &mut session,
                        &mut pending,
                        &events,
                        &frame_pool,
                        &mut failure,
                    ) {
                        break;
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            continue;
        }

        if pending.is_none()
            && let Some(active) = session.as_mut()
        {
            let generation = active.generation();
            if let Some(message) = failure
                .as_ref()
                .and_then(|failure| failure.message_if_due(generation))
            {
                failure = None;
                let _ = events.send(DecoderEvent::Error {
                    generation,
                    message,
                });
                session = None;
                continue;
            }
            match active.next_frame() {
                Ok(Some(frame)) => {
                    if let Some(failure) = failure.as_mut() {
                        failure.record_frame(generation);
                    }
                    pending = Some(frame);
                }
                Ok(None) => {
                    let generation = active.generation();
                    let _ = events.send(DecoderEvent::Ended { generation });
                    session = None;
                }
                Err(message) => {
                    let generation = active.generation();
                    let _ = events.send(DecoderEvent::Error {
                        generation,
                        message,
                    });
                    session = None;
                }
            }
        }

        if let Some(frame) = pending.take() {
            match frames.try_send(frame) {
                Ok(()) => {}
                Err(TrySendError::Full(frame)) => {
                    if session.as_ref().is_some_and(Session::is_live) {
                        // Never let a stalled render loop create seconds of
                        // camera latency. Drop capture frames until the
                        // bounded render queue has room again.
                        continue;
                    }
                    pending = Some(frame);
                    match commands.recv_timeout(Duration::from_millis(2)) {
                        Ok(command) => {
                            if handle_command(
                                command,
                                &mut session,
                                &mut pending,
                                &events,
                                &frame_pool,
                                &mut failure,
                            ) {
                                break;
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                    }
                }
                Err(TrySendError::Disconnected(_)) => break,
            }
        } else if session.is_none() {
            match commands.recv() {
                Ok(command) => {
                    if handle_command(
                        command,
                        &mut session,
                        &mut pending,
                        &events,
                        &frame_pool,
                        &mut failure,
                    ) {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    }
}

fn handle_command(
    command: DecoderCommand,
    session: &mut Option<Session>,
    pending: &mut Option<ScheduledFrame<VideoFramePayload>>,
    events: &mpsc::Sender<DecoderEvent>,
    frame_pool: &FrameBufferPool,
    failure: &mut Option<ArmedDecoderFailure>,
) -> bool {
    match command {
        DecoderCommand::Load {
            path,
            decode_path,
            generation,
            start_at,
            seek_to,
        } => {
            *pending = None;
            let opened = match decode_path {
                DecodePath::DirectHap => HapDemuxer::open(&path)
                    .map(|demuxer| Session::Hap {
                        demuxer,
                        decoder: HapDecoder::default(),
                        generation,
                        skip_before: start_at,
                    })
                    .map_err(|error| error.to_string()),
                DecodePath::FfmpegVideo => {
                    { FfmpegVideoDecoder::open_at_with_pool(&path, seek_to, frame_pool.clone()) }
                        .map(|decoder| Session::Ffmpeg {
                            decoder,
                            generation,
                            skip_before: start_at,
                        })
                        .map_err(|error| error.to_string())
                }
            };
            match opened {
                Ok(opened) => {
                    if let Some(failure) = failure.as_mut() {
                        failure.arm(generation);
                    }
                    *session = Some(opened);
                    let _ = events.send(DecoderEvent::Loaded { generation });
                }
                Err(message) => {
                    *session = None;
                    let _ = events.send(DecoderEvent::Error {
                        generation,
                        message,
                    });
                }
            }
            false
        }
        DecoderCommand::Camera {
            config,
            generation,
            cancellation,
        } => {
            *pending = None;
            *session = None;
            if cancellation.is_canceled() {
                return false;
            }
            match FfmpegVideoDecoder::open_camera_cancellable(
                &config,
                frame_pool.clone(),
                cancellation,
            ) {
                Ok(decoder) => {
                    if let Some(failure) = failure.as_mut() {
                        failure.arm(generation);
                    }
                    *session = Some(Session::Ffmpeg {
                        decoder,
                        generation,
                        skip_before: None,
                    });
                    let _ = events.send(DecoderEvent::Loaded { generation });
                }
                Err(error) => {
                    *session = None;
                    let _ = events.send(DecoderEvent::Error {
                        generation,
                        message: error.to_string(),
                    });
                }
            }
            false
        }
        DecoderCommand::Generator {
            settings,
            generation,
            link,
        } => {
            *pending = None;
            if let Some(failure) = failure.as_mut() {
                failure.arm(generation);
            }
            *session = Some(Session::Generator {
                generator: Box::new(Generator::new(settings)),
                generation,
                sequence: 0,
                clock: 0.0,
                next_due: Instant::now(),
                pool: frame_pool.clone(),
                link,
            });
            let _ = events.send(DecoderEvent::Loaded { generation });
            false
        }
        DecoderCommand::GeneratorSettings(settings) => {
            if let Some(Session::Generator { generator, .. }) = session.as_mut() {
                generator.set_settings(settings);
            }
            false
        }
        DecoderCommand::Stop => {
            *session = None;
            *pending = None;
            false
        }
        DecoderCommand::Shutdown => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small_generator() -> GeneratorSettings {
        GeneratorSettings {
            resolution: [128, 72],
            fps: 120,
            ..GeneratorSettings::default()
        }
    }

    fn next_rgba(decoder: &DeckDecoder) -> ScheduledFrame<VideoFramePayload> {
        decoder
            .recv_frame_timeout(Duration::from_secs(5))
            .expect("generator frame")
    }

    #[test]
    fn generator_streams_paced_frames_and_applies_live_settings() {
        let decoder = DeckDecoder::spawn(2);
        decoder.connect_generator(small_generator(), 9);
        assert_eq!(
            decoder.recv_event_timeout(Duration::from_secs(5)).unwrap(),
            DecoderEvent::Loaded { generation: 9 }
        );
        let first = next_rgba(&decoder);
        let second = next_rgba(&decoder);
        assert_eq!(first.generation, 9);
        assert!(second.pts > first.pts, "generated time advances");
        let VideoFramePayload::Rgba8(frame) = &first.payload else {
            panic!("generator frames are RGBA");
        };
        assert_eq!(frame.extent, [128, 72]);
        assert_eq!(frame.data.len(), 128 * 72 * 4);
        assert!(decoder.generator_stats().segments > 0);

        decoder.update_generator(GeneratorSettings {
            resolution: [64, 64],
            ..small_generator()
        });
        let resized = (0..50)
            .map(|_| next_rgba(&decoder))
            .find_map(|frame| match frame.payload {
                VideoFramePayload::Rgba8(rgba) if rgba.extent == [64, 64] => Some(frame.generation),
                _ => None,
            });
        assert_eq!(resized, Some(9), "live update keeps the generation");

        decoder.stop();
        while decoder.try_frame().is_ok() {}
        std::thread::sleep(Duration::from_millis(50));
        while decoder.try_frame().is_ok() {}
        assert!(
            decoder
                .recv_frame_timeout(Duration::from_millis(100))
                .is_err(),
            "stop ends the generator session"
        );
    }
}
