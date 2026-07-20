use std::collections::BTreeMap;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use fs2::FileExt;
use serde::Serialize;

mod recovery;

pub use recovery::*;

const READ_ROOT: &str = ".athanor/generated/index-generations";
const STATE_ROOT: &str = ".athanor/state";
const LOCK_PATH: &str = ".athanor/state/index-publication.lock";
const TOMBSTONE_PREFIX: &str = ".cleanup-";

#[derive(Debug, Clone)]
pub struct RepairRecoverIndexCleanupOptions {
    pub root: PathBuf,
    pub dry_run: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IndexCleanupTombstone {
    pub generation: String,
    pub token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_model: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index_state: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RepairRecoverIndexCleanupReport {
    pub schema: String,
    pub root: PathBuf,
    pub dry_run: bool,
    pub needed: bool,
    pub recovered: bool,
    pub tombstones: Vec<IndexCleanupTombstone>,
    pub remaining_issues: Vec<RepairIssue>,
}

/// Finishes or rolls back index-generation deletion that was already staged through sibling tombstones.
///
/// The function never selects live generations for deletion. It only handles direct-child tombstones
/// created by the confirmed retention protocol, under the application publication lock.
pub fn recover_index_cleanup(
    options: RepairRecoverIndexCleanupOptions,
) -> Result<RepairRecoverIndexCleanupReport> {
    let root = crate::project_path::normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );
    let initial = scan_tombstones(&root)?;
    if options.dry_run || initial.is_empty() {
        return report(root, options.dry_run, false, initial);
    }

    let _lock = CleanupLock::acquire(root.join(LOCK_PATH))?;
    let staged = scan_tombstones(&root)?;
    for tombstone in &staged {
        recover_tombstone(&root, tombstone)?;
    }
    let remaining = scan_tombstones(&root)?;
    if !remaining.is_empty() {
        bail!("index cleanup recovery left staged tombstones behind");
    }
    report(root, false, !staged.is_empty(), staged)
}

fn report(
    root: PathBuf,
    dry_run: bool,
    recovered: bool,
    tombstones: Vec<IndexCleanupTombstone>,
) -> Result<RepairRecoverIndexCleanupReport> {
    let needed = !tombstones.is_empty();
    let remaining_issues = if recovered {
        inspect_repair(RepairInspectOptions { root: root.clone() })?.issues
    } else {
        Vec::new()
    };
    Ok(RepairRecoverIndexCleanupReport {
        schema: "athanor.repair_recover_index_cleanup.v1".to_string(),
        root,
        dry_run,
        needed,
        recovered,
        tombstones,
        remaining_issues,
    })
}

fn scan_tombstones(root: &Path) -> Result<Vec<IndexCleanupTombstone>> {
    let mut rows = BTreeMap::<String, IndexCleanupTombstone>::new();
    scan_root(&root.join(READ_ROOT), TombstoneKind::ReadModel, &mut rows)?;
    scan_root(&root.join(STATE_ROOT), TombstoneKind::IndexState, &mut rows)?;
    Ok(rows.into_values().collect())
}

#[derive(Debug, Clone, Copy)]
enum TombstoneKind {
    ReadModel,
    IndexState,
}

fn scan_root(
    root: &Path,
    kind: TombstoneKind,
    rows: &mut BTreeMap<String, IndexCleanupTombstone>,
) -> Result<()> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error).context(format!("failed to inspect {}", root.display())),
    };
    for entry in entries {
        let entry = entry.with_context(|| format!("failed to inspect {}", root.display()))?;
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            bail!(
                "refusing index cleanup tombstone symlink {}",
                entry.path().display()
            );
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let token = match kind {
            TombstoneKind::ReadModel if file_type.is_dir() => name.strip_prefix(TOMBSTONE_PREFIX),
            TombstoneKind::IndexState if file_type.is_file() => name
                .strip_prefix(TOMBSTONE_PREFIX)
                .and_then(|name| name.strip_suffix(".json")),
            _ => None,
        };
        let Some(token) = token else {
            continue;
        };
        let generation = parse_generation(token)?;
        let row = rows
            .entry(token.to_string())
            .or_insert_with(|| IndexCleanupTombstone {
                generation,
                token: token.to_string(),
                read_model: None,
                index_state: None,
            });
        match kind {
            TombstoneKind::ReadModel => row.read_model = Some(entry.path()),
            TombstoneKind::IndexState => row.index_state = Some(entry.path()),
        }
    }
    Ok(())
}

fn parse_generation(token: &str) -> Result<String> {
    let (before_nanos, nanos) = token
        .rsplit_once('-')
        .with_context(|| format!("invalid index cleanup tombstone token `{token}`"))?;
    let (generation, process_id) = before_nanos
        .rsplit_once('-')
        .with_context(|| format!("invalid index cleanup tombstone token `{token}`"))?;
    if !generation.starts_with("gen_")
        || process_id.parse::<u32>().is_err()
        || nanos.parse::<u128>().is_err()
    {
        bail!("invalid index cleanup tombstone token `{token}`");
    }
    Ok(generation.to_string())
}

fn recover_tombstone(root: &Path, tombstone: &IndexCleanupTombstone) -> Result<()> {
    let live_read = root.join(READ_ROOT).join(&tombstone.generation);
    let live_state = root
        .join(STATE_ROOT)
        .join(format!("index-state-{}.json", tombstone.generation));
    match (&tombstone.read_model, &tombstone.index_state) {
        (Some(read_tombstone), Some(state_tombstone)) => {
            ensure_absent(&live_read, "read model", tombstone)?;
            ensure_absent(&live_state, "index state", tombstone)?;
            fs::remove_dir_all(read_tombstone).with_context(|| {
                format!(
                    "failed to remove staged read model {}",
                    read_tombstone.display()
                )
            })?;
            fs::remove_file(state_tombstone).with_context(|| {
                format!(
                    "failed to remove staged index state {}",
                    state_tombstone.display()
                )
            })?;
        }
        (Some(read_tombstone), None) if live_state.is_file() => {
            ensure_absent(&live_read, "read model", tombstone)?;
            fs::rename(read_tombstone, &live_read).with_context(|| {
                format!(
                    "failed to roll back staged read model {} to {}",
                    read_tombstone.display(),
                    live_read.display()
                )
            })?;
        }
        (Some(read_tombstone), None) => {
            ensure_absent(&live_read, "read model", tombstone)?;
            fs::remove_dir_all(read_tombstone).with_context(|| {
                format!(
                    "failed to remove staged read model {}",
                    read_tombstone.display()
                )
            })?;
        }
        (None, Some(state_tombstone)) => {
            ensure_absent(&live_read, "read model", tombstone)?;
            ensure_absent(&live_state, "index state", tombstone)?;
            fs::remove_file(state_tombstone).with_context(|| {
                format!(
                    "failed to remove staged index state {}",
                    state_tombstone.display()
                )
            })?;
        }
        (None, None) => bail!("empty index cleanup tombstone row {}", tombstone.token),
    }
    Ok(())
}

fn ensure_absent(path: &Path, kind: &str, tombstone: &IndexCleanupTombstone) -> Result<()> {
    if path.exists() {
        bail!(
            "refusing cleanup recovery because live {kind} {} conflicts with tombstone {}",
            path.display(),
            tombstone.token
        );
    }
    Ok(())
}

struct CleanupLock {
    _file: File,
}

impl CleanupLock {
    fn acquire(path: PathBuf) -> Result<Self> {
        let parent = path.parent().with_context(|| {
            format!("index cleanup lock path has no parent: {}", path.display())
        })?;
        fs::create_dir_all(parent)?;
        let file = File::create(&path)
            .with_context(|| format!("failed to open publication lock {}", path.display()))?;
        file.lock_exclusive()
            .with_context(|| format!("failed to acquire publication lock {}", path.display()))?;
        Ok(Self { _file: file })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovers_both_staged_tombstones_idempotently() {
        let root = test_root("both");
        let tombstone = stage_pair(&root, "gen_snap_both", "77-1000");

        let plan = recover_index_cleanup(RepairRecoverIndexCleanupOptions {
            root: root.clone(),
            dry_run: true,
        })
        .unwrap();
        assert!(plan.needed);
        assert!(!plan.recovered);
        assert_eq!(plan.tombstones, vec![tombstone.clone()]);

        let applied = recover_index_cleanup(RepairRecoverIndexCleanupOptions {
            root: root.clone(),
            dry_run: false,
        })
        .unwrap();
        assert!(applied.recovered);
        assert!(!tombstone.read_model.unwrap().exists());
        assert!(!tombstone.index_state.unwrap().exists());

        let repeated = recover_index_cleanup(RepairRecoverIndexCleanupOptions {
            root: root.clone(),
            dry_run: false,
        })
        .unwrap();
        assert!(!repeated.needed);
        assert!(!repeated.recovered);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rolls_back_read_tombstone_before_state_staging() {
        let root = test_root("read-only");
        let tombstone = stage_pair(&root, "gen_snap_read", "66-900");
        let live_state = root.join(STATE_ROOT).join("index-state-gen_snap_read.json");
        fs::rename(tombstone.index_state.as_ref().unwrap(), &live_state).unwrap();

        let applied = recover_index_cleanup(RepairRecoverIndexCleanupOptions {
            root: root.clone(),
            dry_run: false,
        })
        .unwrap();
        let live_read = root.join(READ_ROOT).join("gen_snap_read");
        assert!(applied.recovered);
        assert!(live_read.is_dir());
        assert!(live_state.is_file());
        assert!(!tombstone.read_model.unwrap().exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn recovers_state_tombstone_after_read_removal_fault() {
        let root = test_root("state-only");
        let tombstone = stage_pair(&root, "gen_snap_state", "88-2000");
        fs::remove_dir_all(tombstone.read_model.as_ref().unwrap()).unwrap();

        let applied = recover_index_cleanup(RepairRecoverIndexCleanupOptions {
            root: root.clone(),
            dry_run: false,
        })
        .unwrap();
        assert!(applied.recovered);
        assert!(!tombstone.index_state.unwrap().exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn live_generation_conflict_fails_closed() {
        let root = test_root("conflict");
        let tombstone = stage_pair(&root, "gen_snap_conflict", "99-3000");
        fs::create_dir_all(root.join(READ_ROOT).join("gen_snap_conflict")).unwrap();

        let error = recover_index_cleanup(RepairRecoverIndexCleanupOptions {
            root: root.clone(),
            dry_run: false,
        })
        .expect_err("live generation must conflict with staged tombstone");
        assert!(error.to_string().contains("live read model"));
        assert!(tombstone.read_model.unwrap().exists());
        fs::remove_dir_all(root).unwrap();
    }

    fn stage_pair(root: &Path, generation: &str, nonce: &str) -> IndexCleanupTombstone {
        let read_root = root.join(READ_ROOT);
        let state_root = root.join(STATE_ROOT);
        fs::create_dir_all(&read_root).unwrap();
        fs::create_dir_all(&state_root).unwrap();
        let token = format!("{generation}-{nonce}");
        let read_model = read_root.join(format!("{TOMBSTONE_PREFIX}{token}"));
        let index_state = state_root.join(format!("{TOMBSTONE_PREFIX}{token}.json"));
        fs::create_dir_all(&read_model).unwrap();
        fs::write(read_model.join("manifest.json"), "{}").unwrap();
        fs::write(&index_state, "{}").unwrap();
        IndexCleanupTombstone {
            generation: generation.to_string(),
            token,
            read_model: Some(read_model),
            index_state: Some(index_state),
        }
    }

    fn test_root(label: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("athanor-cleanup-recovery-{label}-{nonce}"));
        fs::create_dir_all(&root).unwrap();
        root
    }
}
