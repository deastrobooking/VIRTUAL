//! NDI® network output of the program frame.
//!
//! The NDI runtime is loaded when the output starts, from the operator's NDI
//! Tools or NDI runtime installation; VIRTUAL does not redistribute it. The
//! program frame is read back only while at least one receiver is connected,
//! and it is sent from a worker thread, so a slow network or receiver drops
//! frames instead of blocking rendering.

use std::ffi::{CString, c_char, c_int, c_void};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use virtual_render::{ProgramReadback, ProgramTarget};

pub(crate) const DEFAULT_SOURCE_NAME: &str = "VIRTUAL Program";

/// Nominal rate advertised to receivers; frames carry synthesized timecode.
const FRAME_RATE: [c_int; 2] = [60_000, 1_000];

/// How often an idle worker refreshes the receiver count.
const IDLE_POLL: Duration = Duration::from_millis(250);

const fn fourcc(code: &[u8; 4]) -> u32 {
    code[0] as u32 | (code[1] as u32) << 8 | (code[2] as u32) << 16 | (code[3] as u32) << 24
}

/// 8-bit RGB with ignored alpha: the program frame is opaque.
const FOURCC_RGBX: u32 = fourcc(b"RGBX");
const FRAME_FORMAT_PROGRESSIVE: c_int = 1;
const TIMECODE_SYNTHESIZE: i64 = i64::MAX;

/// `NDIlib_send_create_t`.
#[repr(C)]
struct SendCreate {
    p_ndi_name: *const c_char,
    p_groups: *const c_char,
    clock_video: bool,
    clock_audio: bool,
}

/// `NDIlib_video_frame_v2_t`.
#[repr(C)]
struct VideoFrameV2 {
    xres: c_int,
    yres: c_int,
    four_cc: u32,
    frame_rate_n: c_int,
    frame_rate_d: c_int,
    picture_aspect_ratio: f32,
    frame_format_type: c_int,
    timecode: i64,
    p_data: *const u8,
    line_stride_in_bytes: c_int,
    p_metadata: *const c_char,
    timestamp: i64,
}

/// One frame handed to the worker: RGBA8 rows `stride` bytes apart.
pub(crate) struct Frame {
    pub data: Vec<u8>,
    pub extent: [u32; 2],
    pub stride: u32,
}

/// Where the worker sends frames; the NDI runtime in production.
pub(crate) trait VideoSink {
    fn send(&mut self, frame: &Frame);
    fn connections(&mut self) -> u32;
}

/// Locations to try for the NDI runtime, most specific first.
fn runtime_candidates() -> Vec<PathBuf> {
    let file = if cfg!(target_os = "windows") {
        "Processing.NDI.Lib.x64.dll"
    } else if cfg!(target_os = "macos") {
        "libndi.dylib"
    } else {
        "libndi.so.6"
    };
    let mut candidates: Vec<PathBuf> = ["NDI_RUNTIME_DIR_V6", "NDI_RUNTIME_DIR_V5"]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(|dir| PathBuf::from(dir).join(file))
        .collect();
    if cfg!(target_os = "macos") {
        candidates.extend(
            [
                "/usr/local/lib/libndi.dylib",
                "/Library/NDI SDK for Apple/lib/macOS/libndi.dylib",
                "/opt/homebrew/lib/libndi.dylib",
            ]
            .map(PathBuf::from),
        );
    } else if cfg!(target_os = "linux") {
        candidates.extend(["libndi.so.5", "libndi.so"].map(PathBuf::from));
    }
    // Finally the platform's default library search path.
    candidates.push(PathBuf::from(file));
    candidates
}

type SendInstance = *mut c_void;

/// Entry points resolved from the NDI runtime library.
struct NdiRuntime {
    // Declared before `_library` so the pointers never outlive it.
    destroy: unsafe extern "C" fn(),
    send_create: unsafe extern "C" fn(*const SendCreate) -> SendInstance,
    send_destroy: unsafe extern "C" fn(SendInstance),
    send_video_v2: unsafe extern "C" fn(SendInstance, *const VideoFrameV2),
    send_get_no_connections: unsafe extern "C" fn(SendInstance, u32) -> c_int,
    _library: libloading::Library,
}

impl NdiRuntime {
    fn load() -> Result<Self, String> {
        let mut last_error = None;
        for path in runtime_candidates() {
            // SAFETY: loading the NDI runtime runs its initializers, which is
            // the documented way to use the dynamically loaded NDI library.
            match unsafe { libloading::Library::new(&path) } {
                Ok(library) => return Self::resolve(library),
                Err(error) => last_error = Some(error),
            }
        }
        Err(format!(
            "NDI runtime not found ({}). Install NDI Tools from ndi.video.",
            last_error.map_or_else(|| "no candidates".to_owned(), |error| error.to_string())
        ))
    }

    fn resolve(library: libloading::Library) -> Result<Self, String> {
        /// Copies a typed entry point out of the library.
        ///
        /// SAFETY: `T` must be the C signature the NDI SDK declares for `name`.
        unsafe fn entry<T: Copy>(library: &libloading::Library, name: &str) -> Result<T, String> {
            let symbol = format!("{name}\0");
            // SAFETY: upheld by the caller.
            unsafe { library.get::<T>(symbol.as_bytes()) }
                .map(|symbol| *symbol)
                .map_err(|error| format!("NDI runtime is missing {name}: {error}"))
        }
        // SAFETY: each type below matches the NDI SDK's C declaration.
        unsafe {
            let initialize: unsafe extern "C" fn() -> bool = entry(&library, "NDIlib_initialize")?;
            let runtime = Self {
                destroy: entry(&library, "NDIlib_destroy")?,
                send_create: entry(&library, "NDIlib_send_create")?,
                send_destroy: entry(&library, "NDIlib_send_destroy")?,
                send_video_v2: entry(&library, "NDIlib_send_send_video_v2")?,
                send_get_no_connections: entry(&library, "NDIlib_send_get_no_connections")?,
                _library: library,
            };
            if !initialize() {
                return Err("NDI runtime refused to initialize (unsupported CPU?)".to_owned());
            }
            Ok(runtime)
        }
    }
}

/// A live NDI sender. Created and used only on the worker thread.
struct NdiSink {
    runtime: NdiRuntime,
    instance: SendInstance,
    _name: CString,
}

impl NdiSink {
    fn new(name: &str) -> Result<Self, String> {
        let runtime = NdiRuntime::load()?;
        let name = CString::new(name).map_err(|_| "NDI source name contains NUL".to_owned())?;
        let settings = SendCreate {
            p_ndi_name: name.as_ptr(),
            p_groups: std::ptr::null(),
            // The render loop already paces to vsync.
            clock_video: false,
            clock_audio: false,
        };
        // SAFETY: settings and name are live for the call; NDI copies them.
        let instance = unsafe { (runtime.send_create)(&settings) };
        if instance.is_null() {
            // SAFETY: balances the successful NDIlib_initialize.
            unsafe { (runtime.destroy)() };
            return Err("NDI could not create the sender".to_owned());
        }
        Ok(Self {
            runtime,
            instance,
            _name: name,
        })
    }
}

impl VideoSink for NdiSink {
    fn send(&mut self, frame: &Frame) {
        let video = VideoFrameV2 {
            xres: frame.extent[0] as c_int,
            yres: frame.extent[1] as c_int,
            four_cc: FOURCC_RGBX,
            frame_rate_n: FRAME_RATE[0],
            frame_rate_d: FRAME_RATE[1],
            // Zero means square pixels.
            picture_aspect_ratio: 0.0,
            frame_format_type: FRAME_FORMAT_PROGRESSIVE,
            timecode: TIMECODE_SYNTHESIZE,
            p_data: frame.data.as_ptr(),
            line_stride_in_bytes: frame.stride as c_int,
            p_metadata: std::ptr::null(),
            timestamp: 0,
        };
        // SAFETY: the synchronous send copies the frame before returning, so
        // `frame.data` only has to outlive the call.
        unsafe { (self.runtime.send_video_v2)(self.instance, &video) };
    }

    fn connections(&mut self) -> u32 {
        // SAFETY: a zero timeout only reads the current count.
        let count = unsafe { (self.runtime.send_get_no_connections)(self.instance, 0) };
        count.max(0) as u32
    }
}

impl Drop for NdiSink {
    fn drop(&mut self) {
        // SAFETY: the instance came from send_create and is destroyed once,
        // before the runtime is torn down.
        unsafe {
            (self.runtime.send_destroy)(self.instance);
            (self.runtime.destroy)();
        }
    }
}

#[derive(Default)]
struct Shared {
    running: AtomicBool,
    connections: AtomicU32,
    sent: AtomicU64,
    dropped: AtomicU64,
    error: Mutex<Option<String>>,
}

struct Worker {
    name: String,
    frames: Option<SyncSender<Frame>>,
    recycled: Receiver<Vec<u8>>,
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl Worker {
    fn spawn<S: VideoSink + 'static>(
        name: String,
        open: impl FnOnce(&str) -> Result<S, String> + Send + 'static,
    ) -> Self {
        // One queued frame: a busy worker means the next frame is dropped.
        let (frames, incoming) = mpsc::sync_channel::<Frame>(1);
        let (recycle, recycled) = mpsc::sync_channel(2);
        let shared = Arc::new(Shared::default());
        let worker_shared = Arc::clone(&shared);
        let worker_name = name.clone();
        let thread = std::thread::Builder::new()
            .name("virtual-ndi".to_owned())
            .spawn(move || {
                let mut sink = match open(&worker_name) {
                    Ok(sink) => sink,
                    Err(error) => {
                        *worker_shared.error.lock().unwrap() = Some(error);
                        return;
                    }
                };
                worker_shared.running.store(true, Ordering::Release);
                loop {
                    match incoming.recv_timeout(IDLE_POLL) {
                        Ok(frame) => {
                            sink.send(&frame);
                            worker_shared.sent.fetch_add(1, Ordering::Relaxed);
                            let _ = recycle.try_send(frame.data);
                        }
                        Err(RecvTimeoutError::Timeout) => {}
                        Err(RecvTimeoutError::Disconnected) => break,
                    }
                    worker_shared
                        .connections
                        .store(sink.connections(), Ordering::Release);
                }
                worker_shared.running.store(false, Ordering::Release);
                worker_shared.connections.store(0, Ordering::Release);
            })
            .expect("spawn NDI worker");
        Self {
            name,
            frames: Some(frames),
            recycled,
            shared,
            thread: Some(thread),
        }
    }

    fn submit(&self, frame: Frame) {
        if let Some(frames) = &self.frames {
            match frames.try_send(frame) {
                Ok(()) => {}
                Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {
                    self.shared.dropped.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
    }

    /// A buffer the worker has finished with, or a new one.
    fn buffer(&self, len: usize) -> Vec<u8> {
        let mut buffer = self.recycled.try_recv().unwrap_or_default();
        buffer.clear();
        buffer.reserve(len);
        buffer
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        // Closing the channel ends the loop within one idle poll.
        self.frames = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Snapshot for the UI.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct NdiStatus {
    pub running: bool,
    pub source_name: String,
    pub connections: u32,
    pub sent: u64,
    pub dropped: u64,
    pub error: Option<String>,
}

/// The program's NDI output: worker lifecycle plus GPU readback.
#[derive(Default)]
pub(crate) struct NdiOutput {
    worker: Option<Worker>,
    readback: Option<ProgramReadback>,
}

impl NdiOutput {
    /// Starts, stops or renames the sender to match the operator's settings.
    pub fn configure(&mut self, enabled: bool, name: &str) {
        let name = name.trim();
        let name = if name.is_empty() {
            DEFAULT_SOURCE_NAME
        } else {
            name
        };
        match (&self.worker, enabled) {
            (Some(worker), true) if worker.name == name => {}
            (_, true) => {
                self.worker = None;
                self.worker = Some(Worker::spawn(name.to_owned(), NdiSink::new));
            }
            (Some(_), false) => {
                self.worker = None;
                self.readback = None;
            }
            (None, false) => {}
        }
    }

    fn sending(&self) -> bool {
        self.worker.as_ref().is_some_and(|worker| {
            worker.shared.running.load(Ordering::Acquire)
                && worker.shared.connections.load(Ordering::Acquire) > 0
        })
    }

    /// Records a copy of the finished program frame while receivers watch.
    pub fn capture(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        program: &ProgramTarget,
    ) {
        if !self.sending() {
            return;
        }
        if self
            .readback
            .as_ref()
            .is_none_or(|readback| readback.extent() != program.extent())
        {
            self.readback = Some(ProgramReadback::new(device, program.extent()));
        }
        let readback = self.readback.as_mut().expect("created above");
        if !readback.capture(encoder, program.texture())
            && let Some(worker) = &self.worker
        {
            worker.shared.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Call after the frame's submission: forwards finished readbacks.
    pub fn after_submit(&mut self, device: &wgpu::Device) {
        let (Some(readback), Some(worker)) = (self.readback.as_mut(), self.worker.as_ref()) else {
            return;
        };
        readback.poll(device, |frame| {
            let mut data = worker.buffer(frame.data.len());
            data.extend_from_slice(frame.data);
            worker.submit(Frame {
                data,
                extent: frame.extent,
                stride: frame.stride,
            });
        });
    }

    pub fn status(&self) -> NdiStatus {
        let Some(worker) = &self.worker else {
            return NdiStatus::default();
        };
        let shared = &worker.shared;
        NdiStatus {
            running: shared.running.load(Ordering::Acquire),
            source_name: worker.name.clone(),
            connections: shared.connections.load(Ordering::Acquire),
            sent: shared.sent.load(Ordering::Relaxed),
            dropped: shared.dropped.load(Ordering::Relaxed),
            error: shared.error.lock().unwrap().clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn ffi_layouts_match_the_ndi_sdk() {
        assert_eq!(FOURCC_RGBX, 0x5842_4752);
        if cfg!(target_pointer_width = "64") {
            assert_eq!(std::mem::size_of::<SendCreate>(), 24);
            assert_eq!(std::mem::size_of::<VideoFrameV2>(), 72);
            assert_eq!(std::mem::offset_of!(VideoFrameV2, timecode), 32);
            assert_eq!(std::mem::offset_of!(VideoFrameV2, p_data), 40);
            assert_eq!(std::mem::offset_of!(VideoFrameV2, p_metadata), 56);
        }
    }

    #[test]
    fn runtime_search_ends_with_the_platform_library_name() {
        let candidates = runtime_candidates();
        let last = candidates.last().unwrap().to_string_lossy().into_owned();
        assert!(last.contains("ndi") || last.contains("NDI"), "{last}");
    }

    #[derive(Clone, Default)]
    struct Probe {
        sent: Arc<Mutex<Vec<[u32; 3]>>>,
        receivers: Arc<AtomicU32>,
        busy: Arc<AtomicBool>,
        delay: Duration,
    }

    impl VideoSink for Probe {
        fn send(&mut self, frame: &Frame) {
            self.busy.store(true, Ordering::Release);
            std::thread::sleep(self.delay);
            self.sent
                .lock()
                .unwrap()
                .push([frame.extent[0], frame.extent[1], frame.stride]);
        }
        fn connections(&mut self) -> u32 {
            self.receivers.load(Ordering::Acquire)
        }
    }

    fn wait_for(mut condition: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !condition() {
            assert!(Instant::now() < deadline, "timed out");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn frame(worker: &Worker) -> Frame {
        let mut data = worker.buffer(16);
        data.resize(16, 7);
        Frame {
            data,
            extent: [2, 2],
            stride: 8,
        }
    }

    #[test]
    fn worker_sends_reports_receivers_and_recycles_buffers() {
        let probe = Probe::default();
        let opened = probe.clone();
        let worker = Worker::spawn("Test".to_owned(), move |_| Ok(opened));
        wait_for(|| worker.shared.running.load(Ordering::Acquire));
        probe.receivers.store(2, Ordering::Release);
        wait_for(|| worker.shared.connections.load(Ordering::Acquire) == 2);
        worker.submit(frame(&worker));
        wait_for(|| worker.shared.sent.load(Ordering::Relaxed) == 1);
        assert_eq!(*probe.sent.lock().unwrap(), [[2, 2, 8]]);
        wait_for(|| {
            worker
                .recycled
                .try_recv()
                .is_ok_and(|buffer| buffer.capacity() >= 16)
        });
    }

    #[test]
    fn a_busy_worker_drops_frames_instead_of_queueing_them() {
        let probe = Probe {
            delay: Duration::from_millis(200),
            ..Probe::default()
        };
        let opened = probe.clone();
        let worker = Worker::spawn("Slow".to_owned(), move |_| Ok(opened));
        wait_for(|| worker.shared.running.load(Ordering::Acquire));
        worker.submit(frame(&worker));
        wait_for(|| probe.busy.load(Ordering::Acquire));
        for _ in 0..4 {
            worker.submit(frame(&worker));
        }
        // One in the sink, one queued, three dropped.
        assert_eq!(worker.shared.dropped.load(Ordering::Relaxed), 3);
        wait_for(|| worker.shared.sent.load(Ordering::Relaxed) == 2);
    }

    #[test]
    fn a_failed_open_reports_why_and_never_runs() {
        let worker = Worker::spawn::<Probe>("Missing".to_owned(), |_| {
            Err("NDI runtime not found".to_owned())
        });
        wait_for(|| worker.shared.error.lock().unwrap().is_some());
        assert!(!worker.shared.running.load(Ordering::Acquire));
        drop(worker); // must not hang
    }

    #[test]
    fn enabling_without_a_runtime_reports_it_and_never_sends() {
        if NdiRuntime::load().is_ok() {
            eprintln!("NDI runtime installed; skipping the missing-runtime path");
            return;
        }
        let mut output = NdiOutput::default();
        output.configure(true, "  ");
        wait_for(|| output.status().error.is_some());
        let status = output.status();
        assert_eq!(
            status.source_name, DEFAULT_SOURCE_NAME,
            "blank names fall back"
        );
        assert!(!status.running);
        assert!(status.error.unwrap().contains("ndi.video"));
        assert!(!output.sending());
        output.configure(false, "");
        assert_eq!(output.status(), NdiStatus::default());
    }

    #[test]
    fn dropping_the_worker_stops_the_thread_promptly() {
        let probe = Probe::default();
        let opened = probe.clone();
        let worker = Worker::spawn("Stop".to_owned(), move |_| Ok(opened));
        wait_for(|| worker.shared.running.load(Ordering::Acquire));
        let shared = Arc::clone(&worker.shared);
        let started = Instant::now();
        drop(worker);
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(!shared.running.load(Ordering::Acquire));
    }
}
