//! Frame-based planned-show edits. Independent of UI, media decoding and wall time.
use serde::{Deserialize, Serialize};

pub const MAX_SEQUENCE_CLIPS: usize = 512;
pub const MAX_SEQUENCE_MARKERS: usize = 128;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SequenceClip {
    pub id: u64,
    pub track: u8,
    pub slot: u8,
    pub name: String,
    pub start: u64,
    pub duration: u64,
    pub source_in: u64,
}

impl SequenceClip {
    pub fn end(&self) -> u64 {
        self.start.saturating_add(self.duration)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SequenceMarker {
    pub frame: u64,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShowSequence {
    pub name: String,
    pub fps: u32,
    pub clips: Vec<SequenceClip>,
    pub markers: Vec<SequenceMarker>,
}

impl Default for ShowSequence {
    fn default() -> Self {
        Self {
            name: "Main show".into(),
            fps: 30,
            clips: Vec::new(),
            markers: Vec::new(),
        }
    }
}

impl ShowSequence {
    pub fn duration(&self) -> u64 {
        self.clips.iter().map(SequenceClip::end).max().unwrap_or(0)
    }

    pub fn at(&self, track: u8, frame: u64) -> Option<&SequenceClip> {
        self.clips
            .iter()
            .find(|c| c.track == track && c.start <= frame && frame < c.end())
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if ![24, 25, 30, 50, 60].contains(&self.fps) || self.name.len() > 128 {
            return Err("Invalid sequence name or frame rate");
        }
        if self.clips.len() > MAX_SEQUENCE_CLIPS || self.markers.len() > MAX_SEQUENCE_MARKERS {
            return Err("Sequence capacity exceeded");
        }
        let limit = u64::from(self.fps) * 21_600;
        for (i, clip) in self.clips.iter().enumerate() {
            if clip.id == 0
                || clip.track >= 4
                || clip.slot >= 8
                || clip.name.len() > 128
                || clip.duration == 0
                || clip.end() > limit
                || clip.source_in.saturating_add(clip.duration) > limit
            {
                return Err("Invalid sequence clip");
            }
            for other in &self.clips[..i] {
                if clip.id == other.id {
                    return Err("Duplicate sequence clip identity");
                }
                if clip.track == other.track && clip.start < other.end() && other.start < clip.end()
                {
                    return Err("Clips overlap on the same track");
                }
            }
        }
        if self
            .markers
            .iter()
            .any(|m| m.frame > limit || m.name.len() > 128)
        {
            return Err("Invalid sequence marker");
        }
        Ok(())
    }

    pub fn next_id(&self) -> u64 {
        (1..)
            .find(|id| self.clips.iter().all(|c| c.id != *id))
            .unwrap_or(1)
    }

    /// Non-ripple split: both pieces retain source continuity and the original end.
    pub fn split(&mut self, id: u64, frame: u64) -> bool {
        if self.clips.len() >= MAX_SEQUENCE_CLIPS {
            return false;
        }
        let next = self.next_id();
        let Some(clip) = self.clips.iter_mut().find(|c| c.id == id) else {
            return false;
        };
        if frame <= clip.start || frame >= clip.end() {
            return false;
        }
        let offset = frame - clip.start;
        let mut right = clip.clone();
        right.id = next;
        right.start = frame;
        right.source_in += offset;
        right.duration -= offset;
        clip.duration = offset;
        self.clips.push(right);
        true
    }
}

pub fn timecode(frame: u64, fps: u32) -> String {
    let fps = u64::from(fps.max(1));
    format!(
        "{:02}:{:02}:{:02}:{:02}",
        frame / fps / 3600,
        frame / fps / 60 % 60,
        frame / fps % 60,
        frame % fps
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn clip(id: u64, start: u64, duration: u64) -> SequenceClip {
        SequenceClip {
            id,
            track: 0,
            slot: 0,
            name: "clip".into(),
            start,
            duration,
            source_in: 12,
        }
    }
    #[test]
    fn split_preserves_source_continuity_and_half_open_boundaries() {
        let mut s = ShowSequence::default();
        s.clips.push(clip(1, 30, 90));
        assert!(s.split(1, 60));
        assert_eq!(s.at(0, 59).unwrap().id, 1);
        assert_eq!(s.at(0, 60).unwrap().source_in, 42);
        assert!(s.at(0, 120).is_none());
        assert_eq!(s.duration(), 120);
        assert!(s.validate().is_ok());
        assert!(!s.split(1, 30));
    }
    #[test]
    fn rejects_overlap_duplicate_identity_overflow_and_invalid_track() {
        for other in [
            clip(2, 29, 10),
            clip(1, 30, 10),
            clip(2, u64::MAX, 10),
            SequenceClip {
                track: 4,
                ..clip(2, 30, 10)
            },
        ] {
            let s = ShowSequence {
                clips: vec![clip(1, 0, 30), other],
                ..ShowSequence::default()
            };
            assert!(s.validate().is_err());
        }
    }
    #[test]
    fn gaps_and_frame_timecode_are_exact() {
        let s = ShowSequence {
            clips: vec![clip(1, 30, 30)],
            ..ShowSequence::default()
        };
        assert!(s.at(0, 29).is_none());
        assert_eq!(timecode(108_031, 30), "01:00:01:01");
        assert!(s.validate().is_ok());
    }
}
