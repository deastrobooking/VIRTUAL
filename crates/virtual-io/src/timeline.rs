//! Non-destructive sequential edits. Video cuts and control takes are distinct tracks.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const MAX_TIMELINE_SEGMENTS: usize = 128;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VideoTimelineProject {
    pub videos: Vec<EditSegmentProject>,
    pub takes: Vec<EditSegmentProject>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EditSegmentProject {
    pub path: PathBuf,
    pub name: String,
    pub in_seconds: f64,
    pub out_seconds: f64,
    #[serde(default)]
    pub source_start: f64,
    pub source_duration: f64,
}

impl EditSegmentProject {
    pub fn duration(&self) -> f64 {
        self.out_seconds - self.in_seconds
    }
    pub fn valid(&self) -> bool {
        !self.path.as_os_str().is_empty()
            && !self.name.is_empty()
            && self.name.len() <= 512
            && self.in_seconds.is_finite()
            && self.out_seconds.is_finite()
            && self.source_duration.is_finite()
            && self.source_start.is_finite()
            && self.source_start >= 0.0
            && self.in_seconds >= self.source_start
            && self.duration() >= 0.001 - 1e-9
            && self.out_seconds <= self.source_duration
            && self.source_duration <= 604_800.0
    }
}

impl VideoTimelineProject {
    pub fn valid(&self) -> bool {
        [&self.videos, &self.takes].into_iter().all(|track| {
            track.len() <= MAX_TIMELINE_SEGMENTS && track.iter().all(EditSegmentProject::valid)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ProjectFile, load_project, new_project_id, resolve_media_paths, save_project_portable,
    };

    #[test]
    fn timeline_edits_round_trip_with_portable_video_and_take_paths() {
        let directory = std::env::temp_dir().join(format!("virtual-timeline-{}", new_project_id()));
        std::fs::create_dir(&directory).unwrap();
        let mut project = ProjectFile::default();
        let segment = |name: &str| EditSegmentProject {
            path: directory.join(name),
            name: name.into(),
            in_seconds: 1.0,
            out_seconds: 3.0,
            source_start: 0.5,
            source_duration: 4.0,
        };
        project.video_timeline.videos.push(segment("video.mp4"));
        project.video_timeline.takes.push(segment("take.jsonl"));
        let path = directory.join("edit.virtual");
        save_project_portable(&path, &project).unwrap();
        let mut loaded = load_project(&path).unwrap();
        assert_eq!(
            loaded.video_timeline.videos[0].path,
            PathBuf::from("video.mp4")
        );
        assert_eq!(
            loaded.video_timeline.takes[0].path,
            PathBuf::from("take.jsonl")
        );
        resolve_media_paths(&mut loaded, &directory);
        assert_eq!(loaded.video_timeline, project.video_timeline);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn old_projects_default_to_empty_edits_and_invalid_ranges_refuse_saving() {
        let mut json = serde_json::to_value(ProjectFile::default()).unwrap();
        json.as_object_mut().unwrap().remove("video_timeline");
        let mut project: ProjectFile = serde_json::from_value(json).unwrap();
        assert!(project.video_timeline.videos.is_empty());
        project.video_timeline.takes.push(EditSegmentProject {
            path: "take.jsonl".into(),
            name: "Take".into(),
            source_start: 2.0,
            in_seconds: 1.0,
            out_seconds: 3.0,
            source_duration: 4.0,
        });
        assert!(
            project.validate().is_err(),
            "trims cannot precede a take's baseline"
        );
        project.video_timeline.takes[0].in_seconds = 2.0;
        assert!(project.validate().is_ok());
    }
}
