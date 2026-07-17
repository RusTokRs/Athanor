use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use athanor_core::{CoreError, CoreResult, SnapshotSelector};
use athanor_domain::{Diagnostic, Entity, Fact, Relation, SnapshotId};
use fs2::FileExt;
use serde::{Deserialize, Serialize};

use crate::pointer_publication::publish_json;
use crate::snapshot_io::discover_next_snapshot;

#[derive(Debug, Clone)]
pub struct JsonlKnowledgeStore {
    pub(crate) root: PathBuf,
    pub(crate) state: Arc<Mutex<State>>,
}

#[derive(Debug, Default)]
pub(crate) struct State {
    next_snapshot: u64,
    snapshots: HashMap<SnapshotId, SnapshotData>,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct SnapshotData {
    pub(crate) committed: bool,
    pub(crate) prepared: bool,
    pub(crate) entities: Vec<Entity>,
    pub(crate) facts: Vec<Fact>,
    pub(crate) relations: Vec<Relation>,
    pub(crate) diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Deserialize, Serialize)]
struct SnapshotSequence {
    next_snapshot: u64,
}

impl JsonlKnowledgeStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let next_snapshot = discover_next_snapshot(&root);
        Self {
            root,
            state: Arc::new(Mutex::new(State {
                next_snapshot,
                snapshots: HashMap::new(),
            })),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn lock_state(&self) -> CoreResult<std::sync::MutexGuard<'_, State>> {
        self.state
            .lock()
            .map_err(|_| CoreError::Adapter("jsonl store lock poisoned".to_string()))
    }

    pub(crate) fn snapshot_dir(&self, snapshot: &SnapshotId) -> PathBuf {
        self.root.join("snapshots").join(&snapshot.0)
    }

    pub(crate) fn prepared_snapshot_dir(&self, snapshot: &SnapshotId) -> PathBuf {
        self.root
            .join("snapshots")
            .join(format!(".{}.prepared", snapshot.0))
    }

    pub(crate) fn acquire_writer_lock(&self) -> CoreResult<File> {
        fs::create_dir_all(&self.root)
            .map_err(|error| CoreError::Adapter(format!("failed to create store dir: {error}")))?;
        let path = self.root.join(".writer.lock");
        let lock = File::create(&path)
            .map_err(|error| CoreError::Adapter(format!("failed to open writer lock: {error}")))?;
        lock.lock_exclusive()
            .map_err(|error| CoreError::Adapter(format!("failed to acquire writer lock: {error}")))?;
        Ok(lock)
    }

    pub(crate) fn allocate_snapshot_number(&self) -> CoreResult<u64> {
        let path = self.root.join("snapshot-sequence.json");
        let persisted = read_snapshot_sequence(&path)?;
        let next = discover_next_snapshot(&self.root)
            .max(persisted)
            .saturating_add(1);
        publish_json(
            &path,
            &SnapshotSequence {
                next_snapshot: next,
            },
            ".snapshot-sequence.staging-",
            "snapshot sequence",
        )?;
        Ok(next)
    }

    pub(crate) fn recover_known_staging(&self) -> CoreResult<()> {
        self.recover_snapshot_staging()?;
        self.recover_pointer_staging()
    }

    fn recover_snapshot_staging(&self) -> CoreResult<()> {
        let snapshots = self.root.join("snapshots");
        if !snapshots.exists() {
            return Ok(());
        }
        let active_prepared = self
            .lock_state()?
            .snapshots
            .iter()
            .filter(|(_, data)| data.prepared && !data.committed)
            .map(|(snapshot, _)| format!(".{}.prepared", snapshot.0))
            .collect::<HashSet<_>>();
        for entry in fs::read_dir(&snapshots).map_err(|error| {
            CoreError::Adapter(format!("failed to inspect snapshot staging: {error}"))
        })? {
            let entry = entry.map_err(|error| {
                CoreError::Adapter(format!("failed to inspect snapshot staging entry: {error}"))
            })?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if name.starts_with(".snap_jsonl_")
                && (name.contains(".staging-")
                    || name.contains(".prepare-staging-")
                    || (name.ends_with(".prepared") && !active_prepared.contains(name)))
            {
                fs::remove_dir_all(entry.path()).map_err(|error| {
                    CoreError::Adapter(format!(
                        "failed to remove stale snapshot staging {}: {error}",
                        entry.path().display()
                    ))
                })?;
            }
        }
        Ok(())
    }

    fn recover_pointer_staging(&self) -> CoreResult<()> {
        for prefix in [
            ".latest.json.staging-",
            ".latest.json.identity-staging-",
            ".snapshot-sequence.staging-",
        ] {
            let entries = match fs::read_dir(&self.root) {
                Ok(entries) => entries,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => {
                    return Err(CoreError::Adapter(format!(
                        "failed to inspect store staging: {error}"
                    )));
                }
            };
            for entry in entries {
                let entry = entry.map_err(|error| {
                    CoreError::Adapter(format!("failed to inspect store staging entry: {error}"))
                })?;
                if entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.starts_with(prefix))
                {
                    fs::remove_file(entry.path()).map_err(|error| {
                        CoreError::Adapter(format!(
                            "failed to remove stale store staging {}: {error}",
                            entry.path().display()
                        ))
                    })?;
                }
            }
        }
        Ok(())
    }
}

impl State {
    pub(crate) fn reserve_snapshot(&mut self, next_snapshot: u64) -> SnapshotId {
        self.next_snapshot = self.next_snapshot.max(next_snapshot);
        let snapshot = SnapshotId(format!("snap_jsonl_{:08}", self.next_snapshot));
        self.snapshots
            .insert(snapshot.clone(), SnapshotData::default());
        snapshot
    }

    pub(crate) fn snapshot(&self, snapshot: &SnapshotId) -> CoreResult<&SnapshotData> {
        self.snapshots
            .get(snapshot)
            .ok_or_else(|| CoreError::NotFound(format!("snapshot {}", snapshot.0)))
    }

    pub(crate) fn snapshot_mut(
        &mut self,
        snapshot: &SnapshotId,
    ) -> CoreResult<&mut SnapshotData> {
        self.snapshots
            .get_mut(snapshot)
            .ok_or_else(|| CoreError::NotFound(format!("snapshot {}", snapshot.0)))
    }

    pub(crate) fn remove_snapshot(&mut self, snapshot: &SnapshotId) -> CoreResult<()> {
        let data = self
            .snapshots
            .get(snapshot)
            .ok_or_else(|| CoreError::NotFound(format!("snapshot {}", snapshot.0)))?;
        if data.committed {
            return Err(CoreError::Conflict(format!(
                "cannot abort committed snapshot {}",
                snapshot.0
            )));
        }
        self.snapshots.remove(snapshot);
        Ok(())
    }

    pub(crate) fn committed_snapshot(
        &self,
        selector: &SnapshotSelector,
    ) -> CoreResult<Option<SnapshotId>> {
        match selector {
            SnapshotSelector::Exact(snapshot) => {
                let data = self.snapshot(snapshot)?;
                if !data.committed {
                    return Err(CoreError::SnapshotNotCommitted(snapshot.0.clone()));
                }
                Ok(Some(snapshot.clone()))
            }
            SnapshotSelector::LatestCommitted => Ok(self
                .snapshots
                .iter()
                .filter(|(_, data)| data.committed)
                .map(|(snapshot, _)| snapshot.clone())
                .max_by(|left, right| left.0.cmp(&right.0))),
        }
    }

    pub(crate) fn snapshot_data(&self, snapshot: Option<&SnapshotId>) -> Option<&SnapshotData> {
        snapshot.and_then(|snapshot| self.snapshots.get(snapshot))
    }
}

fn read_snapshot_sequence(path: &Path) -> CoreResult<u64> {
    if !path.exists() {
        return Ok(0);
    }
    let content = fs::read_to_string(path)
        .map_err(|error| CoreError::Adapter(format!("failed to read snapshot sequence: {error}")))?;
    serde_json::from_str::<SnapshotSequence>(&content)
        .map(|sequence| sequence.next_snapshot)
        .map_err(|error| CoreError::Adapter(format!("failed to parse snapshot sequence: {error}")))
}
