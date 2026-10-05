//! Serialized project writes. Submission and completion polling never wait on disk.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::thread::{self, JoinHandle};

use virtual_io::{ProjectFile, autosave_path, save_project_portable};

use crate::project;

const SAVE_QUEUE_CAPACITY: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SaveKind {
    Project,
    Recovery,
}

pub(crate) struct SaveRequest {
    pub epoch: u64,
    pub path: PathBuf,
    pub snapshot: ProjectFile,
    pub kind: SaveKind,
    /// Recovery files a successful project save makes obsolete. The
    /// destination's own autosave is always removed; any other path only if
    /// this session wrote it, so an unrecovered crash snapshot survives.
    pub supersedes: Vec<PathBuf>,
}

pub(crate) struct SaveCompletion {
    pub request: SaveRequest,
    pub result: Result<(), String>,
}

impl SaveCompletion {
    pub fn belongs_to(&self, epoch: u64) -> bool {
        self.request.epoch == epoch
    }
}

pub(crate) struct ProjectSaver {
    requests: Option<SyncSender<SaveRequest>>,
    completions: Option<Receiver<SaveCompletion>>,
    worker: Option<JoinHandle<()>>,
}

impl ProjectSaver {
    pub fn new() -> std::io::Result<Self> {
        let mut recovery = RecoveryLedger::default();
        Self::spawn(move |request| {
            if request.kind == SaveKind::Recovery && recovery.is_redundant(request) {
                // Identical to the project just saved: an equal-but-newer
                // autosave would only raise a false recovery offer later.
                remove_recovery(&request.path);
                return Ok(());
            }
            save_project_portable(&request.path, &request.snapshot)
                .map_err(|error| error.to_string())?;
            recovery.record(request);
            Ok(())
        })
    }

    fn spawn(
        mut write: impl FnMut(&SaveRequest) -> Result<(), String> + Send + 'static,
    ) -> std::io::Result<Self> {
        let (requests, incoming) = mpsc::sync_channel::<SaveRequest>(SAVE_QUEUE_CAPACITY);
        let (finished, completions) = mpsc::sync_channel(SAVE_QUEUE_CAPACITY);
        let worker = thread::Builder::new()
            .name("virtual-project-save".to_owned())
            .spawn(move || {
                for request in incoming {
                    let result = write(&request);
                    if let Err(error) = &result {
                        log::error!("save {}: {error}", request.path.display());
                    }
                    // Completion backpressure is bounded and stays on this worker.
                    // Shutdown drains completions before joining.
                    let _ = finished.send(SaveCompletion { request, result });
                }
            })?;
        Ok(Self {
            requests: Some(requests),
            completions: Some(completions),
            worker: Some(worker),
        })
    }

    pub fn submit(&self, request: SaveRequest) -> Result<(), &'static str> {
        let Some(requests) = &self.requests else {
            return Err("Project save worker has stopped.");
        };
        match requests.try_send(request) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => Err("Save queue is busy; try again shortly."),
            Err(TrySendError::Disconnected(_)) => Err("Project save worker is unavailable."),
        }
    }

    pub fn try_recv(&self) -> Option<SaveCompletion> {
        self.completions.as_ref()?.try_recv().ok()
    }

    /// Only used once presentation has stopped. Drain accepted saves before
    /// returning so close-time recovery cannot be lost to a full queue.
    pub fn finish(&mut self) -> Vec<SaveCompletion> {
        self.requests.take();
        let completed = self
            .completions
            .take()
            .map(|receiver| receiver.into_iter().collect())
            .unwrap_or_default();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        completed
    }
}

/// Worker-side memory of what was saved, so recovery files never outlive the
/// state they protect.
#[derive(Default)]
struct RecoveryLedger {
    saved: Option<(PathBuf, ProjectFile)>,
    written: BTreeSet<PathBuf>,
}

impl RecoveryLedger {
    fn is_redundant(&self, request: &SaveRequest) -> bool {
        self.saved.as_ref().is_some_and(|(path, saved)| {
            autosave_path(Some(path), Path::new("")) == request.path
                && !project::is_dirty(&request.snapshot, Some(saved))
        })
    }

    fn record(&mut self, request: &SaveRequest) {
        match request.kind {
            SaveKind::Recovery => {
                self.written.insert(request.path.clone());
            }
            SaveKind::Project => {
                let own = autosave_path(Some(&request.path), Path::new(""));
                for path in &request.supersedes {
                    if *path == own || self.written.remove(path) {
                        remove_recovery(path);
                    }
                }
                remove_recovery(&own);
                self.written.remove(&own);
                self.saved = Some((request.path.clone(), request.snapshot.clone()));
            }
        }
    }
}

fn remove_recovery(path: &Path) {
    if let Err(error) = std::fs::remove_file(path)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        log::warn!("remove superseded recovery {}: {error}", path.display());
    }
}

impl Drop for ProjectSaver {
    fn drop(&mut self) {
        self.finish();
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use super::*;

    fn request(epoch: u64) -> SaveRequest {
        SaveRequest {
            epoch,
            path: "unused.virtual".into(),
            snapshot: ProjectFile::default(),
            kind: SaveKind::Recovery,
            supersedes: Vec::new(),
        }
    }

    #[test]
    fn stalled_disk_does_not_block_submission_and_queue_is_bounded() {
        let (entered, started) = mpsc::channel();
        let (release, blocked) = mpsc::channel();
        let (written, writes) = mpsc::channel();
        let mut first = true;
        let mut saver = ProjectSaver::spawn(move |request| {
            if first {
                first = false;
                entered.send(()).unwrap();
                blocked.recv_timeout(Duration::from_secs(5)).unwrap();
            }
            written.send(request.epoch).unwrap();
            Ok(())
        })
        .unwrap();
        saver.submit(request(0)).unwrap();
        started.recv_timeout(Duration::from_secs(5)).unwrap();
        for epoch in 1..=SAVE_QUEUE_CAPACITY as u64 {
            saver.submit(request(epoch)).unwrap();
        }
        assert!(saver.submit(request(99)).is_err());
        assert!(saver.try_recv().is_none());
        release.send(()).unwrap();
        saver.finish();
        assert_eq!(writes.try_iter().collect::<Vec<_>>(), vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn completions_report_failures_and_identify_stale_projects() {
        let saver = ProjectSaver::spawn(|_| Err("disk unavailable".to_owned())).unwrap();
        saver.submit(request(7)).unwrap();
        let completion = saver
            .completions
            .as_ref()
            .unwrap()
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        assert!(completion.belongs_to(7));
        assert!(!completion.belongs_to(8));
        assert_eq!(completion.result, Err("disk unavailable".to_owned()));
    }

    fn temp_show(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("virtual-{name}-{stamp}"));
        std::fs::create_dir_all(&directory).unwrap();
        directory
    }

    fn save(saver: &ProjectSaver, path: &Path, snapshot: &ProjectFile, kind: SaveKind) {
        saver
            .submit(SaveRequest {
                epoch: 1,
                path: path.to_owned(),
                snapshot: snapshot.clone(),
                kind,
                supersedes: Vec::new(),
            })
            .unwrap();
    }

    #[test]
    fn project_save_retires_its_recovery_and_skips_identical_autosaves() {
        let directory = temp_show("ledger");
        let path = directory.join("show.virtual");
        let recovery = autosave_path(Some(&path), &directory);
        let mut edited = ProjectFile::default();
        edited.settings.bpm = 128.0;

        let saver = ProjectSaver::new().unwrap();
        save(&saver, &recovery, &edited, SaveKind::Recovery);
        save(&saver, &path, &edited, SaveKind::Project);
        // Queued before the save completed, but carries nothing new.
        save(&saver, &recovery, &edited, SaveKind::Recovery);
        drop(saver);
        assert!(path.is_file());
        assert!(!recovery.exists());
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn saving_keeps_a_crash_snapshot_this_session_did_not_write() {
        let directory = temp_show("crash");
        let untitled = autosave_path(None, &directory);
        virtual_io::save_project_atomic(&untitled, &ProjectFile::default()).unwrap();

        let saver = ProjectSaver::new().unwrap();
        saver
            .submit(SaveRequest {
                epoch: 1,
                path: directory.join("new.virtual"),
                snapshot: ProjectFile::default(),
                kind: SaveKind::Project,
                supersedes: vec![untitled.clone()],
            })
            .unwrap();
        drop(saver);
        assert!(untitled.is_file());
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn shutdown_flushes_accepted_snapshots_in_order() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("virtual-save-{stamp}"));
        let path = directory.join("show.virtual");
        let saver = ProjectSaver::new().unwrap();
        let mut last = ProjectFile::default();
        for bpm in [110.0, 125.0, 140.0] {
            last.settings.bpm = bpm;
            saver
                .submit(SaveRequest {
                    epoch: 1,
                    path: path.clone(),
                    snapshot: last.clone(),
                    kind: SaveKind::Project,
                    supersedes: Vec::new(),
                })
                .unwrap();
        }
        drop(saver);
        assert_eq!(virtual_io::load_project(&path).unwrap(), last);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
