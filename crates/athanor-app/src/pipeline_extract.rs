use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::{Context, Result};
use athanor_core::{ExtractInput, Extractor, OperationContext, SourceFile};
use athanor_domain::{Diagnostic, Entity, Fact, RepoId, SnapshotId};
use futures::stream::{self, StreamExt};
use tokio::sync::Semaphore;
use tracing::{Instrument, debug, debug_span, error, info};

use crate::pipeline::{validate_diagnostics, validate_entities, validate_facts};
use crate::pipeline_merge::{canonicalize_diagnostics, canonicalize_entities, canonicalize_facts};
use crate::pipeline_metrics::{adapter_run, elapsed_ms};
use crate::pipeline_support::within_operation_deadline;
use crate::{AdapterRunMetrics, CancellationToken};

#[allow(clippy::too_many_arguments)]
pub(crate) async fn extract(
    extractors: &[Box<dyn Extractor>],
    repo: &RepoId,
    snapshot: &SnapshotId,
    files: &[SourceFile],
    concurrency: usize,
    max_bytes_in_flight: usize,
    concurrency_by_adapter: &BTreeMap<String, usize>,
    operation: &OperationContext,
    cancellation: Option<CancellationToken>,
) -> Result<(
    Vec<Entity>,
    Vec<Fact>,
    Vec<Diagnostic>,
    Vec<AdapterRunMetrics>,
)> {
    let tasks = files
        .iter()
        .flat_map(|source| {
            extractors
                .iter()
                .filter(move |extractor| extractor.supports(source))
                .map(move |extractor| (extractor.as_ref(), source.clone()))
        })
        .collect::<Vec<_>>();
    let byte_budget = Arc::new(Semaphore::new(max_bytes_in_flight.min(u32::MAX as usize)));
    let adapter_budgets = Arc::new(
        extractors
            .iter()
            .filter_map(|extractor| {
                concurrency_by_adapter.get(extractor.name()).map(|limit| {
                    (
                        extractor.name().to_string(),
                        Arc::new(Semaphore::new(*limit)),
                    )
                })
            })
            .collect::<BTreeMap<_, _>>(),
    );
    info!(
        task_count = tasks.len(),
        concurrency, max_bytes_in_flight, "queued extraction tasks"
    );
    let mut outputs = stream::iter(tasks)
        .map(|(extractor, source)| {
            let byte_budget = Arc::clone(&byte_budget);
            let adapter_budgets = Arc::clone(&adapter_budgets);
            let cancellation = cancellation.clone();
            let operation = operation.clone();
            async move {
                let source_bytes = source.content.as_ref().map_or(0, String::len).max(1);
                let permits = source_bytes.min(max_bytes_in_flight).min(u32::MAX as usize) as u32;
                let _byte_permit = byte_budget
                    .acquire_many_owned(permits)
                    .await
                    .map_err(|_| anyhow::anyhow!("extraction byte budget was closed"))?;
                let extractor_name = extractor.name();
                let _adapter_permit = match adapter_budgets.get(extractor_name) {
                    Some(budget) => Some(
                        Arc::clone(budget)
                            .acquire_owned()
                            .await
                            .map_err(|_| anyhow::anyhow!("adapter extraction budget was closed"))?,
                    ),
                    None => None,
                };
                let started = std::time::Instant::now();
                let span =
                    debug_span!("extract_source", extractor = extractor_name, file = %source.path);
                let output = crate::runtime::with_process_execution_context(
                    operation.clone(),
                    cancellation,
                    async {
                        within_operation_deadline(
                            &operation,
                            extractor_name,
                            extractor.extract_with_context(
                                ExtractInput {
                                    repo: repo.clone(),
                                    snapshot: snapshot.clone(),
                                    source,
                                },
                                &operation,
                            ),
                        )
                        .await
                        .with_context(|| format!("extractor {} failed", extractor_name))
                    },
                )
                .instrument(span)
                .await?;
                validate_entities(extractor_name, &output.entities)?;
                validate_facts(extractor_name, &output.facts)?;
                validate_diagnostics(extractor_name, &output.diagnostics)?;
                let mut metrics =
                    adapter_run("extractor", extractor_name, elapsed_ms(started.elapsed()));
                metrics.input_files = 1;
                metrics.output_entities = output.entities.len();
                metrics.output_facts = output.facts.len();
                metrics.output_diagnostics = output.diagnostics.len();
                debug!(
                    extractor = extractor_name,
                    entities = output.entities.len(),
                    facts = output.facts.len(),
                    diagnostics = output.diagnostics.len(),
                    "extractor emitted canonical objects"
                );
                Ok::<_, anyhow::Error>((output, metrics))
            }
        })
        .buffer_unordered(concurrency);
    let mut entities = Vec::new();
    let mut facts = Vec::new();
    let mut diagnostics = Vec::new();
    let mut metrics = Vec::new();
    while let Some(output) = outputs.next().await {
        match output {
            Ok((output, metric)) => {
                entities.extend(output.entities);
                facts.extend(output.facts);
                diagnostics.extend(output.diagnostics);
                metrics.push(metric);
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
        canonicalize_diagnostics(diagnostics),
        metrics,
    ))
}
