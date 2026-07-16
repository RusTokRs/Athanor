use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use athanor_core::{
    CanonicalSnapshot, CanonicalSnapshotStore, CanonicalSnapshotStoreOperationExt, CoreError,
    OperationContext, OperationContextCancellation, SearchIndex, SearchIndexOperationExt, SearchQuery,
};
use athanor_domain::{ContextPack, EntityId};
use serde_json::json;

use crate::config::load_config;
use crate::context::{ContextLimits, ContextOptions, generate_context_pack};
use crate::index_state::IndexStateStore;
use crate::local_source::discover_source_files_with_operation_context;
use crate::project_path::normalize_canonical_path;
use crate::search::{
    get_or_build_search_index_with_factory_and_operation,
    get_or_build_search_index_with_operation_context,
};
use crate::store::init_store;
use crate::RuntimeComposition;

pub(crate) async fn context_project_with_operation_context_impl(
    options: ContextOptions,
    composition: Option<&RuntimeComposition>,
    operation: &OperationContext,
) -> Result<ContextPack> {
    operation.check_active().map_err(anyhow::Error::new)?;
    if options.task.trim().is_empty() && !options.diff {
        bail!("context task must not be empty");
    }

    let root = normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );
    let config = load_config(&root)?;
    let store = match composition {
        Some(composition) => composition.init_store(&root, &config).await?,
        None => init_store(&root, &config).await?,
    };
    let snapshot = store
        .load_latest_snapshot_with_operation_context(operation)
        .await
        .context("failed to load latest canonical snapshot")?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "no canonical snapshot found; run `ath index {}` first",
                root.display()
            )
        })?;

    let mut limits = ContextLimits::for_level(options.level);
    options.limits.apply(&mut limits);
    if limits.max_tokens == 0 || limits.max_files == 0 || limits.max_entities == 0 {
        bail!("context token, file, and entity limits must be greater than zero");
    }

    let pack = if options.diff {
        build_diff_context(&root, &snapshot, &options, limits, operation)?
    } else {
        let direct_matches = search_direct_matches(
            &root,
            &snapshot,
            &options.task,
            limits.max_entities,
            composition,
            operation,
        )
        .await?;
        generate_context_pack(
            &snapshot,
            &options.task,
            options.level,
            limits,
            direct_matches,
        )
    };
    operation.check_active().map_err(anyhow::Error::new)?;
    Ok(pack)
}

fn build_diff_context(
    root: &std::path::Path,
    snapshot: &CanonicalSnapshot,
    options: &ContextOptions,
    limits: ContextLimits,
    operation: &OperationContext,
) -> Result<ContextPack> {
    let state_store = IndexStateStore::new(root.join(".athanor/state/index-state.json"));
    let previous_state = state_store.load().context("failed to load index state")?;
    let current_files = discover_source_files_with_operation_context(root, operation)
        .context("failed to discover source files for diff context")?;
    let affected_files = previous_state.affected_files(&current_files);
    let affected_paths = affected_files
        .changed
        .iter()
        .chain(affected_files.removed.iter())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut direct_matches = snapshot
        .entities
        .iter()
        .filter(|entity| {
            entity
                .ownership
                .iter()
                .any(|ownership| affected_paths.contains(&ownership.source_file))
                || entity
                    .source
                    .as_ref()
                    .is_some_and(|source| affected_paths.contains(&source.path))
        })
        .map(|entity| entity.id.clone())
        .collect::<Vec<_>>();
    if direct_matches.is_empty() {
        direct_matches.push(EntityId("__athanor_no_diff_matches__".to_string()));
    }
    let task = if options.task.trim().is_empty() {
        "changed files".to_string()
    } else {
        options.task.clone()
    };
    let mut pack = generate_context_pack(
        snapshot,
        &task,
        options.level,
        limits,
        Some(direct_matches),
    );
    if let Some(payload) = pack.payload.as_object_mut() {
        payload.insert(
            "diff".to_string(),
            json!({
                "changed_files": affected_files.changed.len(),
                "unchanged_files": affected_files.unchanged.len(),
                "removed_files": affected_files.removed.len(),
            }),
        );
    }
    operation.check_active().map_err(anyhow::Error::new)?;
    Ok(pack)
}

async fn search_direct_matches(
    root: &std::path::Path,
    snapshot: &CanonicalSnapshot,
    task: &str,
    limit: usize,
    composition: Option<&RuntimeComposition>,
    operation: &OperationContext,
) -> Result<Option<Vec<EntityId>>> {
    let snapshot_id = snapshot
        .snapshot
        .as_ref()
        .map(|snapshot| snapshot.0.clone())
        .unwrap_or_else(|| "unknown".to_string());
    let index_dir = root.join(".athanor/generated/current/search");
    let index = build_search_index(
        snapshot,
        snapshot_id,
        index_dir,
        composition.cloned(),
        operation.clone(),
    )
    .await;
    let index = match index {
        Ok(index) => index,
        Err(error) if is_operation_termination(&error) => return Err(error),
        Err(_) => return Ok(None),
    };
    match index
        .search_with_operation_context(
            SearchQuery {
                query: task.to_string(),
                limit,
            },
            operation,
        )
        .await
    {
        Ok(results) => Ok(Some(
            results
                .into_iter()
                .map(|result| EntityId(result.id))
                .collect(),
        )),
        Err(error @ (CoreError::Cancelled(_) | CoreError::DeadlineExceeded(_))) => {
            Err(anyhow::Error::new(error))
        }
        Err(_) => Ok(None),
    }
}

async fn build_search_index(
    snapshot: &CanonicalSnapshot,
    snapshot_id: String,
    index_dir: PathBuf,
    composition: Option<RuntimeComposition>,
    operation: OperationContext,
) -> Result<Arc<dyn SearchIndex>> {
    let snapshot = snapshot.clone();
    tokio::task::spawn_blocking(move || match composition {
        Some(composition) => get_or_build_search_index_with_factory_and_operation(
            &snapshot,
            &snapshot_id,
            &index_dir,
            &operation,
            |directory, documents, operation| {
                composition.build_search_index_with_operation_context(
                    directory,
                    documents,
                    operation,
                )
            },
        ),
        None => get_or_build_search_index_with_operation_context(
            &snapshot,
            &snapshot_id,
            &index_dir,
            &operation,
        ),
    })
    .await
    .context("context search-index worker terminated unexpectedly")?
}

fn is_operation_termination(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause.downcast_ref::<CoreError>().is_some_and(|error| {
            matches!(error, CoreError::Cancelled(_) | CoreError::DeadlineExceeded(_))
        })
    })
}
