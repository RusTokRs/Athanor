use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result, bail};
use athanor_core::CanonicalSnapshotStore;
use athanor_domain::Entity;

use crate::RuntimeComposition;
use crate::config::load_config;
use crate::index_state::IndexStateStore;
use crate::json_contract::IMPACT_ANALYSIS_SCHEMA_V1;
use crate::local_source::discover_source_files;
use crate::project_path::normalize_canonical_path;

use super::model::{ImpactAnalysis, ImpactOptions};
use super::traversal::impact_snapshot;

/// Analyses impact with explicitly supplied runtime dependencies.
pub async fn impact_project_with_composition(
    options: ImpactOptions,
    composition: &RuntimeComposition,
) -> Result<ImpactAnalysis> {
    let root = normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );

    let config = load_config(&root)?;
    let store = composition.init_store(&root, &config).await?;
    let snapshot = store
        .load_latest_snapshot()
        .await
        .context("failed to load latest canonical snapshot")?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "no canonical snapshot found; run `ath index {}` first",
                root.display()
            )
        })?;

    let starting_entities = if options.diff {
        let state_store = IndexStateStore::new(root.join(".athanor/state/index-state.json"));
        let previous_state = state_store.load().context("failed to load index state")?;
        let current_files = discover_source_files(&root)
            .context("failed to discover source files for diff comparison")?;
        let affected_files = previous_state.affected_files(&current_files);

        let mut diff_files = HashSet::new();
        diff_files.extend(affected_files.changed);
        diff_files.extend(affected_files.removed);

        if diff_files.is_empty() {
            println!(
                "No changed files detected in the working tree compared to the last index run."
            );
            return Ok(ImpactAnalysis {
                schema: IMPACT_ANALYSIS_SCHEMA_V1.to_string(),
                snapshot: snapshot
                    .snapshot
                    .as_ref()
                    .map_or_else(|| "unknown".to_string(), |snapshot| snapshot.0.clone()),
                starting_entities: Vec::new(),
                impacted_entities: Vec::new(),
                impacted_files: Vec::new(),
                impacted_diagnostics: Vec::new(),
            });
        }

        snapshot
            .entities
            .iter()
            .filter(|entity| {
                entity
                    .ownership
                    .iter()
                    .any(|ownership| diff_files.contains(&ownership.source_file))
                    || entity
                        .source
                        .as_ref()
                        .is_some_and(|source| diff_files.contains(&source.path))
            })
            .cloned()
            .collect::<Vec<_>>()
    } else if let Some(target) = &options.target {
        resolve_target_entities(&root, &snapshot.entities, target)?
    } else {
        bail!("either a target stable-key/path or the --diff flag must be provided");
    };

    Ok(impact_snapshot(
        &snapshot,
        starting_entities,
        options.max_depth,
    ))
}

fn resolve_target_entities(root: &Path, entities: &[Entity], target: &str) -> Result<Vec<Entity>> {
    let mut resolved = entities
        .iter()
        .filter(|entity| entity.stable_key.0 == target)
        .cloned()
        .collect::<Vec<_>>();

    if resolved.is_empty()
        && let Some(relative_path) = resolve_project_relative_path(root, target)
    {
        resolved = entities
            .iter()
            .filter(|entity| {
                entity
                    .ownership
                    .iter()
                    .any(|ownership| ownership.source_file == relative_path)
                    || entity
                        .source
                        .as_ref()
                        .is_some_and(|source| source.path == relative_path)
            })
            .cloned()
            .collect();
    }

    if resolved.is_empty() {
        bail!(
            "could not resolve target \"{}\" to any canonical entities in the latest snapshot",
            target
        );
    }

    Ok(resolved)
}

fn resolve_project_relative_path(root: &Path, target: &str) -> Option<String> {
    let target_path = Path::new(target);
    let absolute_target = if target_path.is_absolute() {
        target_path.to_path_buf()
    } else {
        root.join(target_path)
    };

    if let Some(relative) = absolute_target
        .canonicalize()
        .ok()
        .and_then(|canonical| canonical.strip_prefix(root).ok().map(Path::to_path_buf))
    {
        return Some(relative.to_string_lossy().replace('\\', "/"));
    }

    let normalized = target.replace('\\', "/");
    let trimmed = normalized.strip_prefix("./").unwrap_or(&normalized);
    Some(trimmed.to_string())
}
