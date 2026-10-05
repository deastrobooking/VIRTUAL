//! Branded paths and non-destructive access to pre-VIRTUAL show data.
use std::path::{Path, PathBuf};

/// Finder does not provide a writable working directory. Bundled launches keep
/// recovery data in Application Support; command-line launches retain their
/// chosen workspace and its existing recovery files.
pub(crate) fn workspace_directory() -> std::io::Result<PathBuf> {
    let current = std::env::current_dir()?;
    let executable = std::env::current_exe()?;
    let bundled = executable.parent().is_some_and(|directory| {
        directory.file_name().is_some_and(|name| name == "MacOS")
            && directory.parent().is_some_and(|contents| {
                contents.file_name().is_some_and(|name| name == "Contents")
                    && contents.join("Info.plist").is_file()
            })
    });
    if cfg!(target_os = "macos") && bundled {
        let home = std::env::var_os("HOME").ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "home directory unavailable")
        })?;
        let workspace = PathBuf::from(home).join("Library/Application Support/VIRTUAL");
        std::fs::create_dir_all(&workspace)?;
        Ok(workspace)
    } else {
        Ok(current)
    }
}

pub(crate) fn is_project_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("virtual") || extension.eq_ignore_ascii_case("oneiroi")
        })
}

pub(crate) fn journal_directories(workspace: &Path) -> [PathBuf; 2] {
    [
        workspace.join(".virtual/session"),
        workspace.join(".oneiroi/session"),
    ]
}

pub(crate) fn untitled_recovery(workspace: &Path) -> Option<PathBuf> {
    [
        virtual_io::autosave_path(None, workspace),
        workspace.join(".oneiroi-untitled.autosave"),
    ]
    .into_iter()
    .filter(|path| path.is_file())
    .max_by_key(|path| {
        path.metadata()
            .and_then(|metadata| metadata.modified())
            .ok()
    })
}

/// Where to save a recovered autosave without overwriting any existing show:
/// `<show> (recovered).virtual` beside the original project, or
/// `untitled (recovered).virtual` in the workspace for untitled work.
pub(crate) fn recovered_save_path(recovery: &Path, workspace: &Path) -> PathBuf {
    let original = recovery
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_prefix('.')?.strip_suffix(".autosave"))
        .map(Path::new)
        .filter(|original| is_project_path(original));
    let (directory, stem) = match original {
        Some(original) => (
            recovery.parent().unwrap_or(workspace),
            original
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("show"),
        ),
        None => (workspace, "untitled"),
    };
    (1..=999)
        .map(|attempt| {
            directory.join(if attempt == 1 {
                format!("{stem} (recovered).virtual")
            } else {
                format!("{stem} (recovered {attempt}).virtual")
            })
        })
        .find(|path| !path.exists())
        .unwrap_or_else(|| {
            directory.join(format!(
                "{stem} (recovered {}).virtual",
                virtual_io::new_project_id()
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_current_and_legacy_project_extensions() {
        for name in [
            "show.virtual",
            "show.VIRTUAL",
            "show.oneiroi",
            "show.ONEIROI",
        ] {
            assert!(is_project_path(Path::new(name)), "{name}");
        }
        for name in ["show.mov", "virtual", "show.virtual.mov"] {
            assert!(!is_project_path(Path::new(name)), "{name}");
        }
    }

    #[test]
    fn recovered_shows_never_overwrite_an_existing_file() {
        let directory = std::env::temp_dir().join(format!(
            "virtual-recovered-{}",
            virtual_io::new_project_id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let recovery = directory.join(".set.virtual.autosave");
        let first = recovered_save_path(&recovery, Path::new("/workspace"));
        assert_eq!(first, directory.join("set (recovered).virtual"));
        std::fs::write(&first, b"earlier recovery").unwrap();
        assert_eq!(
            recovered_save_path(&recovery, Path::new("/workspace")),
            directory.join("set (recovered 2).virtual")
        );
        assert_eq!(
            recovered_save_path(&directory.join(".virtual-untitled.autosave"), &directory),
            directory.join("untitled (recovered).virtual")
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn legacy_recovery_remains_available_without_moving_it() {
        let workspace =
            std::env::temp_dir().join(format!("virtual-paths-{}", virtual_io::new_project_id()));
        std::fs::create_dir_all(&workspace).unwrap();
        assert_eq!(untitled_recovery(&workspace), None);
        let legacy = workspace.join(".oneiroi-untitled.autosave");
        std::fs::write(&legacy, b"legacy").unwrap();
        assert_eq!(untitled_recovery(&workspace), Some(legacy.clone()));
        let current = virtual_io::autosave_path(None, &workspace);
        std::fs::write(&current, b"current").unwrap();
        assert!(untitled_recovery(&workspace).is_some());
        assert!(legacy.is_file());
        std::fs::remove_dir_all(workspace).unwrap();
    }
}
