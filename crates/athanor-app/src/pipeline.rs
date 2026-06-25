use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use athanor_core::{
    AffectedSubset, CanonicalSnapshot, CheckInput, Checker, ExtractInput, Extractor,
    KnowledgeStore, LinkInput, Linker, SourceFile, SourceProvider,
};
use athanor_domain::{Diagnostic, Entity, Fact, Relation, RepoId, SnapshotBase, SnapshotId};
use serde::Serialize;
use serde_json::Value;
use tracing::{Instrument, debug, debug_span, error, info};

use crate::{AffectedFileSet, CancellationToken, IndexState};
use futures::stream::{self, StreamExt};

const EXTRACTION_CONCURRENCY_LIMIT: usize = 16;
pub const INDEX_METRICS_SCHEMA: &str = "athanor.index_metrics.v1";

pub struct IndexPipeline {
    store: Box<dyn KnowledgeStore>,
    sources: Vec<Box<dyn SourceProvider>>,
    extractors: Vec<Box<dyn Extractor>>,
    linkers: Vec<Box<dyn Linker>>,
    checkers: Vec<Box<dyn Checker>>,
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
    pub changed_files: usize,
    pub unchanged_files: usize,
    pub removed_files: usize,
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
        self.run_with_incremental_inner(repo, base, incremental, None)
            .await
    }

    pub async fn run_with_incremental_cancellable(
        self,
        repo: RepoId,
        base: SnapshotBase,
        incremental: IncrementalIndexContext,
        cancellation: CancellationToken,
    ) -> Result<IndexPipelineOutput> {
        self.run_with_incremental_inner(repo, base, incremental, Some(cancellation))
            .await
    }

    async fn run_with_incremental_inner(
        self,
        repo: RepoId,
        base: SnapshotBase,
        incremental: IncrementalIndexContext,
        cancellation: Option<CancellationToken>,
    ) -> Result<IndexPipelineOutput> {
        let pipeline_started = Instant::now();
        let mut metrics = IndexPipelineMetrics {
            schema: INDEX_METRICS_SCHEMA,
            ..IndexPipelineMetrics::default()
        };
        check_cancelled(&cancellation)?;
        let previous_snapshot_available = incremental.previous_snapshot.is_some();

        info!("starting source discovery");
        let source_started = Instant::now();
        let (files, source_metrics) = self.discover_sources().await?;
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
        if has_added_files || !affected_files.removed.is_empty() {
            debug!(
                added_files = has_added_files,
                removed_files = affected_files.removed.len(),
                "forcing full extraction because file additions or removals can affect absence diagnostics"
            );
            affected_files.changed = files.iter().map(|file| file.path.clone()).collect();
            affected_files.unchanged.clear();
        }

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

        let snapshot_started = Instant::now();
        let snapshot = self
            .store
            .begin_snapshot(repo.clone(), base)
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
        let (extracted_entities, extracted_facts, extraction_metrics) =
            self.extract(&repo, &snapshot, &files_to_extract).await?;
        metrics.extraction_ms = elapsed_ms(extraction_started.elapsed());
        metrics.adapters.extend(extraction_metrics);
        check_cancelled(&cancellation)?;
        info!(
            entities = extracted_entities.len(),
            facts = extracted_facts.len(),
            "completed extraction"
        );
        let affected_extracted =
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
        metrics.merge_ms = elapsed_ms(merge_started.elapsed());
        let canonicalize_extracted_started = Instant::now();
        let entities = Arc::new(canonicalize_entities(entities));
        let facts = Arc::new(canonicalize_facts(facts));
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
                &affected_extracted,
            )
            .await?;
        metrics.linking_ms = elapsed_ms(linking_started.elapsed());
        metrics.adapters.extend(linking_metrics);
        check_cancelled(&cancellation)?;
        info!(relations = relations.len(), "completed linking");
        let mut all_relations_for_check = prior_relations.clone();
        all_relations_for_check.extend(relations.clone());
        let affected_checked = affected_extracted.with_relations(relations.clone());
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
        self.store
            .put_entities(snapshot.clone(), entities.clone())
            .await
            .context("failed to store entities")?;
        self.store
            .put_facts(snapshot.clone(), facts.clone())
            .await
            .context("failed to store facts")?;
        self.store
            .put_relations(snapshot.clone(), prior_relations.clone())
            .await
            .context("failed to store relations")?;
        self.store
            .put_diagnostics(snapshot.clone(), prior_diagnostics.clone())
            .await
            .context("failed to store diagnostics")?;
        self.store
            .commit_snapshot(snapshot.clone())
            .await
            .context("failed to commit snapshot")?;
        metrics.storage_ms = elapsed_ms(storage_started.elapsed());
        metrics.total_ms = elapsed_ms(pipeline_started.elapsed());
        metrics.adapters = aggregate_adapter_metrics(metrics.adapters);
        info!(?snapshot, "committed index snapshot");

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

    async fn discover_sources(&self) -> Result<(Vec<SourceFile>, Vec<AdapterRunMetrics>)> {
        let mut files = Vec::new();
        let mut metrics = Vec::new();

        for source in &self.sources {
            let source_name = source.name();
            let span = debug_span!("discover_source", source = source_name);
            let started = Instant::now();
            let discovered = source
                .discover()
                .instrument(span)
                .await
                .with_context(|| format!("source {} failed", source_name))?;
            let duration_ms = elapsed_ms(started.elapsed());
            debug!(
                source = source_name,
                file_count = discovered.len(),
                "source discovery produced files"
            );
            metrics.push(AdapterRunMetrics {
                phase: "source",
                adapter: source_name.to_string(),
                runs: 1,
                duration_ms,
                output_files: discovered.len(),
                ..AdapterRunMetrics::default()
            });
            files.extend(discovered);
        }

        Ok((files, metrics))
    }

    async fn extract(
        &self,
        repo: &RepoId,
        snapshot: &SnapshotId,
        files: &[SourceFile],
    ) -> Result<(Vec<Entity>, Vec<Fact>, Vec<AdapterRunMetrics>)> {
        let tasks = files
            .iter()
            .flat_map(|source| {
                self.extractors
                    .iter()
                    .filter(move |extractor| extractor.supports(source))
                    .map(move |extractor| (extractor.as_ref(), source.clone()))
            })
            .collect::<Vec<_>>();
        info!(
            task_count = tasks.len(),
            concurrency = EXTRACTION_CONCURRENCY_LIMIT,
            "queued extraction tasks"
        );

        let mut outputs = stream::iter(tasks)
            .map(|(extractor, source)| async move {
                let extractor_name = extractor.name();
                let started = Instant::now();
                let span = debug_span!(
                    "extract_source",
                    extractor = extractor_name,
                    file = %source.path
                );
                let input = ExtractInput {
                    repo: repo.clone(),
                    snapshot: snapshot.clone(),
                    source,
                };
                let output = async {
                    extractor
                        .extract(input)
                        .await
                        .with_context(|| format!("extractor {} failed", extractor_name))
                }
                .instrument(span)
                .await?;
                let duration_ms = elapsed_ms(started.elapsed());

                validate_entities(extractor_name, &output.entities)?;
                validate_facts(extractor_name, &output.facts)?;
                debug!(
                    extractor = extractor_name,
                    entities = output.entities.len(),
                    facts = output.facts.len(),
                    "extractor emitted canonical objects"
                );
                let metrics = AdapterRunMetrics {
                    phase: "extractor",
                    adapter: extractor_name.to_string(),
                    runs: 1,
                    duration_ms,
                    input_files: 1,
                    output_entities: output.entities.len(),
                    output_facts: output.facts.len(),
                    ..AdapterRunMetrics::default()
                };
                Ok::<_, anyhow::Error>(ExtractorTaskOutput { output, metrics })
            })
            .buffer_unordered(EXTRACTION_CONCURRENCY_LIMIT);

        let mut entities = Vec::new();
        let mut facts = Vec::new();
        let mut metrics = Vec::new();

        while let Some(output) = outputs.next().await {
            match output {
                Ok(output) => {
                    entities.extend(output.output.entities);
                    facts.extend(output.output.facts);
                    metrics.push(output.metrics);
                }
                Err(error) => {
                    error!(%error, "extraction failed");
                    return Err(error);
                }
            }
        }

        Ok((
            canonicalize_entities(entities),
            canonicalize_facts(facts),
            metrics,
        ))
    }

    async fn link(
        &self,
        snapshot: &SnapshotId,
        entities: Arc<Vec<Entity>>,
        facts: Arc<Vec<Fact>>,
        affected: &AffectedSubset,
    ) -> Result<(Vec<Relation>, Vec<AdapterRunMetrics>)> {
        let mut relations = Vec::new();
        let mut metrics = Vec::new();

        for linker in &self.linkers {
            let linker_name = linker.name();
            let span = debug_span!("link_canonical_objects", linker = linker_name);
            debug!(
                linker = linker_name,
                entities = entities.len(),
                facts = facts.len(),
                affected_entities = affected.entities.len(),
                affected_facts = affected.facts.len(),
                affected_relations = affected.relations.len(),
                "running linker"
            );
            let started = Instant::now();
            let output = linker
                .link(LinkInput {
                    snapshot: snapshot.clone(),
                    entities: entities.clone(),
                    facts: facts.clone(),
                    affected: affected.clone(),
                })
                .instrument(span)
                .await
                .inspect_err(|error| error!(linker = linker.name(), %error, "linker failed"))
                .with_context(|| format!("linker {} failed", linker_name))?;
            let duration_ms = elapsed_ms(started.elapsed());

            validate_relations(linker_name, &output)?;
            debug!(
                linker = linker_name,
                relations = output.len(),
                "linker emitted relations"
            );
            metrics.push(AdapterRunMetrics {
                phase: "linker",
                adapter: linker_name.to_string(),
                runs: 1,
                duration_ms,
                input_entities: entities.len(),
                input_facts: facts.len(),
                output_relations: output.len(),
                ..AdapterRunMetrics::default()
            });
            relations.extend(output);
        }

        Ok((relations, metrics))
    }

    async fn check(
        &self,
        snapshot: &SnapshotId,
        entities: Arc<Vec<Entity>>,
        facts: Arc<Vec<Fact>>,
        relations: Arc<Vec<Relation>>,
        affected: &AffectedSubset,
    ) -> Result<(Vec<Diagnostic>, Vec<AdapterRunMetrics>)> {
        let mut diagnostics = Vec::new();
        let mut metrics = Vec::new();

        for checker in &self.checkers {
            let checker_name = checker.name();
            let span = debug_span!("check_canonical_objects", checker = checker_name);
            debug!(
                checker = checker_name,
                entities = entities.len(),
                facts = facts.len(),
                relations = relations.len(),
                affected_entities = affected.entities.len(),
                affected_facts = affected.facts.len(),
                affected_relations = affected.relations.len(),
                "running checker"
            );
            let started = Instant::now();
            let output = checker
                .check(CheckInput {
                    snapshot: snapshot.clone(),
                    entities: entities.clone(),
                    facts: facts.clone(),
                    relations: relations.clone(),
                    affected: affected.clone(),
                })
                .instrument(span)
                .await
                .inspect_err(|error| error!(checker = checker.name(), %error, "checker failed"))
                .with_context(|| format!("checker {} failed", checker_name))?;
            let duration_ms = elapsed_ms(started.elapsed());

            validate_diagnostics(checker_name, &output)?;
            debug!(
                checker = checker_name,
                diagnostics = output.len(),
                "checker emitted diagnostics"
            );
            metrics.push(AdapterRunMetrics {
                phase: "checker",
                adapter: checker_name.to_string(),
                runs: 1,
                duration_ms,
                input_entities: entities.len(),
                input_facts: facts.len(),
                input_relations: relations.len(),
                output_diagnostics: output.len(),
                ..AdapterRunMetrics::default()
            });
            diagnostics.extend(output);
        }

        Ok((diagnostics, metrics))
    }
}

struct ExtractorTaskOutput {
    output: athanor_core::ExtractOutput,
    metrics: AdapterRunMetrics,
}

fn elapsed_ms(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

fn aggregate_adapter_metrics(metrics: Vec<AdapterRunMetrics>) -> Vec<AdapterRunMetrics> {
    let mut by_adapter = BTreeMap::<(&'static str, String), AdapterRunMetrics>::new();

    for metric in metrics {
        let key = (metric.phase, metric.adapter.clone());
        let entry = by_adapter.entry(key).or_insert_with(|| AdapterRunMetrics {
            phase: metric.phase,
            adapter: metric.adapter.clone(),
            ..AdapterRunMetrics::default()
        });
        entry.runs += metric.runs;
        entry.duration_ms = entry.duration_ms.saturating_add(metric.duration_ms);
        entry.input_files = entry.input_files.saturating_add(metric.input_files);
        entry.input_entities = entry.input_entities.saturating_add(metric.input_entities);
        entry.input_facts = entry.input_facts.saturating_add(metric.input_facts);
        entry.input_relations = entry.input_relations.saturating_add(metric.input_relations);
        entry.output_files = entry.output_files.saturating_add(metric.output_files);
        entry.output_entities = entry.output_entities.saturating_add(metric.output_entities);
        entry.output_facts = entry.output_facts.saturating_add(metric.output_facts);
        entry.output_relations = entry
            .output_relations
            .saturating_add(metric.output_relations);
        entry.output_diagnostics = entry
            .output_diagnostics
            .saturating_add(metric.output_diagnostics);
        entry.validation_issues = entry
            .validation_issues
            .saturating_add(metric.validation_issues);
        entry.timeout_count = entry.timeout_count.saturating_add(metric.timeout_count);
        entry.stdin_bytes = add_optional_bytes(entry.stdin_bytes, metric.stdin_bytes);
        entry.stdout_bytes = add_optional_bytes(entry.stdout_bytes, metric.stdout_bytes);
        entry.stderr_bytes = add_optional_bytes(entry.stderr_bytes, metric.stderr_bytes);
    }

    by_adapter.into_values().collect()
}

fn add_optional_bytes(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.saturating_add(right)),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

fn unwrap_or_clone_arc_vec<T: Clone>(items: Arc<Vec<T>>) -> Vec<T> {
    Arc::try_unwrap(items).unwrap_or_else(|items| (*items).clone())
}

fn check_cancelled(cancellation: &Option<CancellationToken>) -> Result<()> {
    if let Some(cancellation) = cancellation {
        cancellation.check()?;
    }
    Ok(())
}

fn validate_entities(adapter: &str, entities: &[Entity]) -> Result<()> {
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

fn validate_facts(adapter: &str, facts: &[Fact]) -> Result<()> {
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

fn validate_relations(adapter: &str, relations: &[Relation]) -> Result<()> {
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

fn validate_diagnostics(adapter: &str, diagnostics: &[Diagnostic]) -> Result<()> {
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
        })
        .map(|mut diagnostic| {
            diagnostic.snapshot = snapshot.clone();
            diagnostic
        })
        .collect::<Vec<_>>();

    entities.sort_by(|left, right| left.id.0.cmp(&right.id.0));

    (entities, facts, relations, diagnostics)
}

fn canonicalize_entities(entities: Vec<Entity>) -> Vec<Entity> {
    let mut by_id = BTreeMap::new();
    for entity in entities {
        by_id.insert(entity.id.0.clone(), entity);
    }
    by_id.into_values().collect()
}

fn canonicalize_facts(facts: Vec<Fact>) -> Vec<Fact> {
    let mut by_id = BTreeMap::new();
    for fact in facts {
        by_id.insert(fact.id.0.clone(), fact);
    }
    by_id.into_values().collect()
}

fn canonicalize_relations(relations: Vec<Relation>) -> Vec<Relation> {
    let mut by_id = BTreeMap::new();
    for relation in relations {
        by_id.insert(relation.id.0.clone(), relation);
    }
    by_id.into_values().collect()
}

fn canonicalize_diagnostics(diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    let mut by_id = BTreeMap::new();
    for diagnostic in diagnostics {
        by_id.insert(diagnostic.id.0.clone(), diagnostic);
    }
    by_id.into_values().collect()
}

fn entity_owned_by_any_path(entity: &Entity, paths: &BTreeSet<String>) -> bool {
    ownership_belongs_to_any_path(&entity.ownership, paths)
        || belongs_to_any_path(
            entity.source.as_ref().map(|source| source.path.as_str()),
            paths,
        )
}

fn fact_owned_by_any_path(fact: &Fact, paths: &BTreeSet<String>) -> bool {
    ownership_belongs_to_any_path(&fact.ownership, paths)
        || evidence_belongs_to_any_path(&fact.evidence, paths)
}

fn relation_owned_by_any_path(relation: &Relation, paths: &BTreeSet<String>) -> bool {
    ownership_belongs_to_any_path(&relation.ownership, paths)
        || evidence_belongs_to_any_path(&relation.evidence, paths)
}

fn diagnostic_owned_by_any_path(diagnostic: &Diagnostic, paths: &BTreeSet<String>) -> bool {
    ownership_belongs_to_any_path(&diagnostic.ownership, paths)
        || evidence_belongs_to_any_path(&diagnostic.evidence, paths)
}

fn ownership_belongs_to_any_path(
    ownership: &[athanor_domain::Ownership],
    paths: &BTreeSet<String>,
) -> bool {
    ownership
        .iter()
        .any(|owner| paths.contains(&owner.source_file))
}

fn evidence_belongs_to_any_path(
    evidence: &[athanor_domain::Evidence],
    paths: &BTreeSet<String>,
) -> bool {
    evidence
        .iter()
        .any(|evidence| belongs_to_any_path(evidence.source_file.as_deref(), paths))
}

fn belongs_to_any_path(path: Option<&str>, paths: &BTreeSet<String>) -> bool {
    path.is_some_and(|path| paths.contains(path))
}

fn replace_payload_snapshot(value: &mut Value, snapshot: &SnapshotId) {
    if let Value::Object(object) = value
        && object.contains_key("snapshot")
    {
        object.insert("snapshot".to_string(), Value::String(snapshot.0.clone()));
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use athanor_core::{CanonicalSnapshot, CoreResult, ExtractOutput};
    use athanor_domain::{
        DiagnosticId, DiagnosticKind, DiagnosticStatus, EntityId, EntityKind, Evidence,
        EvidenceStatus, FactId, FactKind, Ownership, RelationId, RelationKind, RelationStatus,
        Severity, SourceLocation, StableKey,
    };
    use athanor_store_memory::MemoryKnowledgeStore;
    use serde_json::json;
    use std::sync::Mutex;

    use super::*;

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
        let pipeline = IndexPipeline::new(MemoryKnowledgeStore::new())
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
}
