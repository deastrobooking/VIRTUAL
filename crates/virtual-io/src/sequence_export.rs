//! Source-edit interchange. Does not claim to bake VIRTUAL's GPU effects or buses.
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use virtual_core::sequence::ShowSequence;

fn time(frame: u64, fps: u32) -> Value {
    json!({"OTIO_SCHEMA":"RationalTime.1", "value":frame, "rate":fps})
}
fn range(start: u64, duration: u64, fps: u32) -> Value {
    json!({"OTIO_SCHEMA":"TimeRange.1", "start_time":time(start,fps), "duration":time(duration,fps)})
}
fn file_url(path: &Path) -> String {
    let mut result = String::from("file://");
    for byte in path.to_string_lossy().replace('\\', "/").bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~:".contains(&byte) {
            result.push(char::from(byte));
        } else {
            result.push_str(&format!("%{byte:02X}"));
        }
    }
    result
}

pub fn sequence_otio(sequence: &ShowSequence, sources: &[(u64, PathBuf)]) -> Result<Value, String> {
    sequence.validate().map_err(str::to_owned)?;
    let mut tracks = Vec::new();
    for track in 0..4 {
        let mut clips: Vec<_> = sequence.clips.iter().filter(|c| c.track == track).collect();
        clips.sort_by_key(|c| c.start);
        let mut children = Vec::new();
        let mut cursor = 0;
        for clip in clips {
            if clip.start > cursor {
                children.push(json!({"OTIO_SCHEMA":"Gap.1", "name":"", "metadata":{}, "effects":[], "markers":[],
                    "source_range":range(0,clip.start-cursor,sequence.fps)}));
            }
            let source = sources
                .iter()
                .find(|(id, _)| *id == clip.id)
                .ok_or_else(|| format!("Missing source for {}", clip.name))?;
            if !source.1.is_absolute() {
                return Err("Export sources must be resolved absolute paths".into());
            }
            let reference = json!({"OTIO_SCHEMA":"ExternalReference.1", "target_url":file_url(&source.1), "metadata":{}, "available_range":null});
            children.push(json!({"OTIO_SCHEMA":"Clip.2", "name":clip.name, "metadata":{"virtual":{"clip_id":clip.id,"slot":clip.slot}},
                "effects":[], "markers":[], "enabled":true,
                "source_range":range(clip.source_in,clip.duration,sequence.fps),
                "active_media_reference_key":"DEFAULT_MEDIA", "media_references":{"DEFAULT_MEDIA":reference}}));
            cursor = clip.end();
        }
        if cursor < sequence.duration() {
            children.push(
                json!({"OTIO_SCHEMA":"Gap.1", "name":"", "metadata":{}, "effects":[], "markers":[],
                "source_range":range(0,sequence.duration()-cursor,sequence.fps)}),
            );
        }
        tracks.push(
            json!({"OTIO_SCHEMA":"Track.1", "name":format!("V{}",track+1), "kind":"Video",
            "metadata":{}, "effects":[], "markers":[], "source_range":null, "children":children}),
        );
    }
    let markers: Vec<_> = sequence.markers.iter().map(|m| json!({"OTIO_SCHEMA":"Marker.2", "name":m.name, "color":"CYAN", "metadata":{}, "marked_range":range(m.frame,0,sequence.fps)})).collect();
    Ok(
        json!({"OTIO_SCHEMA":"Timeline.1", "name":sequence.name, "global_start_time":time(0,sequence.fps),
        "metadata":{"virtual":{"export":"source_edits_only", "effects_baked":false}},
        "tracks":{"OTIO_SCHEMA":"Stack.1", "name":"tracks", "source_range":null, "metadata":{}, "effects":[], "markers":markers,"children":tracks}}),
    )
}

pub fn export_sequence_otio(
    path: &Path,
    sequence: &ShowSequence,
    sources: &[(u64, PathBuf)],
) -> Result<(), String> {
    use std::io::Write;
    let resolved: Result<Vec<_>, String> = sources
        .iter()
        .map(|(id, source)| {
            source
                .canonicalize()
                .and_then(|path| {
                    if path.is_file() {
                        Ok((*id, path))
                    } else {
                        Err(std::io::Error::other("Timeline source is not a file"))
                    }
                })
                .map_err(|error| error.to_string())
        })
        .collect();
    let value = sequence_otio(sequence, &resolved?)?;
    // Unique sibling + rename: a failed write does not truncate an existing export.
    let temporary = path.with_extension(format!("otio-{}.tmp", crate::new_project_id()));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        serde_json::to_writer_pretty(&mut file, &value).map_err(|e| e.to_string())?;
        file.flush().map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        std::fs::rename(&temporary, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use virtual_core::sequence::SequenceClip;
    #[test]
    fn export_keeps_gaps_trims_markers_and_escaped_source_urls() {
        let mut sequence = ShowSequence::default();
        sequence.clips.push(SequenceClip {
            id: 1,
            track: 0,
            slot: 0,
            name: "Shot".into(),
            start: 30,
            duration: 60,
            source_in: 12,
        });
        let doc = sequence_otio(&sequence, &[(1, PathBuf::from("/tmp/shot #1.mov"))]).unwrap();
        let children = &doc["tracks"]["children"][0]["children"];
        assert_eq!(children[0]["OTIO_SCHEMA"], "Gap.1");
        assert_eq!(children[0]["source_range"]["duration"]["value"], 30);
        assert_eq!(children[1]["source_range"]["start_time"]["value"], 12);
        assert_eq!(
            children[1]["media_references"]["DEFAULT_MEDIA"]["target_url"],
            "file:///tmp/shot%20%231.mov"
        );
        assert_eq!(doc["tracks"]["children"].as_array().unwrap().len(), 4);
        assert!(sequence_otio(&sequence, &[]).is_err());
    }
}
