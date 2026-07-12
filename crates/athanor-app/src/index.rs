use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use athanor_core::{CanonicalSnapshotStore, KnowledgeStore};
use athanor_domain::{RepoId, SnapshotBase};
use fs2::FileExt;
use serde::{Deserialize, Serialize};

use crate::hash::stable_hash;
use crate::project_path::normalize_canonical_path;
use crate::transient_store::TransientKnowledgeStore;
use crate::{
    AdapterValidationReport, CancellationToken, IncrementalIndexContext, IndexPipelineMetrics,
    IndexState, IndexStateStore, JsonlReadModelWriter, RuntimeBuilder, RuntimeComposition,
    config::load_config, store::init_store,
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
    index_project_inner(options, None, None).await
}

/// Indexes with dependencies supplied by an explicit application composition.
pub async fn index_project_with_composition(
    options: IndexOptions,
    composition: &RuntimeComposition,
) -> Result<IndexReport> {
    index_project_inner(options, None, Some(composition)).await
}

pub async fn index_project_cancellable(
    options: IndexOptions,
    cancellation: CancellationToken,
) -> Result<IndexReport> {
    index_project_inner(options, Some(cancellation), None).await
}

/// Cancellable variant of [`index_project_with_composition`].
pub async fn index_project_cancellable_with_composition(
    options: IndexOptions,
    cancellation: CancellationToken,
    composition: &RuntimeComposition,
) -> Result<IndexReport> {
    index_project_inner(options, Some(cancellation), Some(composition)).await
}

async fn index_project_inner(
    options: IndexOptions,
    cancellation: Option<CancellationToken>,
    composition: Option<&RuntimeComposition>,
) -> Result<IndexReport> {
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
    recover_interrupted_publication(&root, &canonical_store, &state_store, &output_dir).await?;
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
            .extraction_concurrency_by_adapter(
                config.pipeline.extraction_concurrency_by_adapter.clone(),
            );
        if let Some(cancellation) = cancellation.clone() {
            pipeline
                .run_with_incremental_cancellable(
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
                    cancellation,
                )
                .await
        } else {
            pipeline
                .run_with_incremental(
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
                )
                .await
        }
    } else {
        let pipeline = runtime_builder(&root, composition)
            .allow_external_process(config.adapters.allow_external_process)
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
            .extraction_concurrency_by_adapter(
                config.pipeline.extraction_concurrency_by_adapter.clone(),
            );
        if let Some(cancellation) = cancellation.clone() {
            pipeline
                .run_with_incremental_cancellable_deferred(
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
                    cancellation,
                )
                .await
        } else {
            pipeline
                .run_with_incremental_deferred(
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

    let journal = IndexPublicationJournal::new(&root, &output.snapshot.0);
    journal.write()?;

    let read_model_started = Instant::now();
    let prepared_read_model = match JsonlReadModelWriter::new(&output_dir)
        .prepare_with_publication_id(&output, &journal.id)
    {
        Ok(prepared) => prepared,
        Err(error) => {
            let _ = journal.clear();
            abort_deferred_snapshot(&canonical_store, &output.snapshot, error).await?;
            unreachable!("abort_deferred_snapshot always returns an error")
        }
    };
    let read_model_write_ms = elapsed_ms(read_model_started.elapsed());

    let index_state_started = Instant::now();
    let prepared_index_state = match state_store.prepare_with_publication_id(
        &IndexState::from_sources(&output.snapshot.0, &output.files),
        &journal.id,
    ) {
        Ok(prepared) => prepared,
        Err(error) => {
            let rollback_error = prepared_read_model.rollback().err();
            let error = if let Some(rollback_error) = rollback_error {
                error.context(format!("failed to rollback read model: {rollback_error}"))
            } else {
                error
            };
            let _ = journal.clear();
            abort_deferred_snapshot(&canonical_store, &output.snapshot, error).await?;
            unreachable!("abort_deferred_snapshot always returns an error")
        }
    };
    let index_state_write_ms = elapsed_ms(index_state_started.elapsed());

    if let Err(error) = canonical_store
        .commit_snapshot(output.snapshot.clone())
        .await
    {
        let state_rollback_error = prepared_index_state.rollback().err();
        let read_model_rollback_error = prepared_read_model.rollback().err();
        let error = anyhow::Error::new(error).context(
            "failed to publish prepared canonical snapshot after read model and index state",
        );
        let error = if let Some(rollback_error) = state_rollback_error {
            error.context(format!("failed to rollback index state: {rollback_error}"))
        } else {
            error
        };
        let error = if let Some(rollback_error) = read_model_rollback_error {
            error.context(format!("failed to rollback read model: {rollback_error}"))
        } else {
            error
        };
        let _ = journal.clear();
        abort_deferred_snapshot(&canonical_store, &output.snapshot, error).await?;
        unreachable!("abort_deferred_snapshot always returns an error")
    }
    let read_model = prepared_read_model.finalize()?;
    prepared_index_state.finalize()?;
    journal.clear()?;
    let metrics = IndexReportMetrics {
        schema: INDEX_REPORT_METRICS_SCHEMA,
        total_ms: elapsed_ms(index_started.elapsed()),
        pipeline: output.metrics.clone(),
        read_model_write_ms,
        validation_result_write_ms: 0,
        index_state_write_ms,
    };

    Ok(IndexReport {
        root,
        snapshot: output.snapshot.0,
        files_indexed: output.files.len(),
        output_dir: read_model.output_dir,
        changed_files: read_model.changed_files,
        unchanged_files: read_model.unchanged_files,
        removed_files: read_model.removed_files,
        validation_report,
        validation_result: None,
        validate_only: false,
        metrics,
    })
}

fn runtime_builder(root: &Path, composition: Option<&RuntimeComposition>) -> RuntimeBuilder {
    match composition {
        Some(composition) => RuntimeBuilder::from_composition(root, composition),
        None => RuntimeBuilder::new(root),
    }
}

async fn abort_deferred_snapshot(
    store: &impl KnowledgeStore,
    snapshot: &athanor_domain::SnapshotId,
    error: anyhow::Error,
) -> Result<()> {
    match store.abort_snapshot(snapshot.clone()).await {
        Ok(()) => Err(error),
        Err(abort_error) => Err(error.context(format!(
            "failed to abort deferred snapshot {}: {abort_error}",
            snapshot.0
        ))),
    }
}

const INDEX_PUBLICATION_JOURNAL_SCHEMA: &str = "athanor.index_publication.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IndexPublicationJournal {
    schema: String,
    snapshot: String,
    id: String,
    read_model: PathBuf,
    index_state: PathBuf,
}

impl IndexPublicationJournal {
    fn new(root: &Path, snapshot: &str) -> Self {
        Self {
            schema: INDEX_PUBLICATION_JOURNAL_SCHEMA.to_string(),
            snapshot: snapshot.to_string(),
            id: format!(
                "{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            ),
            read_model: root.join(".athanor/generated/current/jsonl"),
            index_state: root.join(".athanor/state/index-state.json"),
        }
    }

    fn path(root: &Path) -> PathBuf {
        root.join(".athanor/state/index-publication.json")
    }

    fn write(&self) -> Result<()> {
        let path = Self::path_from_artifact(&self.index_state);
        let parent = path.parent().ok_or_else(|| {
            anyhow::anyhow!("publication journal has no parent: {}", path.display())
        })?;
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create publication journal directory {}",
                parent.display()
            )
        })?;
        let staging = parent.join(format!(".index-publication.staging-{}", self.id));
        let backup = parent.join(format!(".index-publication.backup-{}", self.id));
        fs::write(&staging, serde_json::to_vec_pretty(self)?).with_context(|| {
            format!("failed to write publication journal {}", staging.display())
        })?;
        if path.exists() {
            fs::rename(&path, &backup).with_context(|| {
                format!(
                    "failed to stage previous publication journal {}",
                    path.display()
                )
            })?;
        }
        if let Err(error) = fs::rename(&staging, &path) {
            if backup.exists() {
                let _ = fs::rename(&backup, &path);
            }
            let _ = fs::remove_file(&staging);
            return Err(error).with_context(|| {
                format!("failed to publish publication journal {}", path.display())
            });
        }
        if backup.exists() {
            fs::remove_file(&backup).with_context(|| {
                format!(
                    "failed to remove publication journal backup {}",
                    backup.display()
                )
            })?;
        }
        Ok(())
    }

    fn clear(&self) -> Result<()> {
        let path = Self::path_from_artifact(&self.index_state);
        if path.exists() {
            fs::remove_file(&path).with_context(|| {
                format!("failed to clear publication journal {}", path.display())
            })?;
        }
        Ok(())
    }

    fn path_from_artifact(index_state: &Path) -> PathBuf {
        index_state.with_file_name("index-publication.json")
    }
}

async fn recover_interrupted_publication(
    root: &Path,
    store: &crate::AthanorStore,
    state_store: &IndexStateStore,
    output_dir: &Path,
) -> Result<()> {
    let journal_path = IndexPublicationJournal::path(root);
    if !journal_path.exists() {
        return Ok(());
    }
    let journal: IndexPublicationJournal =
        serde_json::from_slice(&fs::read(&journal_path).with_context(|| {
            format!(
                "failed to read publication journal {}",
                journal_path.display()
            )
        })?)
        .with_context(|| {
            format!(
                "failed to parse publication journal {}",
                journal_path.display()
            )
        })?;
    if journal.schema != INDEX_PUBLICATION_JOURNAL_SCHEMA {
        anyhow::bail!("unsupported publication journal schema {}", journal.schema);
    }

    let committed = store
        .load_latest_snapshot()
        .await?
        .and_then(|snapshot| snapshot.snapshot)
        .is_some_and(|snapshot| snapshot.0 == journal.snapshot);
    if committed {
        cleanup_publication_artifacts(&journal, output_dir, state_store.path())?;
    } else {
        rollback_publication_artifacts(&journal, output_dir, state_store.path())?;
        match store
            .abort_snapshot(athanor_domain::SnapshotId(journal.snapshot.clone()))
            .await
        {
            Ok(()) | Err(athanor_core::CoreError::NotFound(_)) => {}
            Err(error) => {
                return Err(anyhow::Error::new(error).context("failed to abort recovered snapshot"));
            }
        }
    }
    fs::remove_file(&journal_path).with_context(|| {
        format!(
            "failed to clear recovered journal {}",
            journal_path.display()
        )
    })
}

fn cleanup_publication_artifacts(
    journal: &IndexPublicationJournal,
    output_dir: &Path,
    state_path: &Path,
) -> Result<()> {
    let (read_staging, read_backup) = publication_paths(output_dir, &journal.id)?;
    let (state_staging, state_backup) = publication_paths(state_path, &journal.id)?;
    if read_staging.exists() {
        fs::remove_dir_all(read_staging)?;
    }
    if read_backup.exists() {
        fs::remove_dir_all(read_backup)?;
    }
    if state_staging.exists() {
        fs::remove_file(state_staging)?;
    }
    if state_backup.exists() {
        fs::remove_file(state_backup)?;
    }
    Ok(())
}

fn rollback_publication_artifacts(
    journal: &IndexPublicationJournal,
    output_dir: &Path,
    state_path: &Path,
) -> Result<()> {
    restore_publication_directory(output_dir, &journal.id, &journal.snapshot)?;
    restore_publication_file(state_path, &journal.id, &journal.snapshot)?;
    Ok(())
}

fn publication_paths(path: &Path, id: &str) -> Result<(PathBuf, PathBuf)> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("publication artifact has no parent: {}", path.display()))?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow::anyhow!("invalid publication artifact: {}", path.display()))?;
    Ok((
        parent.join(format!(".{name}.staging-{id}")),
        parent.join(format!(".{name}.backup-{id}")),
    ))
}

fn restore_publication_directory(path: &Path, id: &str, snapshot: &str) -> Result<()> {
    let (staging, backup) = publication_paths(path, id)?;
    if staging.exists() {
        fs::remove_dir_all(staging)?;
    }
    if backup.exists() {
        if path.exists() {
            fs::remove_dir_all(path)?;
        }
        fs::rename(backup, path)?;
    } else if read_model_snapshot(path).is_some_and(|current| current == snapshot) {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}

fn restore_publication_file(path: &Path, id: &str, snapshot: &str) -> Result<()> {
    let (staging, backup) = publication_paths(path, id)?;
    if staging.exists() {
        fs::remove_file(staging)?;
    }
    if backup.exists() {
        if path.exists() {
            fs::remove_file(path)?;
        }
        fs::rename(backup, path)?;
    } else if state_snapshot(path).is_some_and(|current| current == snapshot) {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn read_model_snapshot(path: &Path) -> Option<String> {
    serde_json::from_slice::<serde_json::Value>(&fs::read(path.join("manifest.json")).ok()?)
        .ok()?
        .get("snapshot")?
        .as_str()
        .map(ToOwned::to_owned)
}

fn state_snapshot(path: &Path) -> Option<String> {
    serde_json::from_slice::<serde_json::Value>(&fs::read(path).ok()?)
        .ok()?
        .get("snapshot")?
        .as_str()
        .map(ToOwned::to_owned)
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

#[cfg(test)]
mod tests {
    use std::fs;

    use athanor_core::{CanonicalSnapshot, KnowledgeStore};
    use athanor_domain::{RepoId, SnapshotBase};
    use athanor_store_jsonl::JsonlKnowledgeStore;
    use serde::Serialize;
    use serde_json::Value;

    use super::*;

    fn publication_test_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "athanor-index-publication-{label}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[tokio::test]
    async fn recovery_rolls_back_uncommitted_publication() {
        let root = publication_test_root("rollback");
        let output_dir = root.join(".athanor/generated/current/jsonl");
        let state_path = root.join(".athanor/state/index-state.json");
        fs::create_dir_all(&output_dir).unwrap();
        fs::create_dir_all(state_path.parent().unwrap()).unwrap();
        fs::write(
            output_dir.join("manifest.json"),
            r#"{"snapshot":"snap_old"}"#,
        )
        .unwrap();
        fs::write(&state_path, r#"{"snapshot":"snap_old"}"#).unwrap();

        let journal = IndexPublicationJournal {
            schema: INDEX_PUBLICATION_JOURNAL_SCHEMA.to_string(),
            snapshot: "snap_jsonl_00000001".to_string(),
            id: "test-publication".to_string(),
            read_model: output_dir.clone(),
            index_state: state_path.clone(),
        };
        let (_, read_backup) = publication_paths(&output_dir, &journal.id).unwrap();
        let (_, state_backup) = publication_paths(&state_path, &journal.id).unwrap();
        fs::rename(&output_dir, &read_backup).unwrap();
        fs::rename(&state_path, &state_backup).unwrap();
        fs::create_dir_all(&output_dir).unwrap();
        fs::write(
            output_dir.join("manifest.json"),
            r#"{"snapshot":"snap_jsonl_00000001"}"#,
        )
        .unwrap();
        fs::write(&state_path, r#"{"snapshot":"snap_jsonl_00000001"}"#).unwrap();
        journal.write().unwrap();

        let store = crate::AthanorStore::new(JsonlKnowledgeStore::new(
            root.join(".athanor/store/canonical/jsonl"),
        ));
        let state_store = IndexStateStore::new(&state_path);
        recover_interrupted_publication(&root, &store, &state_store, &output_dir)
            .await
            .unwrap();

        assert_eq!(
            read_model_snapshot(&output_dir).as_deref(),
            Some("snap_old")
        );
        assert_eq!(state_snapshot(&state_path).as_deref(), Some("snap_old"));
        assert!(!IndexPublicationJournal::path(&root).exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn recovery_finalizes_after_canonical_commit() {
        let root = publication_test_root("finalize");
        let output_dir = root.join(".athanor/generated/current/jsonl");
        let state_path = root.join(".athanor/state/index-state.json");
        fs::create_dir_all(&output_dir).unwrap();
        fs::create_dir_all(state_path.parent().unwrap()).unwrap();
        fs::write(
            output_dir.join("manifest.json"),
            r#"{"snapshot":"snap_jsonl_00000001"}"#,
        )
        .unwrap();
        fs::write(&state_path, r#"{"snapshot":"snap_jsonl_00000001"}"#).unwrap();
        let journal = IndexPublicationJournal {
            schema: INDEX_PUBLICATION_JOURNAL_SCHEMA.to_string(),
            snapshot: "snap_jsonl_00000001".to_string(),
            id: "test-publication".to_string(),
            read_model: output_dir.clone(),
            index_state: state_path.clone(),
        };
        let (_, read_backup) = publication_paths(&output_dir, &journal.id).unwrap();
        let (_, state_backup) = publication_paths(&state_path, &journal.id).unwrap();
        fs::create_dir_all(&read_backup).unwrap();
        fs::write(
            read_backup.join("manifest.json"),
            r#"{"snapshot":"snap_old"}"#,
        )
        .unwrap();
        fs::write(&state_backup, r#"{"snapshot":"snap_old"}"#).unwrap();
        journal.write().unwrap();

        let store = crate::AthanorStore::new(JsonlKnowledgeStore::new(
            root.join(".athanor/store/canonical/jsonl"),
        ));
        let snapshot = store
            .begin_snapshot(
                RepoId("repo_test".to_string()),
                SnapshotBase {
                    branch: None,
                    commit: None,
                    parent_snapshot: None,
                    working_tree: true,
                },
            )
            .await
            .unwrap();
        assert_eq!(snapshot.0, journal.snapshot);
        store.commit_snapshot(snapshot).await.unwrap();
        let state_store = IndexStateStore::new(&state_path);
        recover_interrupted_publication(&root, &store, &state_store, &output_dir)
            .await
            .unwrap();

        assert_eq!(
            read_model_snapshot(&output_dir).as_deref(),
            Some("snap_jsonl_00000001")
        );
        assert_eq!(
            state_snapshot(&state_path).as_deref(),
            Some("snap_jsonl_00000001")
        );
        assert!(!read_backup.exists());
        assert!(!state_backup.exists());
        assert!(!IndexPublicationJournal::path(&root).exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn cancelled_index_does_not_publish_snapshot_state_or_read_model() {
        let root = std::env::temp_dir().join(format!(
            "athanor-index-cancelled-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/lib.rs"), "pub fn hello() {}\n").unwrap();
        let cancellation = CancellationToken::new();
        cancellation.cancel();

        let error = index_project_cancellable(
            IndexOptions {
                root: root.clone(),
                validation_report: None,
                validation_result: None,
                validate_only: false,
            },
            cancellation,
        )
        .await
        .unwrap_err();

        assert!(error.to_string().contains("failed to run index pipeline"));
        assert!(
            error
                .chain()
                .any(|cause| cause.to_string() == "operation cancelled")
        );
        assert!(!root.join(".athanor/state/index-state.json").exists());
        assert!(!root.join(".athanor/generated/current/jsonl").exists());

        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn indexes_files_to_jsonl() {
        let root = std::env::temp_dir().join(format!(
            "athanor-index-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("docs")).unwrap();
        fs::write(root.join("src/lib.rs"), "pub fn hello() {}\n").unwrap();
        fs::write(root.join("docs/auth.md"), "# Auth\n\n## Login\n").unwrap();

        let report = index_project(IndexOptions {
            root: root.clone(),
            validation_report: None,
            validation_result: None,
            validate_only: false,
        })
        .await
        .unwrap();

        assert_eq!(report.files_indexed, 2);
        assert!(report.output_dir.join("entities.jsonl").is_file());
        assert!(report.output_dir.join("facts.jsonl").is_file());
        assert!(report.output_dir.join("relations.jsonl").is_file());
        assert!(report.output_dir.join("diagnostics.jsonl").is_file());
        assert!(report.output_dir.join("manifest.json").is_file());

        let entities = fs::read_to_string(report.output_dir.join("entities.jsonl")).unwrap();
        assert!(entities.contains("file://src/lib.rs"));

        let facts = fs::read_to_string(report.output_dir.join("facts.jsonl")).unwrap();
        assert!(facts.contains("file_discovered"));

        let relations = fs::read_to_string(report.output_dir.join("relations.jsonl")).unwrap();
        assert!(relations.contains("contains"));
        assert_eq!(report.changed_files, 2);
        assert_eq!(report.unchanged_files, 0);
        assert_eq!(report.removed_files, 0);
        assert_eq!(report.metrics.schema, INDEX_REPORT_METRICS_SCHEMA);
        assert_eq!(report.metrics.pipeline.schema, crate::INDEX_METRICS_SCHEMA);
        assert_eq!(report.metrics.pipeline.files_discovered, 2);
        assert_eq!(report.metrics.pipeline.files_to_extract, 2);
        assert!(!report.metrics.pipeline.adapters.is_empty());
        assert!(
            report
                .metrics
                .pipeline
                .adapters
                .iter()
                .any(|adapter| adapter.phase == "extractor"
                    && adapter.adapter == "file"
                    && adapter.runs == 2)
        );
        assert!(root.join(".athanor/state/index-state.json").is_file());

        let second_report = index_project(IndexOptions {
            root: root.clone(),
            validation_report: None,
            validation_result: None,
            validate_only: false,
        })
        .await
        .unwrap();

        assert_eq!(second_report.files_indexed, 2);
        assert_eq!(second_report.snapshot, report.snapshot);
        assert_eq!(second_report.changed_files, 0);
        assert_eq!(second_report.unchanged_files, 2);
        assert_eq!(second_report.removed_files, 0);
        assert_eq!(second_report.metrics.pipeline.files_to_extract, 0);
        let second_entities =
            fs::read_to_string(second_report.output_dir.join("entities.jsonl")).unwrap();
        assert!(second_entities.contains("file://src/lib.rs"));
        assert!(second_entities.contains("doc://docs/auth.md#login"));

        fs::write(root.join("docs/auth.md"), "# Auth\n").unwrap();
        let third_report = index_project(IndexOptions {
            root: root.clone(),
            validation_report: None,
            validation_result: None,
            validate_only: false,
        })
        .await
        .unwrap();

        assert_eq!(third_report.files_indexed, 2);
        assert_eq!(third_report.changed_files, 1);
        assert_eq!(third_report.unchanged_files, 1);
        assert_eq!(third_report.removed_files, 0);
        let third_entities =
            fs::read_to_string(third_report.output_dir.join("entities.jsonl")).unwrap();
        assert!(third_entities.contains("file://src/lib.rs"));
        assert!(third_entities.contains("doc://docs/auth.md"));
        assert!(!third_entities.contains("doc://docs/auth.md#login"));

        fs::remove_file(root.join("src/lib.rs")).unwrap();
        let fourth_report = index_project(IndexOptions {
            root: root.clone(),
            validation_report: None,
            validation_result: None,
            validate_only: false,
        })
        .await
        .unwrap();

        assert_eq!(fourth_report.files_indexed, 1);
        assert_eq!(fourth_report.changed_files, 0);
        assert_eq!(fourth_report.unchanged_files, 1);
        assert_eq!(fourth_report.removed_files, 1);
        assert_eq!(fourth_report.metrics.pipeline.files_to_extract, 0);
        let fourth_entities =
            fs::read_to_string(fourth_report.output_dir.join("entities.jsonl")).unwrap();
        assert!(!fourth_entities.contains("file://src/lib.rs"));
        assert!(fourth_entities.contains("doc://docs/auth.md"));

        fs::write(root.join("docs/new.md"), "# New\n").unwrap();
        let fifth_report = index_project(IndexOptions {
            root: root.clone(),
            validation_report: None,
            validation_result: None,
            validate_only: false,
        })
        .await
        .unwrap();

        assert_eq!(fifth_report.files_indexed, 2);
        assert_eq!(fifth_report.changed_files, 1);
        assert_eq!(fifth_report.unchanged_files, 1);
        assert_eq!(fifth_report.removed_files, 0);
        assert_eq!(fifth_report.metrics.pipeline.files_to_extract, 1);

        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn refreshes_frontmatter_relations_and_diagnostics_when_only_target_changes() {
        let root = std::env::temp_dir().join(format!(
            "athanor-frontmatter-incremental-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("docs")).unwrap();
        fs::write(
            root.join("docs/auth.md"),
            "---\nid: doc://product/auth\nentities:\n  - api://POST:/login\n---\n# Auth\n",
        )
        .unwrap();
        write_openapi_path(&root, "/login");

        index_project(IndexOptions {
            root: root.clone(),
            validation_report: None,
            validation_result: None,
            validate_only: false,
        })
        .await
        .unwrap();
        let relations =
            fs::read_to_string(root.join(".athanor/generated/current/jsonl/relations.jsonl"))
                .unwrap();
        let diagnostics =
            fs::read_to_string(root.join(".athanor/generated/current/jsonl/diagnostics.jsonl"))
                .unwrap();
        assert!(relations.contains("markdown_frontmatter_reference"));
        assert!(!diagnostics.contains("documentation_reference_unresolved"));

        write_openapi_path(&root, "/signin");
        let changed = index_project(IndexOptions {
            root: root.clone(),
            validation_report: None,
            validation_result: None,
            validate_only: false,
        })
        .await
        .unwrap();
        assert_eq!(changed.changed_files, 1);
        let diagnostics =
            fs::read_to_string(root.join(".athanor/generated/current/jsonl/diagnostics.jsonl"))
                .unwrap();
        assert!(diagnostics.contains("documentation_reference_unresolved"));

        write_openapi_path(&root, "/login");
        index_project(IndexOptions {
            root: root.clone(),
            validation_report: None,
            validation_result: None,
            validate_only: false,
        })
        .await
        .unwrap();
        let diagnostics =
            fs::read_to_string(root.join(".athanor/generated/current/jsonl/diagnostics.jsonl"))
                .unwrap();
        assert!(!diagnostics.contains("documentation_reference_unresolved"));

        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn incremental_index_matches_fresh_full_index_for_same_final_sources() {
        let incremental_root = temp_root("athanor-incremental-equivalence");
        write_equivalence_fixture(&incremental_root, "# Auth\n\n## Login\n");

        index_project(IndexOptions {
            root: incremental_root.clone(),
            validation_report: None,
            validation_result: None,
            validate_only: false,
        })
        .await
        .unwrap();
        fs::write(
            incremental_root.join("docs/auth.md"),
            "# Auth\n\n## Login\n\n## Logout\n",
        )
        .unwrap();
        let incremental_report = index_project(IndexOptions {
            root: incremental_root.clone(),
            validation_report: None,
            validation_result: None,
            validate_only: false,
        })
        .await
        .unwrap();
        assert_eq!(incremental_report.changed_files, 1);
        assert_eq!(incremental_report.unchanged_files, 2);

        let fresh_root = temp_root("athanor-fresh-equivalence");
        write_equivalence_fixture(&fresh_root, "# Auth\n\n## Login\n\n## Logout\n");
        let fresh_report = index_project(IndexOptions {
            root: fresh_root.clone(),
            validation_report: None,
            validation_result: None,
            validate_only: false,
        })
        .await
        .unwrap();
        assert_eq!(fresh_report.changed_files, 3);

        let incremental = load_latest_canonical_snapshot(&incremental_root).await;
        let fresh = load_latest_canonical_snapshot(&fresh_root).await;
        assert_eq!(
            normalized_snapshot_objects(&incremental.entities),
            normalized_snapshot_objects(&fresh.entities)
        );
        assert_eq!(
            normalized_snapshot_objects(&incremental.facts),
            normalized_snapshot_objects(&fresh.facts)
        );
        assert_eq!(
            normalized_snapshot_objects(&incremental.relations),
            normalized_snapshot_objects(&fresh.relations)
        );
        assert_eq!(
            normalized_snapshot_objects(&incremental.diagnostics),
            normalized_snapshot_objects(&fresh.diagnostics)
        );

        fs::remove_dir_all(incremental_root).unwrap();
        fs::remove_dir_all(fresh_root).unwrap();
    }

    fn write_openapi_path(root: &Path, path: &str) {
        fs::write(
            root.join("openapi.yaml"),
            format!(
                "openapi: 3.0.3\ninfo:\n  title: Test\n  version: 1.0.0\npaths:\n  {path}:\n    post:\n      operationId: login\n      responses:\n        '200':\n          description: ok\n"
            ),
        )
        .unwrap();
    }

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "{label}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn write_equivalence_fixture(root: &Path, auth_markdown: &str) {
        fs::create_dir_all(root.join("docs")).unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("docs/auth.md"), auth_markdown).unwrap();
        fs::write(root.join("src/lib.rs"), "pub fn login() {}\n").unwrap();
        write_openapi_path(root, "/login");
    }

    async fn load_latest_canonical_snapshot(root: &Path) -> CanonicalSnapshot {
        JsonlKnowledgeStore::new(root.join(".athanor/store/canonical/jsonl"))
            .load_latest_snapshot()
            .await
            .unwrap()
            .expect("latest canonical snapshot should exist")
    }

    fn normalized_snapshot_objects<T: Serialize>(items: &[T]) -> Vec<Value> {
        let mut values = items
            .iter()
            .map(|item| {
                let mut value = serde_json::to_value(item).unwrap();
                normalize_snapshot_fields(&mut value);
                value
            })
            .collect::<Vec<_>>();
        values.sort_by(|left, right| {
            serde_json::to_string(left)
                .unwrap()
                .cmp(&serde_json::to_string(right).unwrap())
        });
        values
    }

    fn normalize_snapshot_fields(value: &mut Value) {
        match value {
            Value::Object(object) => {
                if object.contains_key("snapshot") {
                    object.insert(
                        "snapshot".to_string(),
                        Value::String("<snapshot>".to_string()),
                    );
                }
                for child in object.values_mut() {
                    normalize_snapshot_fields(child);
                }
            }
            Value::Array(items) => {
                for item in items {
                    normalize_snapshot_fields(item);
                }
            }
            _ => {}
        }
    }

    #[tokio::test]
    async fn validates_without_writing_outputs_or_state() {
        let root = std::env::temp_dir().join(format!(
            "athanor-validate-only-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        fs::create_dir_all(root.join("docs")).unwrap();
        fs::write(root.join("docs/auth.md"), "# Auth\n\n## Login\n").unwrap();

        let report = index_project(IndexOptions {
            root: root.clone(),
            validation_report: None,
            validation_result: None,
            validate_only: true,
        })
        .await
        .unwrap();

        assert!(report.validate_only);
        assert_eq!(report.files_indexed, 1);
        assert_eq!(report.metrics.schema, INDEX_REPORT_METRICS_SCHEMA);
        assert_eq!(report.metrics.pipeline.schema, crate::INDEX_METRICS_SCHEMA);
        assert_eq!(report.metrics.pipeline.files_discovered, 1);
        assert_eq!(report.metrics.read_model_write_ms, 0);
        let validation_result = report.validation_result.unwrap();
        assert_eq!(
            validation_result.canonicalize().unwrap(),
            root.join(".athanor/generated/current/validation-result.json")
                .canonicalize()
                .unwrap()
        );
        let validation_result_json = fs::read_to_string(validation_result).unwrap();
        assert!(validation_result_json.contains(VALIDATION_RESULT_SCHEMA));
        assert!(validation_result_json.contains("\"status\": \"passed\""));
        assert!(validation_result_json.contains("\"files_indexed\": 1"));
        assert!(!root.join(".athanor/state/index-state.json").exists());
        assert!(
            !root
                .join(".athanor/generated/current/jsonl/entities.jsonl")
                .exists()
        );
        assert!(
            !root
                .join(".athanor/store/canonical/jsonl/latest.json")
                .exists()
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn writes_configured_validation_result_for_validate_only() {
        let root = std::env::temp_dir().join(format!(
            "athanor-validation-result-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        fs::create_dir_all(root.join("docs")).unwrap();
        fs::write(root.join("docs/auth.md"), "# Auth\n\n## Login\n").unwrap();
        let validation_result = root.join("custom-validation-result.json");

        let report = index_project(IndexOptions {
            root: root.clone(),
            validation_report: None,
            validation_result: Some(validation_result.clone()),
            validate_only: true,
        })
        .await
        .unwrap();

        assert_eq!(report.validation_result, Some(validation_result.clone()));
        let content = fs::read_to_string(validation_result).unwrap();
        assert!(content.contains("\"schema\": \"athanor.validation_result.v1\""));
        assert!(content.contains("\"relations\""));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn writes_validation_report_json() {
        let root = std::env::temp_dir().join(format!(
            "athanor-validation-report-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = root.join("validation-report.json");
        let report = AdapterValidationReport {
            adapter: "test-adapter".to_string(),
            issues: vec![crate::AdapterValidationIssue {
                object_type: "relation",
                object_id: "rel_test".to_string(),
                missing: crate::MissingCanonicalMetadata::Ownership,
            }],
        };

        write_validation_report(&path, &report).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("\"adapter\": \"test-adapter\""));
        assert!(content.contains("\"missing\": \"ownership\""));

        fs::remove_dir_all(root).unwrap();
    }
}
