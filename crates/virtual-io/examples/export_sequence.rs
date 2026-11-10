//! Export source edits from a saved VIRTUAL project without opening the GUI.
use std::path::PathBuf;
use virtual_io::{export_sequence_otio, load_project, resolve_media_paths};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let project_path = PathBuf::from(
        args.next()
            .ok_or("usage: export_sequence show.virtual output.otio")?,
    );
    let output_path = PathBuf::from(args.next().ok_or("missing output.otio path")?);
    let mut project = load_project(&project_path)?;
    resolve_media_paths(
        &mut project,
        project_path.parent().unwrap_or(std::path::Path::new(".")),
    );
    let sources: Result<Vec<_>, String> = project
        .sequence
        .clips
        .iter()
        .map(|clip| {
            project.decks[usize::from(clip.track)].clips[usize::from(clip.slot)]
                .clone()
                .map(|path| (clip.id, path))
                .ok_or_else(|| format!("Missing source for {}", clip.name))
        })
        .collect();
    export_sequence_otio(&output_path, &project.sequence, &sources?)?;
    println!("Exported {}", output_path.display());
    Ok(())
}
