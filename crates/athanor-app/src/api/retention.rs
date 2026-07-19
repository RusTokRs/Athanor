use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::config::ApiRetentionConfig;
use crate::project_path::normalize_canonical_path;

use super::model::{
    ApiCleanupArtifact, ApiCleanupArtifactKind, ApiCleanupOptions, ApiCleanupReport,
    ApiContractLatest, ApiRetentionOverrides,
};

pub fn cleanup_api_contracts(options: ApiCleanupOptions) -> Result<ApiCleanupReport> {
    let root = canonical_root(&options.root)?;
    let api_root = root.join(".athanor/api");
    let snapshots_dir = api_root.join("snapshots");
    let diffs_dir = api_root.join("diffs");
    let available_snapshots = available_snapshots(&snapshots_dir)?;
    let latest = read_api_latest(&api_root.join("latest.json"))?;
    let keep_snapshots = options.keep_snapshots.max(1);
    let mut retained_snapshot_ids = BTreeSet::new();

    if latest
        .as_ref()
        .is_some_and(|latest| available_snapshots.contains(&latest.snapshot))
    {
        retained_snapshot_ids.insert(latest.as_ref().expect("latest checked").snapshot.clone());
    }

    for snapshot in available_snapshots.iter().rev() {
        if retained_snapshot_ids.len() >= keep_snapshots {
            break;
        }
        retained_snapshot_ids.insert(snapshot.clone());
    }

    let mut removed = Vec::new();
    let mut retained = Vec::new();

    for snapshot in &available_snapshots {
        let artifact = ApiCleanupArtifact {
            kind: ApiCleanupArtifactKind::Snapshot,
            id: snapshot.clone(),
            path: snapshots_dir.join(format!("{snapshot}.json")),
        };
        if retained_snapshot_ids.contains(snapshot) {
            retained.push(artifact);
        } else {
            if !options.dry_run {
                remove_file_inside(&snapshots_dir, &artifact.path)?;
            }
            removed.push(artifact);
        }
    }

    let diff_files = list_api_diff_files(&diffs_dir)?;
    let mut retained_diff_ids = BTreeSet::new();
    if options.keep_diffs > 0 {
        for diff in diff_files
            .iter()
            .rev()
            .filter(|diff| diff.endpoints_retained(&retained_snapshot_ids))
        {
            if retained_diff_ids.len() >= options.keep_diffs {
                break;
            }
            retained_diff_ids.insert(diff.id.clone());
        }
    }

    for diff in diff_files {
        let artifact = ApiCleanupArtifact {
            kind: ApiCleanupArtifactKind::Diff,
            id: diff.id.clone(),
            path: diff.path,
        };
        if retained_diff_ids.contains(&diff.id) {
            retained.push(artifact);
        } else {
            if !options.dry_run {
                remove_file_inside(&diffs_dir, &artifact.path)?;
            }
            removed.push(artifact);
        }
    }

    Ok(ApiCleanupReport {
        schema: "athanor.api_cleanup.v1".to_string(),
        root,
        dry_run: options.dry_run,
        keep_snapshots,
        keep_diffs: options.keep_diffs,
        removed,
        retained,
    })
}

pub(crate) fn maybe_cleanup_api_contracts(
    root: &Path,
    config: &ApiRetentionConfig,
    overrides: &ApiRetentionOverrides,
) -> Result<Option<ApiCleanupReport>> {
    if !overrides.auto_cleanup.unwrap_or(config.auto_cleanup) {
        return Ok(None);
    }
    cleanup_api_contracts(ApiCleanupOptions {
        root: root.to_path_buf(),
        dry_run: false,
        keep_snapshots: overrides
            .keep_snapshots
            .unwrap_or(config.keep_snapshots)
            .max(1),
        keep_diffs: overrides.keep_diffs.unwrap_or(config.keep_diffs),
    })
    .map(Some)
}

pub(super) fn canonical_root(root: &Path) -> Result<PathBuf> {
    Ok(normalize_canonical_path(root.canonicalize().with_context(
        || format!("failed to canonicalize {}", root.display()),
    )?))
}

pub(super) fn available_snapshots(dir: &Path) -> Result<Vec<String>> {
    let mut snapshots = match fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(std::result::Result::ok)
            .filter_map(|entry| {
                let path = entry.path();
                (path.extension().and_then(|value| value.to_str()) == Some("json"))
                    .then(|| path.file_stem()?.to_str().map(str::to_string))
                    .flatten()
            })
            .collect::<Vec<_>>(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(error).context("failed to list API contract snapshots"),
    };
    snapshots.sort();
    Ok(snapshots)
}

fn read_api_latest(path: &Path) -> Result<Option<ApiContractLatest>> {
    match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content)
            .with_context(|| format!("failed to parse {}", path.display()))
            .map(Some),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("failed to read {}", path.display())),
    }
}

#[derive(Debug, Clone)]
struct ApiDiffFile {
    id: String,
    from: Option<String>,
    to: Option<String>,
    path: PathBuf,
}

impl ApiDiffFile {
    fn endpoints_retained(&self, snapshots: &BTreeSet<String>) -> bool {
        self.from
            .as_ref()
            .is_some_and(|from| snapshots.contains(from))
            && self.to.as_ref().is_some_and(|to| snapshots.contains(to))
    }
}

fn list_api_diff_files(dir: &Path) -> Result<Vec<ApiDiffFile>> {
    let mut diffs = match fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(std::result::Result::ok)
            .filter_map(|entry| {
                let path = entry.path();
                if path.extension().and_then(|value| value.to_str()) != Some("json") {
                    return None;
                }
                let id = path.file_stem()?.to_str()?.to_string();
                let (from, to) = id.split_once("--").map_or((None, None), |(from, to)| {
                    (Some(from.to_string()), Some(to.to_string()))
                });
                Some(ApiDiffFile { id, from, to, path })
            })
            .collect::<Vec<_>>(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(error).context("failed to list API contract diffs"),
    };
    diffs.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(diffs)
}

fn remove_file_inside(root: &Path, path: &Path) -> Result<()> {
    let root = root
        .canonicalize()
        .with_context(|| format!("failed to canonicalize {}", root.display()))?;
    let parent = path
        .parent()
        .context("cleanup target has no parent directory")?
        .canonicalize()
        .with_context(|| format!("failed to canonicalize {}", path.display()))?;
    if !parent.starts_with(&root) {
        bail!(
            "refusing to remove API artifact outside {}: {}",
            root.display(),
            path.display()
        );
    }
    fs::remove_file(path).with_context(|| format!("failed to remove {}", path.display()))
}
