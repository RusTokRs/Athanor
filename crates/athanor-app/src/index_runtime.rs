use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use athanor_core::{
    CanonicalSnapshotStore, CoreError, OperationContext, PreparedSnapshot,
    PreparedSnapshotPublication,
};
use athanor_domain::{RepoId, SnapshotBase};
use fs2::FileExt;
use serde::Serialize;

use crate::hash::stable_hash;
use crate::index_publication::{
    IndexPublicationOutcome, publish_prepared_index, recover_interrupted_publication,
};
use crate::project_path::normalize_canonical_path;
use crate::transient_store::TransientKnowledgeStore;
use crate::{
    AdapterValidationReport, AthanorStore, CancellationToken, IncrementalIndexContext,
    IndexPipelineMetrics, IndexStateStore, RuntimeBuilder, RuntimeComposition, config::load_config,
    store::init_store,
};

#[derive(Debug, Clone)]
pub struct IndexOptions {
    pub root: PathBuf,
    pub validation_report: Option<PathBuf>,
    pub validation_result: Option<PathBuf>,
    pub validate_only: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexReport {
    pub root: PathBuf,
    pub snapshot: String,
    pub files_indexed: usize,
    pub output_dir: PathBuf,
    pub changed_files: usize,
    pub unchanged_files: usize,
    pub removed_files: usize,
    pub validation_report: PathBuf,
    pub validation_result: Option<PathBuf>,
    pub validate_only: bool,
    pub metrics: IndexReportMetrics,
}

pub const VALIDATION_RESULT_SCHEMA: &str = "athanor.validation_result.v1";
pub const INDEX_REPORT_METRICS_SCHEMA: &str = "athanor.index_report_metrics.v1";

#[derive(Debug, Clone, Serialize)]
pub struct IndexReportMetrics {
    pub schema: &'static str,
    pub total_ms: u64,
    pub pipeline: IndexPipelineMetrics,
    pub read_model_write_ms: u64,
    pub validation_result_write_ms: u64,
    pub index_state_write_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
struct ValidationResultFile<'a> {
    schema: &'static str,
    status: &'static str,
    snapshot: &'a str,
    files_indexed: usize,
    changed_files: usize,
    unchanged_files: usize,
    removed_files: usize,
    entities: usize,
    facts: usize,
    relations: usize,
    diagnostics: usize,
}

pub async fn index_project(options: IndexOptions) -> Result<IndexReport> {
    index_project_inner(options, None, None, None).await
}

/// Indexes with explicit transport-neutral operation metadata propagated to all pipeline ports.
pub async fn index_project_with_operation_context(
    options: IndexOptions,
    operation: OperationContext,
) -> Result<IndexReport> {
    index_project_inner(options, None, None, Some(operation)).await
}

/// Indexes with dependencies supplied by an explicit application composition.
pub async fn index_project_with_composition(
    options: IndexOptions,
    composition: &RuntimeComposition,
) -> Result<IndexReport> {
    index_project_inner(options, None, Some(composition), None).await
}

pub async fn index_project_cancellable(
    options: IndexOptions,
    cancellation: CancellationToken,
) -> Result<IndexReport> {
    index_project_inner(options, Some(cancellation), None, None).await
}

/// Cancellable indexing with explicit transport-neutral operation metadata.
pub async fn index_project_cancellable_with_operation_context(
    options: IndexOptions,
    cancellation: CancellationToken,
    operation: OperationContext,
) -> Result<IndexReport> {
    index_project_inner(options, Some(cancellation), None, Some(operation)).await
}

/// Cancellable variant of [`index_project_with_composition`].
pub async fn index_project_cancellable_with_composition(
    options: IndexOptions,
    cancellation: CancellationToken,
    composition: &RuntimeComposition,
) -> Result<IndexReport> {
    index_project_inner(options, Some(cancellation), Some(composition), None).await
}

/// Cancellable composition-aware indexing with explicit operation metadata.
pub async fn index_project_cancellable_with_composition_and_operation_context(
    options: IndexOptions,
    cancellation: CancellationToken,
    composition: &RuntimeComposition,
    operation: OperationContext,
) -> Result<IndexReport> {
    index_project_inner(
        options,
        Some(cancellation),
        Some(composition),
        Some(operation),
    )
    .await
}

async fn index_project_inner(
    options: IndexOptions,
    cancellation: Option<CancellationToken>,
    composition: Option<&RuntimeComposition>,
    operation: Option<OperationContext>,
) -> Result<IndexReport> {
    let operation = operation.unwrap_or_else(|| OperationContext::new("index"));
    let index_started = Instant::now();
    let root = normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );
    let _publication_lock =
        ProjectPublicationLock::acquire(root.join(".athanor/state/index-publication.lock"))?;

    let state_store = IndexStateStore::new(root.join(".athanor/state/index-state.json"));
    let output_dir = root.join(".athanor/generated/current/jsonl");
    let validation_report = validation_report_path(&root, options.validation_report.as_deref());
    let validation_result = validation_result_path(&root, options.validation_result.as_deref());
    let config = load_config(&root)?;
    let canonical_store = match composition {
        Some(composition) => composition.init_store(&root, &config).await?,
        None => init_store(&root, &config).await?,
    };
    recover_interrupted_publication(&root, &canonical_store).await?;
    let previous_state = state_store.load().context("failed to load index state")?;
    let previous_snapshot = match previous_state.snapshot.as_ref() {
        Some(snapshot) => canonical_store
            .load_snapshot(&athanor_domain::SnapshotId(snapshot.clone()))
            .await
            .context("failed to load previous canonical snapshot")?,
        None => None,
    };

    let output_result = if options.validate_only {
        let pipeline = runtime_builder(&root, composition)
            .allow_external_process(config.adapters.allow_external_process)
            .clear_external_process_environment(matches!(
                config.adapters.external_process_sandbox,
                crate::config::ExternalProcessSandboxProfile::CleanEnvironment
            ))
            .allowed_external_process_programs(
                config
                    .adapters
                    .external_process_allowlist
                    .iter()
                    .map(PathBuf::from),
            )
            .with_discovered_plugins()
            .context("failed to discover adapter plugins")?
            .build_index_pipeline(TransientKnowledgeStore::new())
            .extraction_concurrency(config.pipeline.extraction_concurrency)
            .max_extraction_bytes_in_flight(config.pipeline.max_extraction_bytes_in_flight)
            .max_snapshot_batch_objects(config.pipeline.max_snapshot_batch_objects)
            .max_snapshot_batch_bytes(config.pipeline.max_snapshot_batch_bytes)
            .extraction_concurrency_by_adapter(
                config.pipeline.extraction_concurrency_by_adapter.clone(),
            );
        if let Some(cancellation) = cancellation.clone() {
            pipeline
                .run_with_incremental_cancellable_operation_context(
                    RepoId(repo_id_for_root(&root)),
                    SnapshotBase {
                        branch: None,
                        commit: None,
                        parent_snapshot: None,
                        working_tree: true,
                    },
                    IncrementalIndexContext {
                        previous_state: previous_state.clone(),
                        previous_snapshot,
                    },
                    operation.clone(),
                    cancellation,
                )
                .await
        } else {
            pipeline
                .run_with_incremental_operation_context(
                    RepoId(repo_id_for_root(&root)),
                    SnapshotBase {
                        branch: None,
                        commit: None,
                        parent_snapshot: None,
                        working_tree: true,
                    },
                    IncrementalIndexContext {
                        previous_state: previous_state.clone(),
                        previous_snapshot,
                    },
                    operation.clone(),
                )
                .await
        }
    } else {
        let pipeline = runtime_builder(&root, composition)
            .allow_external_process(config.adapters.allow_external_process)
            .clear_external_process_environment(matches!(
                config.adapters.external_process_sandbox,
                crate::config::ExternalProcessSandboxProfile::CleanEnvironment
            ))
            .allowed_external_process_programs(
                config
                    .adapters
                    .external_process_allowlist
                    .iter()
                    .map(PathBuf::from),
            )
            .with_discovered_plugins()
            .context("failed to discover adapter plugins")?
            .build_index_pipeline(canonical_store.clone())
            .extraction_concurrency(config.pipeline.extraction_concurrency)
            .max_extraction_bytes_in_flight(config.pipeline.max_extraction_bytes_in_flight)
            .max_snapshot_batch_objects(config.pipeline.max_snapshot_batch_objects)
            .max_snapshot_batch_bytes(config.pipeline.max_snapshot_batch_bytes)
            .extraction_concurrency_by_adapter(
                config.pipeline.extraction_concurrency_by_adapter.clone(),
            );
        if let Some(cancellation) = cancellation.clone() {
            pipeline
                .run_with_incremental_cancellable_deferred_operation_context(
                    RepoId(repo_id_for_root(&root)),
                    SnapshotBase {
                        branch: None,
                        commit: None,
                        parent_snapshot: None,
                        working_tree: true,
                    },
                    IncrementalIndexContext {
                        previous_state: previous_state.clone(),
                        previous_snapshot,
                    },
                    operation.clone(),
                    cancellation,
                )
                .await
        } else {
            pipeline
                .run_with_incremental_deferred_operation_context(
                    RepoId(repo_id_for_root(&root)),
                    SnapshotBase {
                        branch: None,
                        commit: None,
                        parent_snapshot: None,
                        working_tree: true,
                    },
                    IncrementalIndexContext {
                        previous_state: previous_state.clone(),
                        previous_snapshot,
                    },
                    operation.clone(),
                )
                .await
        }
    };
    let output = match output_result {
        Ok(output) => {
            remove_validation_report(&validation_report)
                .context("failed to remove stale validation report")?;
            output
        }
        Err(error) => {
            remove_validation_result(&validation_result)
                .context("failed to remove stale validation result")?;

            if let Some(report) = error.downcast_ref::<AdapterValidationReport>() {
                write_validation_report(&validation_report, report)
                    .context("failed to write validation report")?;
            }

            return Err(error).context("failed to run index pipeline");
        }
    };

    if options.validate_only {
        let validation_started = Instant::now();
        write_validation_result(&validation_result, &output)
            .context("failed to write validation result")?;
        let validation_result_write_ms = elapsed_ms(validation_started.elapsed());
        let metrics = IndexReportMetrics {
            schema: INDEX_REPORT_METRICS_SCHEMA,
            total_ms: elapsed_ms(index_started.elapsed()),
            pipeline: output.metrics.clone(),
            read_model_write_ms: 0,
            validation_result_write_ms,
            index_state_write_ms: 0,
        };

        return Ok(IndexReport {
            root,
            snapshot: output.snapshot.0,
            files_indexed: output.files.len(),
            output_dir,
            changed_files: output.affected_files.changed.len(),
            unchanged_files: output.affected_files.unchanged.len(),
            removed_files: output.affected_files.removed.len(),
            validation_report,
            validation_result: Some(validation_result),
            validate_only: true,
            metrics,
        });
    }

    // The incremental pipeline deliberately reuses the previous committed snapshot when source
    // discovery finds no changes. There is no prepared snapshot to publish in that case.
    if previous_state.snapshot.as_deref() == Some(output.snapshot.0.as_str())
        && output.affected_files.changed.is_empty()
        && output.affected_files.removed.is_empty()
    {
        let metrics = IndexReportMetrics {
            schema: INDEX_REPORT_METRICS_SCHEMA,
            total_ms: elapsed_ms(index_started.elapsed()),
            pipeline: output.metrics.clone(),
            read_model_write_ms: 0,
            validation_result_write_ms: 0,
            index_state_write_ms: 0,
        };
        return Ok(IndexReport {
            root,
            snapshot: output.snapshot.0,
            files_indexed: output.files.len(),
            output_dir,
            changed_files: output.affected_files.changed.len(),
            unchanged_files: output.affected_files.unchanged.len(),
            removed_files: output.affected_files.removed.len(),
            validation_report,
            validation_result: None,
            validate_only: false,
            metrics,
        });
    }

    remove_validation_result(&validation_result)
        .context("failed to remove stale validation result")?;

    let prepared = PreparedSnapshot::new(output.snapshot.clone());
    let publication = publish_prepared_index_with_cleanup(
        &root,
        &canonical_store,
        &state_store,
        &output_dir,
        &output,
        prepared,
        &operation,
    )
    .await?;
    let metrics = IndexReportMetrics {
        schema: INDEX_REPORT_METRICS_SCHEMA,
        total_ms: elapsed_ms(index_started.elapsed()),
        pipeline: output.metrics.clone(),
        read_model_write_ms: publication.read_model_write_ms,
        validation_result_write_ms: 0,
        index_state_write_ms: publication.index_state_write_ms,
    };

    Ok(IndexReport {
        root,
        snapshot: output.snapshot.0,
        files_indexed: output.files.len(),
        output_dir: publication.read_model.output_dir,
        changed_files: publication.read_model.changed_files,
        unchanged_files: publication.read_model.unchanged_files,
        removed_files: publication.read_model.removed_files,
        validation_report,
        validation_result: None,
        validate_only: false,
        metrics,
    })
}

async fn publish_prepared_index_with_cleanup(
    root: &Path,
    store: &AthanorStore,
    state_store: &IndexStateStore,
    output_dir: &Path,
    output: &crate::IndexPipelineOutput,
    prepared: PreparedSnapshot,
    operation: &OperationContext,
) -> Result<IndexPublicationOutcome> {
    match publish_prepared_index(
        root,
        store,
        state_store,
        output_dir,
        output,
        prepared.clone(),
        operation,
    )
    .await
    {
        Ok(publication) => Ok(publication),
        Err(error) => {
            let latest = match store.load_latest_snapshot().await {
                Ok(latest) => latest,
                Err(status_error) => {
                    return Err(error.context(format!(
                        "failed to determine publication state after coordinator error: {status_error}"
                    )));
                }
            };
            let committed = latest
                .and_then(|snapshot| snapshot.snapshot)
                .is_some_and(|snapshot| snapshot == *prepared.snapshot());
            if committed {
                return Err(error);
            }

            match store.abort_prepared(&prepared).await {
                Ok(()) | Err(CoreError::NotFound(_)) => Err(error),
                Err(abort_error) => Err(error.context(format!(
                    "failed to abort prepared snapshot {} after coordinator error: {abort_error}",
                    prepared.snapshot().0
                ))),
            }
        }
    }
}

fn runtime_builder(root: &Path, composition: Option<&RuntimeComposition>) -> RuntimeBuilder {
    match composition {
        Some(composition) => RuntimeBuilder::from_composition(root, composition),
        None => RuntimeBuilder::new(root),
    }
}

/// An OS-level lock covering source-state classification, canonical writes, and publication.
///
/// JSONL storage has its own writer lock, but the application owns several artefacts that must
/// advance together. Keeping this lock for the full index call prevents two processes from
/// interleaving their staged read models and index states.
struct ProjectPublicationLock {
    _file: File,
}

impl ProjectPublicationLock {
    fn acquire(path: PathBuf) -> Result<Self> {
        let parent = path.parent().ok_or_else(|| {
            anyhow::anyhow!("publication lock path has no parent: {}", path.display())
        })?;
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create publication lock directory {}",
                parent.display()
            )
        })?;
        let file = File::create(&path)
            .with_context(|| format!("failed to open publication lock {}", path.display()))?;
        file.lock_exclusive()
            .with_context(|| format!("failed to acquire publication lock {}", path.display()))?;
        Ok(Self { _file: file })
    }
}

fn elapsed_ms(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

fn validation_report_path(root: &Path, configured: Option<&Path>) -> PathBuf {
    configured
        .map(Path::to_path_buf)
        .unwrap_or_else(|| root.join(".athanor/generated/current/validation-report.json"))
}

fn validation_result_path(root: &Path, configured: Option<&Path>) -> PathBuf {
    configured
        .map(Path::to_path_buf)
        .unwrap_or_else(|| root.join(".athanor/generated/current/validation-result.json"))
}

fn write_validation_report(path: &Path, report: &AdapterValidationReport) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    fs::write(path, serde_json::to_string_pretty(report)?)
        .with_context(|| format!("failed to write {}", path.display()))
}

fn write_validation_result(path: &Path, output: &crate::IndexPipelineOutput) -> Result<()> {
    let result = ValidationResultFile {
        schema: VALIDATION_RESULT_SCHEMA,
        status: "passed",
        snapshot: &output.snapshot.0,
        files_indexed: output.files.len(),
        changed_files: output.affected_files.changed.len(),
        unchanged_files: output.affected_files.unchanged.len(),
        removed_files: output.affected_files.removed.len(),
        entities: output.entities.len(),
        facts: output.facts.len(),
        relations: output.relations.len(),
        diagnostics: output.diagnostics.len(),
    };

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    fs::write(path, serde_json::to_string_pretty(&result)?)
        .with_context(|| format!("failed to write {}", path.display()))
}

fn remove_validation_report(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_file(path).with_context(|| format!("failed to remove {}", path.display()))?;
    }

    Ok(())
}

fn remove_validation_result(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_file(path).with_context(|| format!("failed to remove {}", path.display()))?;
    }

    Ok(())
}

pub(crate) fn repo_id_for_root(root: &Path) -> String {
    format!(
        "repo_{:016x}",
        stable_hash(root.to_string_lossy().as_bytes())
    )
}
