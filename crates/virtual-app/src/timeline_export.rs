//! Bounded background exports; a source-video render and a control-take edit
//! are deliberately separate artifacts.
use crate::State;
use anyhow::{Context, Result, bail};
use std::fs::{self, File};
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread;
use std::time::Duration;
use virtual_io::EditSegmentProject;
use virtual_session::{JournalRecord, ShowTime, TakeSlice, assemble_takes, recover_journal};

pub struct ExportJob {
    cancel: Arc<AtomicBool>,
    completion: mpsc::Receiver<Result<String, String>>,
}
impl ExportJob {
    fn start(work: impl FnOnce(&AtomicBool) -> Result<String> + Send + 'static) -> Result<Self> {
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let (sender, completion) = mpsc::sync_channel(1);
        thread::Builder::new()
            .name("virtual-timeline-export".into())
            .spawn(move || {
                let result = work(&worker_cancel).map_err(|error| format!("{error:#}"));
                let _ = sender.send(result);
            })
            .context("start export worker")?;
        Ok(Self { cancel, completion })
    }
    pub fn poll(&self) -> Option<Result<String, String>> {
        match self.completion.try_recv() {
            Ok(result) => Some(result),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(Err("Export worker stopped".into())),
        }
    }
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
impl Drop for ExportJob {
    fn drop(&mut self) {
        self.cancel();
    }
}

impl State {
    pub(crate) fn export_timeline_video(&mut self) {
        if self.ui.timeline.job.is_some() {
            return;
        }
        let Some(path) = rfd::FileDialog::new()
            .set_file_name("video-edit.mp4")
            .add_filter("H.264 video", &["mp4"])
            .save_file()
        else {
            return;
        };
        let segments = self.ui.timeline.project.videos.clone();
        let extent = self.ui.composition_extent;
        match ExportJob::start(move |cancel| render_videos(&segments, &path, extent, cancel)) {
            Ok(job) => {
                self.ui.timeline.job = Some(job);
                self.ui.timeline.status = "Rendering video cuts…".into();
            }
            Err(error) => self.ui.timeline.status = format!("Export failed: {error:#}"),
        }
    }
    pub(crate) fn export_timeline_take(&mut self) {
        if self.ui.timeline.job.is_some() {
            return;
        }
        let directory = self.workspace.join(".virtual/session");
        let Some(path) = rfd::FileDialog::new()
            .set_directory(directory)
            .set_file_name("assembled-take.jsonl")
            .add_filter("VIRTUAL take journal", &["jsonl"])
            .save_file()
        else {
            return;
        };
        let segments = self.ui.timeline.project.takes.clone();
        match ExportJob::start(move |cancel| export_take(&segments, &path, cancel)) {
            Ok(job) => {
                self.ui.timeline.job = Some(job);
                self.ui.timeline.status = "Assembling take segments…".into();
            }
            Err(error) => self.ui.timeline.status = format!("Export failed: {error:#}"),
        }
    }
}

struct Scratch(PathBuf);
impl Scratch {
    fn new(destination: &Path) -> Result<Self> {
        if destination.exists() {
            bail!("Choose a new filename; an export already exists there");
        }
        let parent = destination
            .parent()
            .context("export needs a parent directory")?;
        let path = parent.join(format!(".virtual-export-{}", virtual_io::new_project_id()));
        fs::create_dir(&path).context("create export workspace")?;
        Ok(Self(path))
    }
    fn publish(&self, staged: &Path, destination: &Path, cancel: &AtomicBool) -> Result<()> {
        check_cancel(cancel)?;
        // Same-filesystem link commits the complete artifact and refuses an
        // existing destination even if one appeared after export started.
        fs::hard_link(staged, destination).context("publish export (choose an unused filename)")?;
        Ok(())
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn check_cancel(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        bail!("Export canceled");
    }
    Ok(())
}
fn validate_segments(segments: &[EditSegmentProject]) -> Result<()> {
    if segments.is_empty()
        || segments.len() > virtual_io::MAX_TIMELINE_SEGMENTS
        || segments.iter().any(|segment| !segment.valid())
    {
        bail!("Choose between one and 128 valid segments");
    }
    for segment in segments {
        if !segment.path.is_file() {
            bail!("Missing source: {}", segment.path.display());
        }
    }
    Ok(())
}
fn ffmpeg_executable() -> PathBuf {
    if let Some(path) = std::env::var_os("VIRTUAL_FFMPEG") {
        return path.into();
    }
    for path in ["/opt/homebrew/bin/ffmpeg", "/usr/local/bin/ffmpeg"] {
        if Path::new(path).is_file() {
            return path.into();
        }
    }
    "ffmpeg".into()
}
fn run_ffmpeg(command: &mut Command, log: &Path, cancel: &AtomicBool) -> Result<()> {
    check_cancel(cancel)?;
    let stderr = File::create(log)?;
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(stderr)
        .spawn()
        .context("start FFmpeg; install the ffmpeg executable or set VIRTUAL_FFMPEG to its path")?;
    let result = loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            bail!("Export canceled");
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => thread::sleep(Duration::from_millis(100)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error).context("wait for FFmpeg");
            }
        }
    };
    if !result.success() {
        let mut file = File::open(log)?;
        let length = file.metadata()?.len();
        file.seek(SeekFrom::Start(length.saturating_sub(8192)))?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        bail!("FFmpeg failed: {}", String::from_utf8_lossy(&bytes));
    }
    Ok(())
}
fn render_videos(
    segments: &[EditSegmentProject],
    destination: &Path,
    extent: [u32; 2],
    cancel: &AtomicBool,
) -> Result<String> {
    validate_segments(segments)?;
    let [width, height] = extent.map(|value| (value.max(2) / 2) * 2);
    if width > 8192 || height > 8192 {
        bail!("Video export size must be at most 8192×8192");
    }
    let scratch = Scratch::new(destination)?;
    let executable = ffmpeg_executable();
    let log = scratch.0.join("ffmpeg.log");
    let mut list = File::create(scratch.0.join("cuts.txt"))?;
    for (index, segment) in segments.iter().enumerate() {
        let name = format!("cut-{index:03}.mp4");
        let output = scratch.0.join(&name);
        let mut command = Command::new(&executable);
        // Each cut is normalized separately: decoder count and memory usage
        // stay bounded even for a long edit with mixed codecs and resolutions.
        command.args(["-hide_banner", "-loglevel", "error", "-nostdin", "-n", "-ss"])
            .arg(format!("{:.9}", segment.in_seconds)).arg("-i").arg(&segment.path)
            .arg("-t").arg(format!("{:.9}", segment.duration()))
            .args(["-map", "0:v:0", "-an", "-vf"])
            .arg(format!("scale={width}:{height}:force_original_aspect_ratio=decrease,pad={width}:{height}:(ow-iw)/2:(oh-ih)/2,setsar=1,fps=30,format=yuv420p,setpts=PTS-STARTPTS"))
            .args(["-c:v", "libx264", "-preset", "fast", "-crf", "18", "-threads", "2"]).arg(&output);
        run_ffmpeg(&mut command, &log, cancel)?;
        // A trim past the real media end must fail before publishing a short
        // or empty edit, even if FFmpeg itself exits successfully.
        let metadata = virtual_media::probe_movie(&output).context("validate rendered cut")?;
        let actual = metadata
            .duration
            .map(|time| time.as_seconds())
            .unwrap_or(0.0);
        if (actual - segment.duration()).abs() > 0.1 {
            bail!(
                "Cut {} ends outside the available video; shorten its Out point",
                index + 1
            );
        }
        writeln!(list, "file '{name}'")?;
    }
    list.sync_all()?;
    let output = scratch.0.join("assembled.mp4");
    let mut command = Command::new(executable);
    command
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-n",
            "-f",
            "concat",
            "-safe",
            "1",
            "-i",
        ])
        .arg(scratch.0.join("cuts.txt"))
        .args(["-c", "copy", "-movflags", "+faststart"])
        .arg(&output);
    run_ffmpeg(&mut command, &log, cancel)?;
    scratch.publish(&output, destination, cancel)?;
    Ok(format!(
        "Rendered {} video cuts · {}",
        segments.len(),
        destination.display()
    ))
}

fn export_take(
    segments: &[EditSegmentProject],
    destination: &Path,
    cancel: &AtomicBool,
) -> Result<String> {
    validate_segments(segments)?;
    let scratch = Scratch::new(destination)?;
    let mut journals = Vec::with_capacity(segments.len());
    for segment in segments {
        check_cancel(cancel)?;
        journals.push(recover_journal(
            &segment.path,
            segment.path.with_extension("checkpoint.json"),
        )?);
    }
    let mut slices = Vec::with_capacity(segments.len());
    for (journal, segment) in journals.iter().zip(segments) {
        let first = journal
            .history_checkpoints
            .iter()
            .chain(journal.checkpoint.iter())
            .map(|checkpoint| checkpoint.at.monotonic_ns)
            .min()
            .unwrap_or(0);
        let latest = journal.latest_time().monotonic_ns;
        let requested_in = (segment.in_seconds * 1e9).round() as u64;
        let requested_out = (segment.out_seconds * 1e9).round() as u64;
        // Floating-point UI conversion can move an endpoint by a nanosecond.
        // A materially changed source must be reselected, not silently shortened.
        if requested_in.saturating_add(1000) < first || requested_out > latest.saturating_add(1000)
        {
            bail!("Take source range changed; reselect {}", segment.name);
        }
        slices.push(TakeSlice {
            journal,
            in_ns: requested_in.max(first),
            out_ns: requested_out.min(latest),
        });
    }
    let name = destination
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("Assembled take");
    let mut edited = assemble_takes(name, &slices)?;
    edited.take_id = Some(virtual_io::new_project_id());
    let output = scratch.0.join("edited.jsonl");
    let mut writer = BufWriter::new(File::create(&output)?);
    let mut write = |record: JournalRecord| -> Result<()> {
        check_cancel(cancel)?;
        serde_json::to_writer(&mut writer, &record)?;
        writer.write_all(b"\n")?;
        Ok(())
    };
    write(JournalRecord::Header {
        format: virtual_session::JOURNAL_FORMAT.into(),
        version: virtual_session::JOURNAL_VERSION,
        take_name: edited.take_name.clone(),
        project_id: edited.project_id.clone(),
        take_id: edited.take_id.clone(),
    })?;
    // Recovery retains the entire history and uses checkpoint times to choose
    // the baseline. Equal-time checkpoints are emitted in cut order.
    for checkpoint in &edited.history_checkpoints {
        write(JournalRecord::Checkpoint {
            checkpoint: checkpoint.clone(),
        })?;
    }
    for command in &edited.history_commands {
        write(JournalRecord::Command {
            command: command.clone(),
        })?;
    }
    for marker in &edited.markers {
        write(JournalRecord::Marker {
            marker: marker.clone(),
        })?;
    }
    writer.flush()?;
    writer.get_ref().sync_all()?;
    // Reopen the artifact using the same reader as recovery before publishing.
    let verified = recover_journal(&output, scratch.0.join("absent.checkpoint.json"))?;
    verified.replay_at(ShowTime::default())?;
    verified.replay_at(verified.latest_time())?;
    scratch.publish(&output, destination, cancel)?;
    Ok(format!(
        "Exported assembled take · {} · Scan recorded takes to open it",
        destination.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use virtual_session::{
        CommandOperation, CommandOrigin, JournalWriter, SessionState, ShowCommand, StateCheckpoint,
        TimelineMarker,
    };

    #[test]
    fn assembled_take_round_trips_without_sidecar_and_keeps_cut_baselines() {
        let directory = std::env::temp_dir().join(format!(
            "virtual-take-edit-{}",
            virtual_io::new_project_id()
        ));
        fs::create_dir(&directory).unwrap();
        let source = directory.join("source.jsonl");
        let writer = JournalWriter::open_linked(
            &source,
            source.with_extension("checkpoint.json"),
            "Original",
            Some("a".repeat(32)),
            Some("b".repeat(32)),
            16,
        )
        .unwrap();
        writer
            .try_checkpoint(StateCheckpoint {
                after_sequence: None,
                at: ShowTime::default(),
                state: SessionState {
                    bpm: 130.0,
                    ..Default::default()
                },
            })
            .unwrap();
        writer
            .try_append(ShowCommand {
                sequence: 0,
                command_id: 0,
                origin: CommandOrigin::Operator,
                execute_at: ShowTime {
                    monotonic_ns: 1_000_000_000,
                    ..Default::default()
                },
                operation: CommandOperation::SetBlackout { enabled: true },
            })
            .unwrap();
        writer
            .try_marker(TimelineMarker {
                at: ShowTime {
                    monotonic_ns: 2_000_000_000,
                    ..Default::default()
                },
                label: "End".into(),
            })
            .unwrap();
        writer.flush().unwrap();
        drop(writer);
        let segments = vec![
            EditSegmentProject {
                path: source.clone(),
                name: "Tail".into(),
                in_seconds: 1.0,
                out_seconds: 2.0,
                source_start: 0.0,
                source_duration: 2.0,
            },
            EditSegmentProject {
                path: source.clone(),
                name: "Full".into(),
                in_seconds: 0.0,
                out_seconds: 2.0,
                source_start: 0.0,
                source_duration: 2.0,
            },
        ];
        let destination = directory.join("assembled.jsonl");
        export_take(&segments, &destination, &AtomicBool::new(false)).unwrap();
        let result =
            recover_journal(&destination, destination.with_extension("checkpoint.json")).unwrap();
        let replay = |ns| {
            result
                .replay_at(ShowTime {
                    monotonic_ns: ns,
                    ..Default::default()
                })
                .unwrap()
        };
        assert!(replay(0).blackout);
        assert!(!replay(1_000_000_000).blackout);
        assert!(result.replay_state().unwrap().blackout);
        assert_eq!(result.replay_state().unwrap().bpm, 130.0);
        assert_eq!(result.latest_time().monotonic_ns, 3_000_000_000);
        assert!(source.exists());
        assert!(!destination.with_extension("checkpoint.json").exists());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    #[ignore = "requires the FFmpeg executable with libx264 and lavfi"]
    fn video_render_preserves_cut_order_duration_and_mixed_source_sizes() {
        let directory = std::env::temp_dir().join(format!(
            "virtual-video-edit-{}",
            virtual_io::new_project_id()
        ));
        fs::create_dir(&directory).unwrap();
        let mut segments = Vec::new();
        for (color, size) in [("red", "96x64"), ("blue", "160x90")] {
            let path = directory.join(format!("{color} ' footage.mp4"));
            assert!(
                Command::new(ffmpeg_executable())
                    .args(["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i"])
                    .arg(format!("color=c={color}:s={size}:r=30"))
                    .args(["-t", "0.4", "-c:v", "libx264", "-pix_fmt", "yuv420p"])
                    .arg(&path)
                    .status()
                    .unwrap()
                    .success()
            );
            segments.push(EditSegmentProject {
                path,
                name: color.into(),
                in_seconds: 0.1,
                out_seconds: 0.3,
                source_start: 0.0,
                source_duration: 0.4,
            });
        }
        let destination = directory.join("render.mp4");
        render_videos(&segments, &destination, [160, 90], &AtomicBool::new(false)).unwrap();
        let metadata = virtual_media::probe_movie(&destination).unwrap();
        assert_eq!(metadata.visible_extent, [160, 90]);
        assert!((metadata.duration.unwrap().as_seconds() - 0.4).abs() < 0.04);
        let sample = |at: &str| {
            let output = Command::new(ffmpeg_executable())
                .args(["-hide_banner", "-loglevel", "error", "-ss", at, "-i"])
                .arg(&destination)
                .args([
                    "-vf",
                    "crop=2:2:(iw-2)/2:(ih-2)/2,scale=1:1",
                    "-frames:v",
                    "1",
                    "-f",
                    "rawvideo",
                    "-pix_fmt",
                    "rgb24",
                    "pipe:1",
                ])
                .output()
                .unwrap();
            assert!(output.status.success());
            assert_eq!(output.stdout.len(), 3);
            output.stdout
        };
        let first = sample("0.05");
        let second = sample("0.25");
        assert!(
            first[0] > 200 && first[2] < 30,
            "first cut must be red: {first:?}"
        );
        assert!(
            second[2] > 200 && second[0] < 30,
            "second cut must be blue: {second:?}"
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn canceled_export_never_creates_a_destination() {
        let directory = std::env::temp_dir().join(format!(
            "virtual-export-test-{}",
            virtual_io::new_project_id()
        ));
        fs::create_dir(&directory).unwrap();
        let destination = directory.join("edit.mp4");
        let scratch = Scratch::new(&destination).unwrap();
        let staged = scratch.0.join("staged");
        fs::write(&staged, "complete").unwrap();
        assert!(
            scratch
                .publish(&staged, &destination, &AtomicBool::new(true))
                .is_err()
        );
        assert!(!destination.exists());
        fs::write(&destination, "original").unwrap();
        assert!(
            scratch
                .publish(&staged, &destination, &AtomicBool::new(false))
                .is_err()
        );
        assert_eq!(fs::read_to_string(&destination).unwrap(), "original");
        drop(scratch);
        fs::remove_dir_all(directory).unwrap();
    }
}
