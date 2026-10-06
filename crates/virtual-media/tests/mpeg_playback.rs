//! Real long-GOP fixtures exercise B-frame ordering, container offsets and seeks.
//! Requires the FFmpeg CLI from the documented build prerequisites.
use std::{path::PathBuf, process::Command};
use virtual_core::MediaTime;
use virtual_media::{FfmpegVideoDecoder, probe_movie};

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn check_movie(extension: &str, codec: &str) {
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "virtual-mpeg-playback-{}.{extension}",
        std::process::id()
    )));
    let output = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=320x180:rate=30",
            "-t",
            "2",
            "-an",
            "-c:v",
            codec,
            "-g",
            "15",
            "-bf",
            "2",
            "-output_ts_offset",
            "10",
        ])
        .arg(&fixture.0)
        .output()
        .expect("FFmpeg CLI is a build prerequisite");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata = probe_movie(&fixture.0).unwrap();
    assert_eq!(
        metadata.keyframes.nearest_preceding(MediaTime::ZERO),
        Some(MediaTime::ZERO)
    );
    let mut decoder = FfmpegVideoDecoder::open(&fixture.0).unwrap();
    let mut times = Vec::new();
    let mut first_pixels = None;
    let mut changed = false;
    while let Some(frame) = decoder.next_frame().unwrap() {
        if let Some(first) = &first_pixels {
            changed |= first != &frame.pixels.data.to_vec();
        } else {
            first_pixels = Some(frame.pixels.data.to_vec());
        }
        times.push(frame.pts);
    }
    assert_eq!(times.len(), 60, "must drain the reordered final frames");
    assert_eq!(
        times[0],
        MediaTime::ZERO,
        "container offset must not hold the launch frame"
    );
    assert!(changed, "the decoded image must advance");
    for pair in times.windows(2) {
        assert!(
            pair[1] > pair[0],
            "B frames must be presented in timestamp order"
        );
    }
    for target in [
        MediaTime::ZERO,
        MediaTime::new(1, 2).unwrap(),
        MediaTime::new(1, 1).unwrap(),
        MediaTime::new(19, 10).unwrap(),
    ] {
        let keyframe = metadata.keyframes.nearest_preceding(target).unwrap();
        let mut seeked = FfmpegVideoDecoder::open_at(&fixture.0, Some(keyframe)).unwrap();
        let frame = loop {
            let frame = seeked
                .next_frame()
                .unwrap()
                .expect("seek must not skip to EOF");
            if frame.pts >= target {
                break frame;
            }
        };
        assert_eq!(frame.pts, target);
    }
}

#[test]
fn mpeg2_program_stream_advances_and_seeks_with_nonzero_start() {
    check_movie("mpg", "mpeg2video");
}

#[test]
fn mpeg2_transport_stream_advances_and_seeks_with_nonzero_start() {
    check_movie("ts", "mpeg2video");
}

#[test]
fn h264_mp4_advances_and_seeks_with_nonzero_start() {
    check_movie("mp4", "libx264");
}
