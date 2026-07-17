use std::collections::{BTreeSet, HashMap};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use athanor_core::{CanonicalSnapshot, CanonicalSnapshotStore};
use athanor_domain::{Entity, EntityKind, RelationKind};
use athanor_projector_support::replace_output_file;

use crate::api::{
    API_CONTRACT_LATEST_SCHEMA, API_CONTRACT_SNAPSHOT_SCHEMA, ApiCleanupOptions,
    ApiCleanupReport, ApiContractItem, ApiContractLatest, ApiContractSnapshot,
    ApiRetentionOverrides, ApiSnapshotOptions, ApiSnapshotReport, cleanup_api_contracts,
};
use crate::api_registry::{ApiRegistryEndpoint, ApiRegistryOptions, ApiRegistryReport};
use crate::composition::RuntimeComposition;
use crate::config::{ApiRetentionConfig, load_config};
use crate::project_path::normalize_canonical_path;

pub(super) async fn snapshot(
    options: ApiSnapshotOptions,
    composition: &RuntimeComposition,
) -> Result<ApiSnapshotReport> {
    let root = canonical_root(&options.root)?;
    let config = load_config(&root)?;
    let store = composition.init_store(&root, &config).await?;
    let canonical = store
        .load_latest_snapshot()
        .await
        .context("failed to load latest canonical snapshot")?
        .ok_or_else(|| anyhow::anyhow!("no canonical snapshot found; run `ath index` first"))?;
    let contract = build_contract(&canonical)?;
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
    let cleanup = maybe_cleanup(&root, &config.api.retention, &options.retention)?;

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

pub(super) async fn registry(
    options: ApiRegistryOptions,
    composition: &RuntimeComposition,
) -> Result<ApiRegistryReport> {
    let root = canonical_root(&options.root)?;
    let config = load_config(&root)?;
    let store = composition.init_store(&root, &config).await?;
    let canonical = store
        .load_latest_snapshot()
        .await
        .context("failed to load latest canonical snapshot")?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "no canonical snapshot found; run `ath index {}` first",
                root.display()
            )
        })?;
    let snapshot = canonical
        .snapshot
        .as_ref()
        .map_or_else(|| "unknown".to_string(), |snapshot| snapshot.0.clone());
    let entities_by_id = canonical
        .entities
        .iter()
        .map(|entity| (&entity.id, entity))
        .collect::<HashMap<_, _>>();
    let mut endpoints = Vec::new();

    for entity in &canonical.entities {
        if entity.kind != EntityKind::ApiEndpoint {
            continue;
        }
        let mut handler = None;
        let mut documentation = BTreeSet::new();
        for relation in &canonical.relations {
            if relation.kind == RelationKind::ImplementedBy
                && relation.from == entity.id
                && let Some(target) = entities_by_id.get(&relation.to)
            {
                handler = Some(target.stable_key.0.clone());
            }
            if relation.to == entity.id
                && matches!(
                    relation.kind,
                    RelationKind::Documents
                        | RelationKind::DocumentsApi
                        | RelationKind::DocumentsOperation
                )
                && let Some(document) = entities_by_id.get(&relation.from)
            {
                documentation.insert(document.stable_key.0.clone());
            }
        }
        endpoints.push(ApiRegistryEndpoint {
            stable_key: entity.stable_key.0.clone(),
            method: entity.payload["method"]
                .as_str()
                .unwrap_or("UNKNOWN")
                .to_string(),
            path: entity.payload["path"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            operation_id: entity.payload["operation_id"].as_str().map(str::to_string),
            summary: entity.payload["summary"].as_str().map(str::to_string),
            handler,
            documentation: documentation.into_iter().collect(),
        });
    }
    endpoints.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
    Ok(ApiRegistryReport {
        schema: crate::api_registry::API_REGISTRY_SCHEMA.to_string(),
        snapshot,
        endpoints,
    })
}

fn build_contract(canonical: &CanonicalSnapshot) -> Result<ApiContractSnapshot> {
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
            file.write_all(content.as_bytes())?;
            file.write_all(b"\n")?;
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

fn maybe_cleanup(
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

fn canonical_root(root: &Path) -> Result<PathBuf> {
    Ok(normalize_canonical_path(root.canonicalize().with_context(
        || format!("failed to canonicalize {}", root.display()),
    )?))
}
