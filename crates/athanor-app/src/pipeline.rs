use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use athanor_core::{
    AffectedSubset, CanonicalSnapshot, Checker, Extractor, KnowledgeStore, Linker,
    OperationContext, SourceFile, SourceProvider,
};
#[cfg(test)]
use athanor_core::{CheckInput, ExtractInput, LinkInput};
use athanor_domain::{
    Diagnostic, Entity, Evidence, Fact, Relation, RepoId, SnapshotBase, SnapshotId,
};
use serde::Serialize;
use tracing::{debug, info};

use crate::pipeline_merge::{
    canonicalize_diagnostics, canonicalize_entities, canonicalize_facts, canonicalize_relations,
};
use crate::pipeline_metrics::{aggregate_adapter_metrics, elapsed_ms};
use crate::pipeline_ownership::{
    diagnostic_invalidated_by_changed_path, diagnostic_owned_by_any_path, entity_owned_by_any_path,
    fact_owned_by_any_path, relation_owned_by_any_path,
};
use crate::pipeline_support::{
    check_cancelled, replace_payload_snapshot, unwrap_or_clone_arc_vec, within_operation_deadline,
};
use crate::{
    AdapterInvalidationDeclaration, AffectedFileSet, CancellationToken, IndexState,
    dependency_closure, plan_invalidation,
};

const DEFAULT_EXTRACTION_CONCURRENCY_LIMIT: usize = 16;
const DEFAULT_MAX_EXTRACTION_BYTES_IN_FLIGHT: usize = 64 * 1024 * 1024;
pub const INDEX_METRICS_SCHEMA: &str = "athanor.index_metrics.v1";

pub struct IndexPipeline {
    store: Box<dyn KnowledgeStore>,
    sources: Vec<Box<dyn SourceProvider>>,
    extractors: Vec<Box<dyn Extractor>>,
    linkers: Vec<Box<dyn Linker>>,
    checkers: Vec<Box<dyn Checker>>,
    extraction_concurrency: usize,
    max_extraction_bytes_in_flight: usize,
    extraction_concurrency_by_adapter: BTreeMap<String, usize>,
}

#[derive(Debug, Clone)]
pub struct IndexPipelineOutput {
    pub snapshot: SnapshotId,
    pub files: Vec<SourceFile>,
    pub entities: Vec<Entity>,
    pub facts: Vec<Fact>,
    pub relations: Vec<Relation>,
    pub diagnostics: Vec<Diagnostic>,
    pub affected_files: AffectedFileSet,
    pub metrics: IndexPipelineMetrics,
}

#[derive(Debug, Clone, Default)]
pub struct IncrementalIndexContext {
    pub previous_state: IndexState,
    pub previous_snapshot: Option<CanonicalSnapshot>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct IndexPipelineMetrics {
    pub schema: &'static str,
    pub total_ms: u64,
    pub source_discovery_ms: u64,
    pub affected_classification_ms: u64,
    pub snapshot_begin_ms: u64,
    pub extraction_ms: u64,
    pub merge_ms: u64,
    pub linking_ms: u64,
    pub checking_ms: u64,
    pub canonicalize_ms: u64,
    pub storage_ms: u64,
    pub files_discovered: usize,
    pub files_to_extract: usize,
    pub extraction_concurrency: usize,
    pub max_extraction_bytes_in_flight: usize,
    pub extraction_concurrency_by_adapter: BTreeMap<String, usize>,
    pub changed_files: usize,
    pub unchanged_files: usize,
    pub removed_files: usize,
    pub invalidation_scope: Option<String>,
    pub invalidation_global_adapters: Vec<String>,
    pub entities: usize,
    pub facts: usize,
    pub relations: usize,
    pub diagnostics: usize,
    pub validation_issues: usize,
    pub adapters: Vec<AdapterRunMetrics>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct AdapterRunMetrics {
    pub phase: &'static str,
    pub adapter: String,
    pub runs: usize,
    pub duration_ms: u64,
    pub input_files: usize,
    pub input_entities: usize,
    pub input_facts: usize,
    pub input_relations: usize,
    pub output_files: usize,
    pub output_entities: usize,
    pub output_facts: usize,
    pub output_relations: usize,
    pub output_diagnostics: usize,
    pub validation_issues: usize,
    pub timeout_count: usize,
    pub stdin_bytes: Option<u64>,
    pub stdout_bytes: Option<u64>,
    pub stderr_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdapterValidationReport {
    pub adapter: String,
    pub issues: Vec<AdapterValidationIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdapterValidationIssue {
    pub object_type: &'static str,
    pub object_id: String,
    pub missing: MissingCanonicalMetadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MissingCanonicalMetadata {
    Evidence,
    Ownership,
}

impl AdapterValidationReport {
    fn new(adapter: &str) -> Self {
        Self {
            adapter: adapter.to_string(),
            issues: Vec::new(),
        }
    }

    fn push(
        &mut self,
        object_type: &'static str,
        object_id: impl Into<String>,
        missing: MissingCanonicalMetadata,
    ) {
        self.issues.push(AdapterValidationIssue {
            object_type,
            object_id: object_id.into(),
            missing,
        });
    }

    fn finish(self) -> Result<()> {
        if self.issues.is_empty() {
            Ok(())
        } else {
            Err(anyhow::Error::new(self))
        }
    }
}

impl fmt::Display for AdapterValidationReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "adapter {} emitted invalid canonical output",
            self.adapter
        )?;

        if self.issues.is_empty() {
            return Ok(());
        }

        write!(formatter, " ({} issue", self.issues.len())?;

        if self.issues.len() != 1 {
            write!(formatter, "s")?;
        }

        write!(formatter, ")")?;

        for issue in &self.issues {
            write!(
                formatter,
                "; {} {} missing {}",
                issue.object_type, issue.object_id, issue.missing
            )?;
        }

        Ok(())
    }
}

impl std::error::Error for AdapterValidationReport {}

impl fmt::Display for MissingCanonicalMetadata {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MissingCanonicalMetadata::Evidence => formatter.write_str("evidence"),
            MissingCanonicalMetadata::Ownership => formatter.write_str("ownership"),
        }
    }
}

impl IndexPipeline {
    pub fn new(store: impl KnowledgeStore + 'static) -> Self {
        Self {
            store: Box::new(store),
            sources: Vec::new(),
            extractors: Vec::new(),
            linkers: Vec::new(),
            checkers: Vec::new(),
            extraction_concurrency: DEFAULT_EXTRACTION_CONCURRENCY_LIMIT,
            max_extraction_bytes_in_flight: DEFAULT_MAX_EXTRACTION_BYTES_IN_FLIGHT,
            extraction_concurrency_by_adapter: BTreeMap::new(),
        }
    }

    pub fn boxed_source(mut self, source: Box<dyn SourceProvider>) -> Self {
        self.sources.push(source);
        self
    }

    pub fn source(mut self, source: impl SourceProvider + 'static) -> Self {
        self.sources.push(Box::new(source));
        self
    }

    pub fn boxed_extractor(mut self, extractor: Box<dyn Extractor>) -> Self {
        self.extractors.push(extractor);
        self
    }

    pub fn extractor(mut self, extractor: impl Extractor + 'static) -> Self {
        self.extractors.push(Box::new(extractor));
        self
    }

    pub fn boxed_linker(mut self, linker: Box<dyn Linker>) -> Self {
        self.linkers.push(linker);
        self
    }

    pub fn linker(mut self, linker: impl Linker + 'static) -> Self {
        self.linkers.push(Box::new(linker));
        self
    }

    pub fn boxed_checker(mut self, checker: Box<dyn Checker>) -> Self {
        self.checkers.push(checker);
        self
    }

    pub fn checker(mut self, checker: impl Checker + 'static) -> Self {
        self.checkers.push(Box::new(checker));
        self
    }

    /// Sets the maximum number of extractor/source-file tasks in flight.
    pub fn extraction_concurrency(mut self, limit: usize) -> Self {
        self.extraction_concurrency = limit.max(1);
        self
    }

    /// Sets the total source-content byte budget held by concurrent extractors.
    pub fn max_extraction_bytes_in_flight(mut self, bytes: usize) -> Self {
        self.max_extraction_bytes_in_flight = bytes.max(1);
        self
    }

    /// Sets optional extractor-specific task limits keyed by adapter name.
    pub fn extraction_concurrency_by_adapter(
        mut self,
        limits: impl IntoIterator<Item = (String, usize)>,
    ) -> Self {
        self.extraction_concurrency_by_adapter = limits
            .into_iter()
            .filter_map(|(adapter, limit)| (limit > 0).then_some((adapter, limit)))
            .collect();
        self
    }

    fn invalidation_declarations(&self) -> Vec<AdapterInvalidationDeclaration> {
        self.extractors
            .iter()
            .map(|adapter| AdapterInvalidationDeclaration {
                adapter: format!("extractor:{}", adapter.name()),
                policy: adapter.invalidation_policy(),
            })
            .chain(
                self.linkers
                    .iter()
                    .map(|adapter| AdapterInvalidationDeclaration {
                        adapter: format!("linker:{}", adapter.name()),
                        policy: adapter.invalidation_policy(),
                    }),
            )
            .chain(
                self.checkers
                    .iter()
                    .map(|adapter| AdapterInvalidationDeclaration {
                        adapter: format!("checker:{}", adapter.name()),
                        policy: adapter.invalidation_policy(),
                    }),
            )
            .collect()
    }

    pub async fn run(self, repo: RepoId, base: SnapshotBase) -> Result<IndexPipelineOutput> {
        self.run_with_incremental(repo, base, IncrementalIndexContext::default())
            .await
    }

    pub async fn run_with_incremental(
        self,
        repo: RepoId,
        base: SnapshotBase,
        incremental: IncrementalIndexContext,
    ) -> Result<IndexPipelineOutput> {
        self.run_with_incremental_operation_context(
            repo,
            base,
            incremental,
            OperationContext::new("index"),
        )
        .await
    }

    /// Runs incremental indexing with transport-neutral adapter operation metadata.
    pub async fn run_with_incremental_operation_context(
        self,
        repo: RepoId,
        base: SnapshotBase,
        incremental: IncrementalIndexContext,
        operation: OperationContext,
    ) -> Result<IndexPipelineOutput> {
        self.run_with_incremental_inner(repo, base, incremental, operation, None, true)
            .await
    }

    /// Writes canonical objects but leaves the snapshot uncommitted for an application-level
    /// publication coordinator. The caller must commit or abort `output.snapshot`.
    pub async fn run_with_incremental_deferred(
        self,
        repo: RepoId,
        base: SnapshotBase,
        incremental: IncrementalIndexContext,
    ) -> Result<IndexPipelineOutput> {
        self.run_with_incremental_deferred_operation_context(
            repo,
            base,
            incremental,
            OperationContext::new("index.deferred"),
        )
        .await
    }

    /// Deferred-publication indexing with transport-neutral adapter operation metadata.
    pub async fn run_with_incremental_deferred_operation_context(
        self,
        repo: RepoId,
        base: SnapshotBase,
        incremental: IncrementalIndexContext,
        operation: OperationContext,
    ) -> Result<IndexPipelineOutput> {
        self.run_with_incremental_inner(repo, base, incremental, operation, None, false)
            .await
    }

    pub async fn run_with_incremental_cancellable(
        self,
        repo: RepoId,
        base: SnapshotBase,
        incremental: IncrementalIndexContext,
        cancellation: CancellationToken,
    ) -> Result<IndexPipelineOutput> {
        self.run_with_incremental_cancellable_operation_context(
            repo,
            base,
            incremental,
            OperationContext::new("index"),
            cancellation,
        )
        .await
    }

    /// Cancellable incremental indexing with operation metadata propagated to every adapter phase.
    pub async fn run_with_incremental_cancellable_operation_context(
        self,
        repo: RepoId,
        base: SnapshotBase,
        incremental: IncrementalIndexContext,
        operation: OperationContext,
        cancellation: CancellationToken,
    ) -> Result<IndexPipelineOutput> {
        self.run_with_incremental_inner(
            repo,
            base,
            incremental,
            operation,
            Some(cancellation),
            true,
        )
        .await
    }

    /// Cancellable deferred-publication variant of [`Self::run_with_incremental_deferred`].
    pub async fn run_with_incremental_cancellable_deferred(
        self,
        repo: RepoId,
        base: SnapshotBase,
        incremental: IncrementalIndexContext,
        cancellation: CancellationToken,
    ) -> Result<IndexPipelineOutput> {
        self.run_with_incremental_cancellable_deferred_operation_context(
            repo,
            base,
            incremental,
            OperationContext::new("index.deferred"),
            cancellation,
        )
        .await
    }

    /// Cancellable deferred-publication indexing with operation metadata propagated to all ports.
    pub async fn run_with_incremental_cancellable_deferred_operation_context(
        self,
        repo: RepoId,
        base: SnapshotBase,
        incremental: IncrementalIndexContext,
        operation: OperationContext,
        cancellation: CancellationToken,
    ) -> Result<IndexPipelineOutput> {
        self.run_with_incremental_inner(
            repo,
            base,
            incremental,
            operation,
            Some(cancellation),
            false,
        )
        .await
    }

    pub async fn run_extraction_only(
        self,
        repo: RepoId,
        base: SnapshotBase,
        affected_files: AffectedFileSet,
    ) -> Result<IndexPipelineOutput> {
        let operation = OperationContext::new("index.extraction_only");
        let pipeline_started = Instant::now();
        let mut metrics = IndexPipelineMetrics {
            schema: INDEX_METRICS_SCHEMA,
            extraction_concurrency: self.extraction_concurrency,
            max_extraction_bytes_in_flight: self.max_extraction_bytes_in_flight,
            extraction_concurrency_by_adapter: self.extraction_concurrency_by_adapter.clone(),
            ..IndexPipelineMetrics::default()
        };

        let source_started = Instant::now();
        let (files, source_metrics) = self.discover_sources(&operation, None).await?;
        metrics.source_discovery_ms = elapsed_ms(source_started.elapsed());
        metrics.files_discovered = files.len();
        metrics.files_to_extract = files.len();
        metrics.changed_files = affected_files.changed.len();
        metrics.unchanged_files = affected_files.unchanged.len();
        metrics.removed_files = affected_files.removed.len();
        metrics.adapters.extend(source_metrics);

        let snapshot_started = Instant::now();
        let snapshot = within_operation_deadline(
            &operation,
            "store.begin_snapshot",
            self.store
                .begin_snapshot_with_context(repo.clone(), base, &operation),
        )
        .await
        .context("failed to begin validation snapshot")?;
        metrics.snapshot_begin_ms = elapsed_ms(snapshot_started.elapsed());

        let extraction_started = Instant::now();
        let (entities, facts, diagnostics, extraction_metrics) = self
            .extract(&repo, &snapshot, &files, &operation, None)
            .await?;
        metrics.extraction_ms = elapsed_ms(extraction_started.elapsed());
        metrics.adapters.extend(extraction_metrics);

        metrics.entities = entities.len();
        metrics.facts = facts.len();
        metrics.diagnostics = diagnostics.len();
        metrics.total_ms = elapsed_ms(pipeline_started.elapsed());
        metrics.adapters = aggregate_adapter_metrics(metrics.adapters);

        Ok(IndexPipelineOutput {
            snapshot,
            files,
            entities,
            facts,
            relations: Vec::new(),
            diagnostics,
            affected_files,
            metrics,
        })
    }

    async fn run_with_incremental_inner(
        self,
        repo: RepoId,
        base: SnapshotBase,
        incremental: IncrementalIndexContext,
        operation: OperationContext,
        cancellation: Option<CancellationToken>,
        commit_snapshot: bool,
    ) -> Result<IndexPipelineOutput> {
        let pipeline_started = Instant::now();
        let mut metrics = IndexPipelineMetrics {
            schema: INDEX_METRICS_SCHEMA,
            extraction_concurrency: self.extraction_concurrency,
            max_extraction_bytes_in_flight: self.max_extraction_bytes_in_flight,
            ..IndexPipelineMetrics::default()
        };
        check_cancelled(&cancellation)?;
        let previous_snapshot_available = incremental.previous_snapshot.is_some();

        info!("starting source discovery");
        let source_started = Instant::now();
        let (files, source_metrics) = self
            .discover_sources(&operation, cancellation.clone())
            .await?;
        metrics.source_discovery_ms = elapsed_ms(source_started.elapsed());
        metrics.files_discovered = files.len();
        metrics.adapters.extend(source_metrics);
        check_cancelled(&cancellation)?;
        info!(file_count = files.len(), "completed source discovery");
        let classification_started = Instant::now();
        let mut affected_files = incremental.previous_state.affected_files(&files);

        let has_added_files = affected_files
            .changed
            .iter()
            .any(|path| !incremental.previous_state.files.contains_key(path));
        let invalidation_plan = plan_invalidation(
            self.invalidation_declarations(),
            has_added_files,
            !affected_files.removed.is_empty(),
        );
        debug!(
            scope = ?invalidation_plan.scope,
            global_adapters = ?invalidation_plan.global_adapters,
            "planned adapter invalidation"
        );
        metrics.invalidation_scope = Some(
            match invalidation_plan.scope {
                crate::PlannedInvalidationScope::FileLocal => "file_local",
                crate::PlannedInvalidationScope::DependencyClosure => "dependency_closure",
                crate::PlannedInvalidationScope::Global => "global",
            }
            .to_string(),
        );
        metrics.invalidation_global_adapters = invalidation_plan.global_adapters.clone();
        let removed_dependency_neighbor_ids = if matches!(
            invalidation_plan.scope,
            crate::PlannedInvalidationScope::DependencyClosure
        ) {
            removed_dependency_neighbor_ids(
                incremental.previous_snapshot.as_ref(),
                &affected_files.removed,
            )
        } else {
            HashSet::new()
        };
        if !previous_snapshot_available {
            debug!("forcing full extraction because no previous canonical snapshot is available");
            affected_files.changed = files.iter().map(|file| file.path.clone()).collect();
            affected_files.unchanged.clear();
        }

        debug!(
            changed_files = affected_files.changed.len(),
            unchanged_files = affected_files.unchanged.len(),
            removed_files = affected_files.removed.len(),
            previous_snapshot_available,
            "classified affected files"
        );
        metrics.affected_classification_ms = elapsed_ms(classification_started.elapsed());
        metrics.changed_files = affected_files.changed.len();
        metrics.unchanged_files = affected_files.unchanged.len();
        metrics.removed_files = affected_files.removed.len();

        if previous_snapshot_available
            && affected_files.changed.is_empty()
            && affected_files.removed.is_empty()
            && let Some(previous) = incremental.previous_snapshot.as_ref()
        {
            let snapshot = previous
                .snapshot
                .clone()
                .context("previous canonical snapshot has no snapshot id")?;
            metrics.files_to_extract = 0;
            metrics.entities = previous.entities.len();
            metrics.facts = previous.facts.len();
            metrics.relations = previous.relations.len();
            metrics.diagnostics = previous.diagnostics.len();
            metrics.total_ms = elapsed_ms(pipeline_started.elapsed());
            metrics.adapters = aggregate_adapter_metrics(metrics.adapters);

            info!(
                snapshot = snapshot.0,
                unchanged_files = affected_files.unchanged.len(),
                "skipping canonical snapshot creation because no source files changed"
            );

            return Ok(IndexPipelineOutput {
                snapshot,
                files,
                entities: previous.entities.clone(),
                facts: previous.facts.clone(),
                relations: previous.relations.clone(),
                diagnostics: previous.diagnostics.clone(),
                affected_files,
                metrics,
            });
        }

        let snapshot_started = Instant::now();
        let snapshot = within_operation_deadline(
            &operation,
            "store.begin_snapshot",
            self.store
                .begin_snapshot_with_context(repo.clone(), base, &operation),
        )
        .await
        .context("failed to begin snapshot")?;
        metrics.snapshot_begin_ms = elapsed_ms(snapshot_started.elapsed());
        let files_to_extract = files
            .iter()
            .filter(|file| affected_files.changed.contains(&file.path))
            .cloned()
            .collect::<Vec<_>>();
        metrics.files_to_extract = files_to_extract.len();
        info!(
            changed_files = files_to_extract.len(),
            unchanged_files = affected_files.unchanged.len(),
            removed_files = affected_files.removed.len(),
            "starting extraction"
        );
        let extraction_started = Instant::now();
        let (extracted_entities, extracted_facts, extracted_diagnostics, extraction_metrics) = self
            .extract(
                &repo,
                &snapshot,
                &files_to_extract,
                &operation,
                cancellation.clone(),
            )
            .await?;
        metrics.extraction_ms = elapsed_ms(extraction_started.elapsed());
        metrics.adapters.extend(extraction_metrics);
        check_cancelled(&cancellation)?;
        info!(
            entities = extracted_entities.len(),
            facts = extracted_facts.len(),
            diagnostics = extracted_diagnostics.len(),
            "completed extraction"
        );
        let mut affected_extracted =
            AffectedSubset::from_extracted(extracted_entities.clone(), extracted_facts.clone());
        let merge_started = Instant::now();
        let (mut entities, mut facts, mut prior_relations, mut prior_diagnostics) =
            carried_forward_read_model(
                incremental.previous_snapshot,
                &snapshot,
                &affected_files.changed,
                &affected_files.removed,
            );
        debug!(
            carried_entities = entities.len(),
            carried_facts = facts.len(),
            carried_relations = prior_relations.len(),
            carried_diagnostics = prior_diagnostics.len(),
            "carried forward previous canonical objects"
        );
        entities.extend(extracted_entities);
        facts.extend(extracted_facts);
        prior_diagnostics.extend(extracted_diagnostics);
        metrics.merge_ms = elapsed_ms(merge_started.elapsed());
        let canonicalize_extracted_started = Instant::now();
        let entities = Arc::new(canonicalize_entities(entities));
        let facts = Arc::new(canonicalize_facts(facts));
        if !removed_dependency_neighbor_ids.is_empty() {
            affected_extracted.entities.extend(
                entities
                    .iter()
                    .filter(|entity| removed_dependency_neighbor_ids.contains(&entity.id))
                    .cloned(),
            );
        }
        let refresh_derived_globally = matches!(
            invalidation_plan.scope,
            crate::PlannedInvalidationScope::Global
        );
        if refresh_derived_globally {
            let linker_names = self
                .linkers
                .iter()
                .map(|linker| linker.name())
                .collect::<BTreeSet<_>>();
            let checker_names = self
                .checkers
                .iter()
                .map(|checker| checker.name())
                .collect::<BTreeSet<_>>();
            prior_relations.retain(|relation| !has_producer(&relation.evidence, &linker_names));
            prior_diagnostics
                .retain(|diagnostic| !has_producer(&diagnostic.evidence, &checker_names));
        }
        let affected_for_derived = match invalidation_plan.scope {
            crate::PlannedInvalidationScope::Global => {
                AffectedSubset::from_extracted(entities.as_ref().clone(), facts.as_ref().clone())
            }
            crate::PlannedInvalidationScope::DependencyClosure => dependency_closure(
                &affected_extracted,
                entities.as_ref(),
                facts.as_ref(),
                &prior_relations,
            ),
            crate::PlannedInvalidationScope::FileLocal => affected_extracted.clone(),
        };
        metrics.canonicalize_ms += elapsed_ms(canonicalize_extracted_started.elapsed());
        debug!(
            entities = entities.len(),
            facts = facts.len(),
            "canonicalized merged extracted objects"
        );

        info!("starting linking");
        let linking_started = Instant::now();
        let (relations, linking_metrics) = self
            .link(
                &snapshot,
                entities.clone(),
                facts.clone(),
                &affected_for_derived,
                &operation,
                cancellation.clone(),
            )
            .await?;
        metrics.linking_ms = elapsed_ms(linking_started.elapsed());
        metrics.adapters.extend(linking_metrics);
        check_cancelled(&cancellation)?;
        info!(relations = relations.len(), "completed linking");
        let mut all_relations_for_check = prior_relations.clone();
        all_relations_for_check.extend(relations.clone());
        let mut affected_relations = affected_for_derived.relations.clone();
        affected_relations.extend(relations.clone());
        let affected_checked = affected_for_derived.with_relations(affected_relations);
        let all_relations_for_check = Arc::new(all_relations_for_check);
        info!("starting checking");
        let checking_started = Instant::now();
        let (diagnostics, checking_metrics) = self
            .check(
                &snapshot,
                entities.clone(),
                facts.clone(),
                all_relations_for_check,
                &affected_checked,
                &operation,
                cancellation.clone(),
            )
            .await?;
        metrics.checking_ms = elapsed_ms(checking_started.elapsed());
        metrics.adapters.extend(checking_metrics);
        check_cancelled(&cancellation)?;
        info!(diagnostics = diagnostics.len(), "completed checking");
        prior_relations.extend(relations);
        prior_diagnostics.extend(diagnostics);
        let canonicalize_final_started = Instant::now();
        prior_relations = canonicalize_relations(prior_relations);
        prior_diagnostics = canonicalize_diagnostics(prior_diagnostics);
        metrics.canonicalize_ms += elapsed_ms(canonicalize_final_started.elapsed());
        debug!(
            entities = entities.len(),
            facts = facts.len(),
            relations = prior_relations.len(),
            diagnostics = prior_diagnostics.len(),
            "storing canonical objects"
        );
        let entities = unwrap_or_clone_arc_vec(entities);
        let facts = unwrap_or_clone_arc_vec(facts);
        metrics.entities = entities.len();
        metrics.facts = facts.len();
        metrics.relations = prior_relations.len();
        metrics.diagnostics = prior_diagnostics.len();

        let storage_started = Instant::now();
        let storage_result: Result<()> = async {
            within_operation_deadline(
                &operation,
                "store.put_entities",
                self.store.put_entities_with_context(
                    snapshot.clone(),
                    entities.clone(),
                    &operation,
                ),
            )
            .await
            .context("failed to store entities")?;
            within_operation_deadline(
                &operation,
                "store.put_facts",
                self.store
                    .put_facts_with_context(snapshot.clone(), facts.clone(), &operation),
            )
            .await
            .context("failed to store facts")?;
            within_operation_deadline(
                &operation,
                "store.put_relations",
                self.store.put_relations_with_context(
                    snapshot.clone(),
                    prior_relations.clone(),
                    &operation,
                ),
            )
            .await
            .context("failed to store relations")?;
            within_operation_deadline(
                &operation,
                "store.put_diagnostics",
                self.store.put_diagnostics_with_context(
                    snapshot.clone(),
                    prior_diagnostics.clone(),
                    &operation,
                ),
            )
            .await
            .context("failed to store diagnostics")?;
            if commit_snapshot {
                within_operation_deadline(
                    &operation,
                    "store.commit_snapshot",
                    self.store
                        .commit_snapshot_with_context(snapshot.clone(), &operation),
                )
                .await
                .context("failed to commit snapshot")?;
            }
            Ok(())
        }
        .await;
        if let Err(error) = storage_result {
            if let Err(rollback_error) = self.store.abort_snapshot(snapshot.clone()).await {
                return Err(error.context(format!(
                    "failed to abort incomplete snapshot {}: {rollback_error}",
                    snapshot.0
                )));
            }
            return Err(error);
        }
        metrics.storage_ms = elapsed_ms(storage_started.elapsed());
        metrics.total_ms = elapsed_ms(pipeline_started.elapsed());
        metrics.adapters = aggregate_adapter_metrics(metrics.adapters);
        if commit_snapshot {
            info!(?snapshot, "committed index snapshot");
        } else {
            info!(?snapshot, "prepared uncommitted index snapshot");
        }

        Ok(IndexPipelineOutput {
            snapshot,
            files,
            entities,
            facts,
            relations: prior_relations,
            diagnostics: prior_diagnostics,
            affected_files,
            metrics,
        })
    }

    async fn discover_sources(
        &self,
        operation: &OperationContext,
        cancellation: Option<CancellationToken>,
    ) -> Result<(Vec<SourceFile>, Vec<AdapterRunMetrics>)> {
        crate::pipeline_source::discover(&self.sources, operation, cancellation).await
    }

    async fn extract(
        &self,
        repo: &RepoId,
        snapshot: &SnapshotId,
        files: &[SourceFile],
        operation: &OperationContext,
        cancellation: Option<CancellationToken>,
    ) -> Result<(
        Vec<Entity>,
        Vec<Fact>,
        Vec<Diagnostic>,
        Vec<AdapterRunMetrics>,
    )> {
        crate::pipeline_extract::extract(
            &self.extractors,
            repo,
            snapshot,
            files,
            self.extraction_concurrency,
            self.max_extraction_bytes_in_flight,
            &self.extraction_concurrency_by_adapter,
            operation,
            cancellation,
        )
        .await
    }

    async fn link(
        &self,
        snapshot: &SnapshotId,
        entities: Arc<Vec<Entity>>,
        facts: Arc<Vec<Fact>>,
        affected: &AffectedSubset,
        operation: &OperationContext,
        cancellation: Option<CancellationToken>,
    ) -> Result<(Vec<Relation>, Vec<AdapterRunMetrics>)> {
        crate::pipeline_link::link(
            &self.linkers,
            snapshot,
            entities,
            facts,
            affected,
            operation,
            cancellation,
        )
        .await
    }

    async fn check(
        &self,
        snapshot: &SnapshotId,
        entities: Arc<Vec<Entity>>,
        facts: Arc<Vec<Fact>>,
        relations: Arc<Vec<Relation>>,
        affected: &AffectedSubset,
        operation: &OperationContext,
        cancellation: Option<CancellationToken>,
    ) -> Result<(Vec<Diagnostic>, Vec<AdapterRunMetrics>)> {
        crate::pipeline_check::check(
            &self.checkers,
            snapshot,
            entities,
            facts,
            relations,
            affected,
            operation,
            cancellation,
        )
        .await
    }
}

fn has_producer(evidence: &[Evidence], producers: &BTreeSet<&str>) -> bool {
    evidence.iter().any(|item| {
        item.extractor
            .as_deref()
            .is_some_and(|producer| producers.contains(producer))
    })
}

pub(crate) fn validate_entities(adapter: &str, entities: &[Entity]) -> Result<()> {
    let mut report = AdapterValidationReport::new(adapter);

    for entity in entities {
        if entity.ownership.is_empty() {
            report.push(
                "entity",
                entity.id.0.clone(),
                MissingCanonicalMetadata::Ownership,
            );
        }
    }

    report.finish()
}

pub(crate) fn validate_facts(adapter: &str, facts: &[Fact]) -> Result<()> {
    let mut report = AdapterValidationReport::new(adapter);

    for fact in facts {
        if fact.evidence.is_empty() {
            report.push(
                "fact",
                fact.id.0.clone(),
                MissingCanonicalMetadata::Evidence,
            );
        }

        if fact.ownership.is_empty() {
            report.push(
                "fact",
                fact.id.0.clone(),
                MissingCanonicalMetadata::Ownership,
            );
        }
    }

    report.finish()
}

pub(crate) fn validate_relations(adapter: &str, relations: &[Relation]) -> Result<()> {
    let mut report = AdapterValidationReport::new(adapter);

    for relation in relations {
        if relation.evidence.is_empty() {
            report.push(
                "relation",
                relation.id.0.clone(),
                MissingCanonicalMetadata::Evidence,
            );
        }

        if relation.ownership.is_empty() {
            report.push(
                "relation",
                relation.id.0.clone(),
                MissingCanonicalMetadata::Ownership,
            );
        }
    }

    report.finish()
}

pub(crate) fn validate_diagnostics(adapter: &str, diagnostics: &[Diagnostic]) -> Result<()> {
    let mut report = AdapterValidationReport::new(adapter);

    for diagnostic in diagnostics {
        if diagnostic.evidence.is_empty() {
            report.push(
                "diagnostic",
                diagnostic.id.0.clone(),
                MissingCanonicalMetadata::Evidence,
            );
        }

        if diagnostic.ownership.is_empty() {
            report.push(
                "diagnostic",
                diagnostic.id.0.clone(),
                MissingCanonicalMetadata::Ownership,
            );
        }
    }

    report.finish()
}

fn carried_forward_read_model(
    previous: Option<CanonicalSnapshot>,
    snapshot: &SnapshotId,
    changed_paths: &BTreeSet<String>,
    removed_paths: &BTreeSet<String>,
) -> (Vec<Entity>, Vec<Fact>, Vec<Relation>, Vec<Diagnostic>) {
    let Some(previous) = previous else {
        return (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    };

    let affected_paths = changed_paths
        .iter()
        .chain(removed_paths.iter())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut entities = previous
        .entities
        .into_iter()
        .filter(|entity| !entity_owned_by_any_path(entity, &affected_paths))
        .map(|mut entity| {
            replace_payload_snapshot(&mut entity.payload, snapshot);
            entity
        })
        .collect::<Vec<_>>();
    let entity_ids = entities
        .iter()
        .map(|entity| entity.id.clone())
        .collect::<HashSet<_>>();
    let facts = previous
        .facts
        .into_iter()
        .filter(|fact| {
            entity_ids.contains(&fact.subject)
                && fact
                    .object
                    .as_ref()
                    .is_none_or(|object| entity_ids.contains(object))
                && !fact_owned_by_any_path(fact, &affected_paths)
        })
        .map(|mut fact| {
            fact.snapshot = snapshot.clone();
            fact
        })
        .collect::<Vec<_>>();
    let relations = previous
        .relations
        .into_iter()
        .filter(|relation| {
            entity_ids.contains(&relation.from)
                && entity_ids.contains(&relation.to)
                && !relation_owned_by_any_path(relation, &affected_paths)
        })
        .map(|mut relation| {
            relation.snapshot = snapshot.clone();
            relation
        })
        .collect::<Vec<_>>();
    let diagnostics = previous
        .diagnostics
        .into_iter()
        .filter(|diagnostic| {
            diagnostic
                .entities
                .iter()
                .all(|entity| entity_ids.contains(entity))
                && !diagnostic_owned_by_any_path(diagnostic, &affected_paths)
                && !diagnostic_invalidated_by_changed_path(diagnostic, &affected_paths)
        })
        .map(|mut diagnostic| {
            diagnostic.snapshot = snapshot.clone();
            diagnostic
        })
        .collect::<Vec<_>>();

    entities.sort_by(|left, right| left.id.0.cmp(&right.id.0));

    (entities, facts, relations, diagnostics)
}

fn removed_dependency_neighbor_ids(
    previous: Option<&CanonicalSnapshot>,
    removed_paths: &BTreeSet<String>,
) -> HashSet<athanor_domain::EntityId> {
    let Some(previous) = previous else {
        return HashSet::new();
    };
    let removed_entity_ids = previous
        .entities
        .iter()
        .filter(|entity| entity_owned_by_any_path(entity, removed_paths))
        .map(|entity| entity.id.clone())
        .collect::<HashSet<_>>();
    previous
        .relations
        .iter()
        .filter_map(|relation| {
            if removed_entity_ids.contains(&relation.from) {
                Some(relation.to.clone())
            } else if removed_entity_ids.contains(&relation.to) {
                Some(relation.from.clone())
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use athanor_core::{CanonicalSnapshot, CoreResult, ExtractOutput, OperationContext};
    use athanor_domain::{
        DiagnosticId, DiagnosticKind, DiagnosticStatus, EntityId, EntityKind, Evidence,
        EvidenceStatus, FactId, FactKind, Ownership, RelationId, RelationKind, RelationStatus,
        Severity, SourceLocation, StableKey,
    };
    use serde_json::json;
    use std::sync::Mutex;

    use super::*;
    use crate::index_state::FileState;
    use crate::transient_store::TransientKnowledgeStore;

    #[test]
    fn prunes_relation_when_any_owned_source_changes() {
        let snapshot = SnapshotId("snap_next".to_string());
        let left = entity("ent_left", "docs/left.md");
        let right = entity("ent_right", "docs/right.md");
        let relation = Relation {
            id: RelationId("rel_cross_file".to_string()),
            kind: RelationKind::Documents,
            from: left.id.clone(),
            to: right.id.clone(),
            status: RelationStatus::Verified,
            confidence: 1.0,
            evidence: Vec::new(),
            ownership: vec![ownership("docs/left.md"), ownership("docs/right.md")],
            snapshot: SnapshotId("snap_previous".to_string()),
            payload: json!({}),
        };

        let (_, _, relations, _) = carried_forward_read_model(
            Some(CanonicalSnapshot {
                snapshot: Some(SnapshotId("snap_previous".to_string())),
                entities: vec![left, right],
                facts: Vec::new(),
                relations: vec![relation],
                diagnostics: Vec::new(),
            }),
            &snapshot,
            &BTreeSet::from(["docs/right.md".to_string()]),
            &BTreeSet::new(),
        );

        assert!(relations.is_empty());
    }

    #[test]
    fn finds_surviving_neighbors_of_removed_entities_for_dependency_closure() {
        let removed = entity("ent_removed", "src/removed.rs");
        let survivor = entity("ent_survivor", "src/survivor.rs");
        let neighbors = removed_dependency_neighbor_ids(
            Some(&CanonicalSnapshot {
                entities: vec![removed.clone(), survivor.clone()],
                relations: vec![relation(
                    "rel_removed_survivor",
                    "ent_removed",
                    "ent_survivor",
                )],
                ..CanonicalSnapshot::default()
            }),
            &BTreeSet::from(["src/removed.rs".to_string()]),
        );

        assert_eq!(neighbors, HashSet::from([survivor.id]));
    }

    #[test]
    fn prunes_rustok_ffa_diagnostics_when_related_plan_changes() {
        let snapshot = SnapshotId("snap_next".to_string());
        let entity = entity("ent_left", "docs/modules/registry.md");
        let mut diagnostic = diagnostic(
            "diag_rustok_ffa_docs_drift",
            "stale docs drift",
            "snap_previous",
        );
        diagnostic.kind = DiagnosticKind::Other("rustok_ffa_docs_drift".to_string());
        diagnostic.evidence = vec![evidence("docs/modules/registry.md")];
        diagnostic.ownership = vec![ownership("docs/modules/registry.md")];

        let (_, _, _, diagnostics) = carried_forward_read_model(
            Some(CanonicalSnapshot {
                snapshot: Some(SnapshotId("snap_previous".to_string())),
                entities: vec![entity],
                facts: Vec::new(),
                relations: Vec::new(),
                diagnostics: vec![diagnostic],
            }),
            &snapshot,
            &BTreeSet::from(["crates/rustok-ai-content/docs/implementation-plan.md".to_string()]),
            &BTreeSet::new(),
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn canonicalizes_diagnostics_by_id_with_latest_winning() {
        let previous = diagnostic("diag_duplicate", "old title", "snap_previous");
        let current = diagnostic("diag_duplicate", "new title", "snap_next");
        let other = diagnostic("diag_other", "other title", "snap_next");

        let diagnostics = canonicalize_diagnostics(vec![previous, current, other]);

        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].id.0, "diag_duplicate");
        assert_eq!(diagnostics[0].title, "new title");
        assert_eq!(diagnostics[1].id.0, "diag_other");
    }

    #[tokio::test]
    async fn shares_full_context_between_linkers_and_checkers() {
        let tracker = Arc::new(Mutex::new(SharedInputPointers::default()));
        let pipeline = IndexPipeline::new(TransientKnowledgeStore::new())
            .source(TestSource)
            .extractor(TestExtractor)
            .linker(CapturingLinker {
                tracker: tracker.clone(),
            })
            .checker(CapturingChecker {
                tracker: tracker.clone(),
            });

        let output = pipeline
            .run(
                RepoId("repo_shared_context".to_string()),
                SnapshotBase {
                    branch: None,
                    commit: None,
                    parent_snapshot: None,
                    working_tree: true,
                },
            )
            .await
            .expect("pipeline should run");

        assert_eq!(output.entities.len(), 1);
        assert_eq!(output.facts.len(), 1);
        assert_eq!(output.relations.len(), 1);
        assert_eq!(output.diagnostics.len(), 1);

        let pointers = tracker.lock().unwrap();
        assert_eq!(pointers.linker_entities, pointers.checker_entities);
        assert_eq!(pointers.linker_facts, pointers.checker_facts);
    }

    #[tokio::test]
    async fn operation_context_deadline_stops_pipeline_before_source_discovery() {
        let pipeline = IndexPipeline::new(TransientKnowledgeStore::new()).source(TestSource);
        let error = pipeline
            .run_with_incremental_operation_context(
                RepoId("repo_operation_deadline".to_string()),
                SnapshotBase {
                    branch: None,
                    commit: None,
                    parent_snapshot: None,
                    working_tree: true,
                },
                IncrementalIndexContext::default(),
                OperationContext::new("deadline-test").with_deadline_unix_ms(0),
            )
            .await
            .expect_err("elapsed operation deadline should stop source discovery");

        assert!(error.to_string().contains("deadline"));
    }

    #[tokio::test]
    async fn dependency_closure_reaches_connected_carried_entities() {
        let affected_entities = Arc::new(Mutex::new(Vec::new()));
        let carried_neighbor = entity("ent_neighbor", "docs/neighbor.md");
        let previous = CanonicalSnapshot {
            snapshot: Some(SnapshotId("snap_previous".to_string())),
            entities: vec![
                entity("ent_left", "docs/shared.md"),
                carried_neighbor.clone(),
            ],
            relations: vec![relation("rel_previous", "ent_left", "ent_neighbor")],
            ..CanonicalSnapshot::default()
        };
        let pipeline = IndexPipeline::new(TransientKnowledgeStore::new())
            .source(TestSource)
            .extractor(TestExtractor)
            .linker(DependencyCapturingLinker {
                affected_entities: affected_entities.clone(),
            });

        pipeline
            .run_with_incremental(
                RepoId("repo_dependency_closure".to_string()),
                SnapshotBase {
                    branch: None,
                    commit: None,
                    parent_snapshot: None,
                    working_tree: true,
                },
                IncrementalIndexContext {
                    previous_state: IndexState {
                        schema: "test".to_string(),
                        snapshot: Some("snap_previous".to_string()),
                        files: BTreeMap::from([(
                            "docs/shared.md".to_string(),
                            FileState {
                                content_hash: Some("old".to_string()),
                                language_hint: Some("markdown".to_string()),
                            },
                        )]),
                    },
                    previous_snapshot: Some(previous),
                },
            )
            .await
            .expect("incremental pipeline should run");

        assert_eq!(
            *affected_entities.lock().unwrap(),
            vec!["ent_left".to_string(), "ent_neighbor".to_string()]
        );
    }

    #[test]
    fn rejects_fact_without_ownership() {
        let mut fact = fact("fact_without_ownership");
        fact.ownership.clear();

        let error = validate_facts("test-extractor", &[fact]).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("fact fact_without_ownership missing ownership")
        );
    }

    #[test]
    fn rejects_relation_without_evidence() {
        let mut relation = relation("rel_without_evidence", "ent_left", "ent_right");
        relation.evidence.clear();

        let error = validate_relations("test-linker", &[relation]).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("relation rel_without_evidence missing evidence")
        );
    }

    #[test]
    fn reports_multiple_validation_issues() {
        let mut first = relation("rel_missing_both", "ent_left", "ent_right");
        first.evidence.clear();
        first.ownership.clear();
        let mut second = relation("rel_missing_ownership", "ent_left", "ent_right");
        second.ownership.clear();

        let error = validate_relations("test-linker", &[first, second]).unwrap_err();
        let message = error.to_string();

        assert!(
            message.contains("adapter test-linker emitted invalid canonical output (3 issues)")
        );
        assert!(message.contains("relation rel_missing_both missing evidence"));
        assert!(message.contains("relation rel_missing_both missing ownership"));
        assert!(message.contains("relation rel_missing_ownership missing ownership"));
    }

    fn entity(id: &str, path: &str) -> Entity {
        Entity {
            id: EntityId(id.to_string()),
            stable_key: StableKey(format!("file://{path}")),
            kind: EntityKind::File,
            name: path.to_string(),
            title: None,
            source: Some(SourceLocation {
                path: path.to_string(),
                line_start: None,
                line_end: None,
            }),
            language: None,
            aliases: Vec::new(),
            ownership: vec![ownership(path)],
            payload: json!({}),
        }
    }

    fn fact(id: &str) -> Fact {
        Fact {
            id: FactId(id.to_string()),
            kind: FactKind::FileDiscovered,
            subject: EntityId("ent_left".to_string()),
            object: None,
            value: json!({}),
            evidence: vec![evidence("docs/left.md")],
            ownership: vec![ownership("docs/left.md")],
            snapshot: SnapshotId("snap_test".to_string()),
            extractor: "test".to_string(),
            confidence: 1.0,
        }
    }

    fn relation(id: &str, from: &str, to: &str) -> Relation {
        Relation {
            id: RelationId(id.to_string()),
            kind: RelationKind::Documents,
            from: EntityId(from.to_string()),
            to: EntityId(to.to_string()),
            status: RelationStatus::Verified,
            confidence: 1.0,
            evidence: vec![evidence("docs/left.md")],
            ownership: vec![ownership("docs/left.md")],
            snapshot: SnapshotId("snap_test".to_string()),
            payload: json!({}),
        }
    }

    fn diagnostic(id: &str, title: &str, snapshot: &str) -> Diagnostic {
        Diagnostic {
            id: DiagnosticId(id.to_string()),
            kind: DiagnosticKind::MissingDocumentation,
            severity: Severity::Medium,
            status: DiagnosticStatus::Open,
            title: title.to_string(),
            message: title.to_string(),
            entities: vec![EntityId("ent_left".to_string())],
            evidence: vec![evidence("docs/left.md")],
            ownership: vec![ownership("docs/left.md")],
            snapshot: SnapshotId(snapshot.to_string()),
            suggested_fix: None,
            payload: json!({}),
        }
    }

    fn evidence(path: &str) -> Evidence {
        Evidence {
            source_file: Some(path.to_string()),
            line_start: None,
            line_end: None,
            extractor: Some("test".to_string()),
            commit_hash: None,
            confidence: 1.0,
            status: EvidenceStatus::Verified,
        }
    }

    fn ownership(path: &str) -> Ownership {
        Ownership {
            source_file: path.to_string(),
        }
    }

    #[derive(Default)]
    struct SharedInputPointers {
        linker_entities: Option<usize>,
        linker_facts: Option<usize>,
        checker_entities: Option<usize>,
        checker_facts: Option<usize>,
    }

    struct TestSource;

    #[async_trait]
    impl SourceProvider for TestSource {
        fn name(&self) -> &str {
            "test-source"
        }

        async fn discover(&self) -> CoreResult<Vec<SourceFile>> {
            Ok(vec![SourceFile {
                path: "docs/shared.md".to_string(),
                language_hint: Some("markdown".to_string()),
                content_hash: Some("shared".to_string()),
                content: Some("# Shared".to_string()),
            }])
        }
    }

    struct TestExtractor;

    #[async_trait]
    impl Extractor for TestExtractor {
        fn name(&self) -> &str {
            "test-extractor"
        }

        fn supports(&self, _source: &SourceFile) -> bool {
            true
        }

        async fn extract(&self, _input: ExtractInput) -> CoreResult<ExtractOutput> {
            Ok(ExtractOutput {
                entities: vec![entity("ent_left", "docs/shared.md")],
                facts: vec![fact("fact_shared")],
                diagnostics: Vec::new(),
            })
        }
    }

    struct CapturingLinker {
        tracker: Arc<Mutex<SharedInputPointers>>,
    }

    #[async_trait]
    impl Linker for CapturingLinker {
        fn name(&self) -> &str {
            "capturing-linker"
        }

        async fn link(&self, input: LinkInput) -> CoreResult<Vec<Relation>> {
            let mut tracker = self.tracker.lock().unwrap();
            tracker.linker_entities = Some(Arc::as_ptr(&input.entities) as usize);
            tracker.linker_facts = Some(Arc::as_ptr(&input.facts) as usize);
            drop(tracker);
            Ok(vec![relation("rel_shared", "ent_left", "ent_left")])
        }
    }

    struct CapturingChecker {
        tracker: Arc<Mutex<SharedInputPointers>>,
    }

    #[async_trait]
    impl Checker for CapturingChecker {
        fn name(&self) -> &str {
            "capturing-checker"
        }

        async fn check(&self, input: CheckInput) -> CoreResult<Vec<Diagnostic>> {
            assert_eq!(input.relations.len(), 1);
            let mut tracker = self.tracker.lock().unwrap();
            tracker.checker_entities = Some(Arc::as_ptr(&input.entities) as usize);
            tracker.checker_facts = Some(Arc::as_ptr(&input.facts) as usize);
            Ok(vec![diagnostic(
                "diag_shared",
                "shared context was checked",
                &input.snapshot.0,
            )])
        }
    }

    struct DependencyCapturingLinker {
        affected_entities: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait]
    impl Linker for DependencyCapturingLinker {
        fn name(&self) -> &str {
            "dependency-capturing-linker"
        }

        fn invalidation_policy(&self) -> athanor_core::InvalidationPolicy {
            athanor_core::InvalidationPolicy {
                on_change: athanor_core::InvalidationScope::DependencyClosure,
                on_add: athanor_core::InvalidationScope::DependencyClosure,
                on_remove: athanor_core::InvalidationScope::DependencyClosure,
            }
        }

        async fn link(&self, input: LinkInput) -> CoreResult<Vec<Relation>> {
            let mut ids = input
                .affected
                .entities
                .iter()
                .map(|entity| entity.id.0.clone())
                .collect::<Vec<_>>();
            ids.sort();
            *self.affected_entities.lock().unwrap() = ids;
            Ok(Vec::new())
        }
    }
}
