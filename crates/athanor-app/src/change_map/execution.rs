use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Context, Result, bail};
use athanor_core::{CanonicalSnapshot, CanonicalSnapshotStore};
use athanor_domain::Entity;

use crate::RuntimeComposition;
use crate::config::load_config;
use crate::index_state::IndexStateStore;
use crate::local_source::discover_source_files;
use crate::project_path::normalize_canonical_path;
use crate::search::search_snapshot_with_composition;

use super::evidence::entity_files;
use super::model::{ChangeMapLimits, ChangeMapOptions, ChangeMapQuery, ChangeMapReport, Seed};
use super::ranking::build_change_map;

/// Builds a bounded change map with explicitly supplied runtime dependencies.
pub async fn change_map_project_with_composition(
    options: ChangeMapOptions,
    composition: &RuntimeComposition,
) -> Result<ChangeMapReport> {
    change_map_project_inner(options, composition).await
}

async fn change_map_project_inner(
    options: ChangeMapOptions,
    composition: &RuntimeComposition,
) -> Result<ChangeMapReport> {
    validate_options(&options)?;
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
        .ok_or_else(|| anyhow::anyhow!("no canonical snapshot found; run `ath index` first"))?;

    let mut seeds = Vec::new();
    let mut changed_files = BTreeSet::new();

    if let Some(target) = options.target.as_deref() {
        let resolved = resolve_target_entities(&root, &snapshot, target);
        if resolved.is_empty() {
            bail!("could not resolve target `{target}` to a canonical stable key or source path");
        }
        seeds.extend(resolved.into_iter().map(|entity| Seed {
            id: entity.id.clone(),
            score: 1_200,
            reason: format!("explicit target `{target}`"),
        }));
    }

    if options.diff {
        let state = IndexStateStore::new(root.join(".athanor/state/index-state.json"))
            .load()
            .context("failed to load index state")?;
        let current = discover_source_files(&root)
            .context("failed to discover source files for change-map diff")?;
        let affected = state.affected_files(&current);
        changed_files.extend(affected.changed);
        changed_files.extend(affected.removed);
        for entity in snapshot.entities.iter().filter(|entity| {
            entity_files(entity)
                .iter()
                .any(|path| changed_files.contains(path))
        }) {
            seeds.push(Seed {
                id: entity.id.clone(),
                score: 1_100,
                reason: "owned by a changed or removed file".to_string(),
            });
        }
    }

    if let Some(task) = options
        .task
        .as_deref()
        .filter(|task| !task.trim().is_empty())
    {
        let search_limit = options.max_entities.saturating_mul(4).clamp(10, 200);
        let search = search_snapshot_with_composition(
            &root,
            &snapshot,
            task.to_string(),
            search_limit,
            composition,
        )
        .await?;
        for (rank, result) in search.results.into_iter().enumerate() {
            seeds.push(Seed {
                id: result.entity_id,
                score: 1_000_i64.saturating_sub((rank as i64) * 10),
                reason: format!("task search matched `{task}`"),
            });
        }
    }

    let query = ChangeMapQuery {
        task: options.task.filter(|task| !task.trim().is_empty()),
        target: options.target,
        diff: options.diff,
        changed_files: changed_files.into_iter().collect(),
    };
    Ok(build_change_map(
        &snapshot,
        query,
        seeds,
        ChangeMapLimits {
            max_entities: options.max_entities,
            max_files: options.max_files,
            max_diagnostics: options.max_diagnostics,
            max_depth: options.max_depth,
        },
    ))
}

pub(super) fn validate_options(options: &ChangeMapOptions) -> Result<()> {
    let has_task = options
        .task
        .as_ref()
        .is_some_and(|task| !task.trim().is_empty());
    if !has_task && options.target.is_none() && !options.diff {
        bail!("change-map requires a task, --target, or --diff");
    }
    if options.max_entities == 0 || options.max_files == 0 || options.max_diagnostics == 0 {
        bail!("change-map entity, file, and diagnostic limits must be greater than zero");
    }
    Ok(())
}

fn resolve_target_entities<'a>(
    root: &Path,
    snapshot: &'a CanonicalSnapshot,
    target: &str,
) -> Vec<&'a Entity> {
    let exact = snapshot
        .entities
        .iter()
        .filter(|entity| entity.stable_key.0 == target)
        .collect::<Vec<_>>();
    if !exact.is_empty() {
        return exact;
    }
    let path = normalize_target_path(root, target);
    snapshot
        .entities
        .iter()
        .filter(|entity| entity_files(entity).iter().any(|file| file == &path))
        .collect()
}

fn normalize_target_path(root: &Path, target: &str) -> String {
    let candidate = Path::new(target);
    let absolute = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        root.join(candidate)
    };
    absolute
        .canonicalize()
        .ok()
        .and_then(|path| path.strip_prefix(root).ok().map(Path::to_path_buf))
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| {
            target
                .replace('\\', "/")
                .trim_start_matches("./")
                .to_string()
        })
}
