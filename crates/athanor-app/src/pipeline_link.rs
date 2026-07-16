use std::sync::Arc;

use anyhow::{Context, Result};
use athanor_core::{AffectedSubset, LinkInput, Linker, OperationContext};
use athanor_domain::{Entity, Fact, Relation, SnapshotId};
use tracing::{Instrument, debug, debug_span, error};

use crate::pipeline::validate_relations;
use crate::pipeline_metrics::{adapter_run, elapsed_ms};
use crate::pipeline_support::within_operation_deadline;
use crate::{AdapterRunMetrics, CancellationToken};

pub(crate) async fn link(
    linkers: &[Box<dyn Linker>],
    snapshot: &SnapshotId,
    entities: Arc<Vec<Entity>>,
    facts: Arc<Vec<Fact>>,
    affected: &AffectedSubset,
    operation: &OperationContext,
    cancellation: Option<CancellationToken>,
) -> Result<(Vec<Relation>, Vec<AdapterRunMetrics>)> {
    let mut relations = Vec::new();
    let mut metrics = Vec::new();
    for linker in linkers {
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
        let started = std::time::Instant::now();
        let output = crate::with_process_execution_context(
            operation.clone(),
            cancellation.clone(),
            async {
                within_operation_deadline(
                    operation,
                    linker_name,
                    linker.link_with_context(
                        LinkInput {
                            snapshot: snapshot.clone(),
                            entities: entities.clone(),
                            facts: facts.clone(),
                            affected: affected.clone(),
                        },
                        operation,
                    ),
                )
                .await
            },
        )
        .instrument(span)
        .await
        .inspect_err(|error| error!(linker = linker.name(), %error, "linker failed"))
        .with_context(|| format!("linker {} failed", linker_name))?;
        validate_relations(linker_name, &output)?;
        let mut adapter_metrics = adapter_run("linker", linker_name, elapsed_ms(started.elapsed()));
        adapter_metrics.input_entities = entities.len();
        adapter_metrics.input_facts = facts.len();
        adapter_metrics.output_relations = output.len();
        metrics.push(adapter_metrics);
        relations.extend(output);
    }
    Ok((relations, metrics))
}
