use std::fs;
use std::path::Path;

use athanor_core::{CanonicalLatestIdentity, CoreError, CoreResult};
use athanor_domain::{GenerationId, SnapshotId};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::commit_marker::{
    SNAPSHOT_COMMIT_SCHEMA, SNAPSHOT_COMMIT_SCHEMA_V1, validate_commit_marker,
    validate_commit_marker_schema,
};
use crate::pointer_publication::publish_json;

const CANONICAL_MANIFEST_SCHEMA: &str = "athanor.canonical_snapshot.v1";
const LATEST_SCHEMA: &str = "athanor.canonical_latest.v1";

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LatestIdentityDocument {
    schema: String,
    snapshot: SnapshotId,
    generation: GenerationId,
}

pub(crate) fn write_latest_identity(root: &Path, snapshot: &SnapshotId) -> CoreResult<()> {
    let document = LatestIdentityDocument {
        schema: LATEST_SCHEMA.to_string(),
        snapshot: snapshot.clone(),
        generation: GenerationId::for_snapshot(snapshot),
    };
    publish_json(
        &root.join("latest.json"),
        &document,
        ".latest.json.staging-",
        "canonical latest pointer",
    )
}

pub(crate) fn read_latest_identity(root: &Path) -> CoreResult<Option<CanonicalLatestIdentity>> {
    let path = root.join("latest.json");
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(&path).map_err(|error| {
        CoreError::Adapter(format!(
            "failed to read canonical latest pointer {}: {error}",
            path.display()
        ))
    })?;
    let document: LatestIdentityDocument = serde_json::from_slice(&bytes).map_err(|error| {
        CoreError::AdapterProtocol(format!(
            "canonical latest pointer {} requires normalization: {error}",
            path.display()
        ))
    })?;
    if document.schema != LATEST_SCHEMA {
        return Err(CoreError::AdapterProtocol(format!(
            "canonical latest pointer {} has unsupported schema {}",
            path.display(),
            document.schema
        )));
    }
    let identity = CanonicalLatestIdentity {
        snapshot: document.snapshot,
        generation: document.generation,
    };
    identity.validate()?;
    Ok(Some(identity))
}

pub(crate) fn read_compatible_latest_snapshot(root: &Path) -> CoreResult<Option<SnapshotId>> {
    let path = root.join("latest.json");
    if !path.exists() {
        return Ok(None);
    }
    let value: Value = serde_json::from_slice(&fs::read(&path).map_err(|error| {
        CoreError::Adapter(format!(
            "failed to read latest pointer {}: {error}",
            path.display()
        ))
    })?)
    .map_err(|error| {
        CoreError::AdapterProtocol(format!(
            "failed to parse latest pointer {}: {error}",
            path.display()
        ))
    })?;
    let snapshot = value
        .get("snapshot")
        .and_then(Value::as_str)
        .map(|snapshot| SnapshotId(snapshot.to_string()))
        .ok_or_else(|| {
            CoreError::AdapterProtocol(format!(
                "latest pointer {} has no snapshot identity",
                path.display()
            ))
        })?;
    let identity = CanonicalLatestIdentity::for_snapshot(snapshot.clone());
    if let Some(schema) = value.get("schema")
        && schema.as_str() != Some(LATEST_SCHEMA)
    {
        return Err(CoreError::AdapterProtocol(format!(
            "latest pointer {} has unsupported schema",
            path.display()
        )));
    }
    if let Some(generation) = value.get("generation")
        && generation.as_str() != Some(identity.generation.as_str())
    {
        return Err(CoreError::AdapterProtocol(format!(
            "latest pointer {} generation does not match snapshot {}",
            path.display(),
            snapshot.0
        )));
    }
    Ok(Some(snapshot))
}

pub(crate) fn discover_latest_identity(
    root: &Path,
) -> CoreResult<Option<CanonicalLatestIdentity>> {
    let snapshots = root.join("snapshots");
    let entries = match fs::read_dir(&snapshots) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(CoreError::Adapter(format!(
                "failed to inspect canonical snapshots {}: {error}",
                snapshots.display()
            )));
        }
    };

    let mut latest = None;
    for entry in entries {
        let entry = entry.map_err(|error| {
            CoreError::Adapter(format!("failed to inspect canonical snapshot: {error}"))
        })?;
        if !entry
            .file_type()
            .map_err(|error| {
                CoreError::Adapter(format!("failed to inspect snapshot type: {error}"))
            })?
            .is_dir()
        {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let snapshot = SnapshotId(name);
        if latest
            .as_ref()
            .is_none_or(|current: &SnapshotId| snapshot.0 > current.0)
        {
            latest = Some(snapshot);
        }
    }

    let Some(snapshot) = latest else {
        return Ok(None);
    };
    let snapshot_dir = snapshots.join(&snapshot.0);
    validate_exact_generation(&snapshot_dir, &snapshot)?;
    validate_repair_target(&snapshot_dir, &snapshot)?;
    Ok(Some(CanonicalLatestIdentity::for_snapshot(snapshot)))
}

pub(crate) fn validate_exact_generation(
    snapshot_dir: &Path,
    snapshot: &SnapshotId,
) -> CoreResult<()> {
    let manifest_path = snapshot_dir.join("manifest.json");
    let manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).map_err(|error| {
        CoreError::Adapter(format!(
            "failed to read canonical manifest {}: {error}",
            manifest_path.display()
        ))
    })?)
    .map_err(|error| {
        CoreError::AdapterProtocol(format!(
            "failed to parse canonical manifest {}: {error}",
            manifest_path.display()
        ))
    })?;
    if manifest.get("schema").and_then(Value::as_str) != Some(CANONICAL_MANIFEST_SCHEMA) {
        return Err(CoreError::AdapterProtocol(format!(
            "canonical manifest {} has an unsupported schema",
            manifest_path.display()
        )));
    }
    if manifest.get("snapshot").and_then(Value::as_str) != Some(snapshot.0.as_str()) {
        return Err(CoreError::AdapterProtocol(format!(
            "canonical manifest {} does not identify `{}`",
            manifest_path.display(),
            snapshot.0
        )));
    }

    let marker_path = snapshot_dir.join("commit.json");
    match manifest.get("commit_marker_schema") {
        Some(Value::String(schema))
            if schema == SNAPSHOT_COMMIT_SCHEMA || schema == SNAPSHOT_COMMIT_SCHEMA_V1 =>
        {
            validate_commit_marker_schema(snapshot_dir, snapshot, schema)
        }
        Some(Value::String(schema)) => Err(CoreError::AdapterProtocol(format!(
            "canonical manifest {} requires unsupported commit marker schema `{schema}`",
            manifest_path.display()
        ))),
        Some(_) => Err(CoreError::AdapterProtocol(format!(
            "canonical manifest {} has a non-string commit_marker_schema",
            manifest_path.display()
        ))),
        None if marker_path.exists() => validate_commit_marker(snapshot_dir, snapshot),
        None => Ok(()),
    }
}

pub(crate) fn validate_repair_target(
    snapshot_dir: &Path,
    snapshot: &SnapshotId,
) -> CoreResult<()> {
    let manifest_path = snapshot_dir.join("manifest.json");
    let manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).map_err(|error| {
        CoreError::Adapter(format!(
            "failed to read canonical manifest {}: {error}",
            manifest_path.display()
        ))
    })?)
    .map_err(|error| {
        CoreError::AdapterProtocol(format!(
            "failed to parse canonical manifest {}: {error}",
            manifest_path.display()
        ))
    })?;
    if manifest.get("commit_marker_schema").and_then(Value::as_str)
        != Some(SNAPSHOT_COMMIT_SCHEMA)
    {
        return Err(CoreError::AdapterProtocol(format!(
            "latest repair target {} must declare commit marker schema {}",
            snapshot.0, SNAPSHOT_COMMIT_SCHEMA
        )));
    }
    validate_commit_marker_schema(snapshot_dir, snapshot, SNAPSHOT_COMMIT_SCHEMA)
}
