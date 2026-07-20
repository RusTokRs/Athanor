use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;

use athanor_core::{CoreError, CoreResult};
use athanor_domain::SnapshotId;
use serde::Serialize;
use serde_json::json;

use crate::indexes::write_indexes;
use crate::pointer_publication::unique_suffix;
use crate::store::SnapshotData;

pub(crate) fn write_snapshot(
    root: &Path,
    snapshot: &SnapshotId,
    data: &SnapshotData,
) -> CoreResult<()> {
    let snapshot_dir = root.join("snapshots").join(&snapshot.0);
    let parent = snapshot_dir.parent().ok_or_else(|| {
        CoreError::Adapter(format!("snapshot {} has no parent directory", snapshot.0))
    })?;
    fs::create_dir_all(parent)
        .map_err(|error| CoreError::Adapter(format!("failed to create store dir: {error}")))?;
    if snapshot_dir.exists() {
        return Err(CoreError::Conflict(format!(
            "snapshot directory {} already exists",
            snapshot_dir.display()
        )));
    }

    let staging = parent.join(format!(".{}.staging-{}", snapshot.0, unique_suffix()));
    fs::create_dir(&staging).map_err(|error| {
        CoreError::Adapter(format!(
            "failed to create snapshot staging directory: {error}"
        ))
    })?;
    if let Err(error) = write_snapshot_contents(&staging, snapshot, data) {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    fs::rename(&staging, &snapshot_dir).map_err(|error| {
        let _ = fs::remove_dir_all(&staging);
        CoreError::Adapter(format!("failed to publish snapshot atomically: {error}"))
    })
}

pub(crate) fn write_prepared_snapshot(
    root: &Path,
    snapshot: &SnapshotId,
    data: &SnapshotData,
) -> CoreResult<()> {
    let prepared = root
        .join("snapshots")
        .join(format!(".{}.prepared", snapshot.0));
    let parent = prepared.parent().ok_or_else(|| {
        CoreError::Adapter(format!(
            "prepared snapshot {} has no parent directory",
            snapshot.0
        ))
    })?;
    fs::create_dir_all(parent)
        .map_err(|error| CoreError::Adapter(format!("failed to create store dir: {error}")))?;
    if prepared.exists() {
        return Err(CoreError::Conflict(format!(
            "prepared snapshot directory {} already exists",
            prepared.display()
        )));
    }

    let staging = parent.join(format!(
        ".{}.prepare-staging-{}",
        snapshot.0,
        unique_suffix()
    ));
    fs::create_dir(&staging).map_err(|error| {
        CoreError::Adapter(format!(
            "failed to create prepared snapshot staging directory: {error}"
        ))
    })?;
    if let Err(error) = write_snapshot_contents(&staging, snapshot, data) {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    fs::rename(&staging, &prepared).map_err(|error| {
        let _ = fs::remove_dir_all(&staging);
        CoreError::Adapter(format!("failed to finalize prepared snapshot: {error}"))
    })
}

pub(crate) fn publish_prepared_snapshot(root: &Path, snapshot: &SnapshotId) -> CoreResult<()> {
    let snapshots = root.join("snapshots");
    let prepared = snapshots.join(format!(".{}.prepared", snapshot.0));
    let target = snapshots.join(&snapshot.0);
    if !prepared.exists() {
        return Err(CoreError::SnapshotNotCommitted(format!(
            "prepared snapshot {} is missing",
            snapshot.0
        )));
    }
    if target.exists() {
        return Err(CoreError::Conflict(format!(
            "snapshot directory {} already exists",
            target.display()
        )));
    }
    fs::rename(&prepared, &target).map_err(|error| {
        CoreError::Adapter(format!(
            "failed to publish prepared snapshot atomically: {error}"
        ))
    })
}

pub(crate) fn write_snapshot_contents(
    snapshot_dir: &Path,
    snapshot: &SnapshotId,
    data: &SnapshotData,
) -> CoreResult<()> {
    write_jsonl(&snapshot_dir.join("entities.jsonl"), &data.entities)?;
    write_jsonl(&snapshot_dir.join("facts.jsonl"), &data.facts)?;
    write_jsonl(&snapshot_dir.join("relations.jsonl"), &data.relations)?;
    write_jsonl(&snapshot_dir.join("diagnostics.jsonl"), &data.diagnostics)?;
    write_indexes(
        snapshot_dir,
        snapshot,
        &data.entities,
        &data.facts,
        &data.relations,
        &data.diagnostics,
    )?;

    let manifest = json!({
        "schema": "athanor.canonical_snapshot.v1",
        "snapshot": snapshot.0,
        "entities": data.entities.len(),
        "facts": data.facts.len(),
        "relations": data.relations.len(),
        "diagnostics": data.diagnostics.len(),
    });
    let content = serde_json::to_vec_pretty(&manifest)
        .map_err(|error| CoreError::Adapter(format!("failed to serialize manifest: {error}")))?;
    fs::write(snapshot_dir.join("manifest.json"), content)
        .map_err(|error| CoreError::Adapter(format!("failed to write manifest: {error}")))
}

pub(crate) fn read_jsonl<T: serde::de::DeserializeOwned>(path: &Path) -> CoreResult<Vec<T>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let file = File::open(path)
        .map_err(|error| CoreError::Adapter(format!("failed to open JSONL file: {error}")))?;
    let mut reader = BufReader::new(file);
    let mut items = Vec::new();
    let mut line = String::new();
    loop {
        line.clear();
        let read = reader
            .read_line(&mut line)
            .map_err(|error| CoreError::Adapter(format!("failed to read JSONL line: {error}")))?;
        if read == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        items.push(
            serde_json::from_str(trimmed).map_err(|error| {
                CoreError::Adapter(format!("failed to parse JSONL item: {error}"))
            })?,
        );
    }
    Ok(items)
}

pub(crate) fn discover_next_snapshot(root: &Path) -> u64 {
    let snapshots = root.join("snapshots");
    let Ok(entries) = fs::read_dir(snapshots) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().to_str().map(str::to_string))
        .filter_map(|name| name.strip_prefix("snap_jsonl_")?.parse::<u64>().ok())
        .max()
        .unwrap_or(0)
}

fn write_jsonl<T: Serialize>(path: &Path, items: &[T]) -> CoreResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| CoreError::Adapter(format!("failed to create JSONL dir: {error}")))?;
    }
    let file = File::create(path)
        .map_err(|error| CoreError::Adapter(format!("failed to create JSONL file: {error}")))?;
    let mut writer = BufWriter::with_capacity(1024 * 1024, file);
    for item in items {
        serde_json::to_writer(&mut writer, item)
            .map_err(|error| CoreError::Adapter(format!("failed to write JSONL item: {error}")))?;
        writer.write_all(b"\n").map_err(|error| {
            CoreError::Adapter(format!("failed to write JSONL newline: {error}"))
        })?;
    }
    writer
        .flush()
        .map_err(|error| CoreError::Adapter(format!("failed to flush JSONL file: {error}")))
}
