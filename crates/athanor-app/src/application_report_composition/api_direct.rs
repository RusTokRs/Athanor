use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use athanor_core::CanonicalSnapshotStore;
use athanor_domain::{EntityKind, RelationKind};

use crate::api::{ApiSnapshotOptions, ApiSnapshotReport, publish_api_contract_snapshot};
use crate::api_registry::{ApiRegistryEndpoint, ApiRegistryOptions, ApiRegistryReport};
use crate::composition::RuntimeComposition;
use crate::config::load_config;
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
    publish_api_contract_snapshot(&root, &canonical, &config.api.retention, &options.retention)
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
        for relation in &canonical.relations {
            if relation.kind == RelationKind::ImplementedBy
                && relation.from == entity.id
                && let Some(target) = entities_by_id.get(&relation.to)
            {
                handler = Some(target.stable_key.0.clone());
                break;
            }
        }
        let mut documentation = BTreeSet::new();
        for relation in &canonical.relations {
            if relation.to == entity.id
                && (relation.kind == RelationKind::Documents
                    || relation.kind == RelationKind::DocumentsApi
                    || relation.kind == RelationKind::DocumentsOperation)
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

fn canonical_root(root: &Path) -> Result<PathBuf> {
    Ok(normalize_canonical_path(root.canonicalize().with_context(
        || format!("failed to canonicalize {}", root.display()),
    )?))
}
