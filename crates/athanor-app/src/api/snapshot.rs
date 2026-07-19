use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result, bail};
use athanor_core::CanonicalSnapshot;
use athanor_domain::{Entity, EntityKind};
use athanor_projector_support::replace_output_file;

use crate::config::ApiRetentionConfig;

use super::model::{
    API_CONTRACT_LATEST_SCHEMA, API_CONTRACT_SNAPSHOT_SCHEMA, ApiContractItem, ApiContractLatest,
    ApiContractSnapshot, ApiRetentionOverrides, ApiSnapshotReport,
};
use super::retention::maybe_cleanup_api_contracts;

pub(crate) fn publish_api_contract_snapshot(
    root: &Path,
    canonical: &CanonicalSnapshot,
    retention_config: &ApiRetentionConfig,
    retention_overrides: &ApiRetentionOverrides,
) -> Result<ApiSnapshotReport> {
    let contract = build_api_contract_snapshot(canonical)?;
    let api_root = root.join(".athanor/api");
    let snapshots_dir = api_root.join("snapshots");
    fs::create_dir_all(&snapshots_dir).with_context(|| {
        format!(
            "failed to create API snapshot dir {}",
            snapshots_dir.display()
        )
    })?;
    let path = snapshots_dir.join(format!("{}.json", contract.snapshot));
    let serialized = serde_json::to_string_pretty(&contract)
        .context("failed to serialize API contract snapshot")?;
    let created = write_immutable(&path, &serialized)?;
    let pointer = ApiContractLatest {
        schema: API_CONTRACT_LATEST_SCHEMA.to_string(),
        snapshot: contract.snapshot.clone(),
        path: format!("snapshots/{}.json", contract.snapshot),
    };
    replace_output_file(
        &api_root.join("latest.json"),
        &serde_json::to_string_pretty(&pointer)
            .context("failed to serialize API contract pointer")?,
        "API contract pointer",
    )
    .context("failed to update API contract pointer")?;
    let cleanup = maybe_cleanup_api_contracts(root, retention_config, retention_overrides)?;

    Ok(ApiSnapshotReport {
        snapshot: contract.snapshot,
        path,
        created,
        endpoints: contract.endpoints.len(),
        schemas: contract.schemas.len(),
        examples: contract.examples.len(),
        cleanup,
    })
}

pub(super) fn build_api_contract_snapshot(
    canonical: &CanonicalSnapshot,
) -> Result<ApiContractSnapshot> {
    let snapshot = canonical
        .snapshot
        .as_ref()
        .context("canonical snapshot has no snapshot id")?
        .0
        .clone();
    Ok(ApiContractSnapshot {
        schema: API_CONTRACT_SNAPSHOT_SCHEMA.to_string(),
        snapshot,
        endpoints: contract_items(&canonical.entities, EntityKind::ApiEndpoint),
        schemas: contract_items(&canonical.entities, EntityKind::ApiSchema),
        examples: contract_items(&canonical.entities, EntityKind::ApiExample),
    })
}

fn contract_items(entities: &[Entity], kind: EntityKind) -> Vec<ApiContractItem> {
    let mut items = entities
        .iter()
        .filter(|entity| entity.kind == kind)
        .map(|entity| ApiContractItem {
            entity_id: Some(entity.id.clone()),
            stable_key: entity.stable_key.0.clone(),
            name: entity.name.clone(),
            source: entity.source.clone(),
            ownership: entity.ownership.clone(),
            payload: entity.payload.clone(),
        })
        .collect::<Vec<_>>();
    items.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
    items
}

fn write_immutable(path: &Path, content: &str) -> Result<bool> {
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file) => {
            file.write_all(content.as_bytes())
                .with_context(|| format!("failed to write {}", path.display()))?;
            file.write_all(b"\n")
                .with_context(|| format!("failed to finish {}", path.display()))?;
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let existing: ApiContractSnapshot = serde_json::from_str(
                &fs::read_to_string(path)
                    .with_context(|| format!("failed to read {}", path.display()))?,
            )
            .with_context(|| format!("failed to parse {}", path.display()))?;
            let expected: ApiContractSnapshot = serde_json::from_str(content)
                .context("failed to parse generated API contract snapshot")?;
            if existing != expected {
                bail!(
                    "immutable API snapshot {} has conflicting content",
                    path.display()
                );
            }
            Ok(false)
        }
        Err(error) => Err(error).with_context(|| format!("failed to create {}", path.display())),
    }
}
