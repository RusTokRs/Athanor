use std::sync::Arc;

use anyhow::{Context, Result};
use athanor_core::{AffectedSubset, CheckInput, Checker, OperationContext};
use athanor_domain::{Diagnostic, Entity, Fact, Relation, SnapshotId};
use tracing::{Instrument, debug, debug_span, error};

use crate::pipeline::validate_diagnostics;
use crate::pipeline_metrics::{adapter_run, elapsed_ms};
use crate::pipeline_support::within_operation_deadline;
use crate::{AdapterRunMetrics, CancellationToken};

#[allow(clippy::too_many_arguments)]
pub(crate) async fn check(
    checkers: &[Box<dyn Checker>],
    snapshot: &SnapshotId,
    entities: Arc<Vec<Entity>>,
    facts: Arc<Vec<Fact>>,
    relations: Arc<Vec<Relation>>,
    affected: &AffectedSubset,
    operation: &OperationContext,
    cancellation: Option<CancellationToken>,
) -> Result<(Vec<Diagnostic>, Vec<AdapterRunMetrics>)> {
    let mut diagnostics = Vec::new();
    let mut metrics = Vec::new();
    for checker in checkers {
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
        let started = std::time::Instant::now();
        let output =
            crate::with_process_execution_context(operation.clone(), cancellation.clone(), async {
                within_operation_deadline(
                    operation,
                    checker_name,
                    checker.check_with_context(
                        CheckInput {
                            snapshot: snapshot.clone(),
                            entities: entities.clone(),
                            facts: facts.clone(),
                            relations: relations.clone(),
                            affected: affected.clone(),
                        },
                        operation,
                    ),
                )
                .await
            })
            .instrument(span)
            .await
            .inspect_err(|error| error!(checker = checker.name(), %error, "checker failed"))
            .with_context(|| format!("checker {} failed", checker_name))?;
        validate_diagnostics(checker_name, &output)?;
        let mut adapter_metrics =
            adapter_run("checker", checker_name, elapsed_ms(started.elapsed()));
        adapter_metrics.input_entities = entities.len();
        adapter_metrics.input_facts = facts.len();
        adapter_metrics.input_relations = relations.len();
        adapter_metrics.output_diagnostics = output.len();
        metrics.push(adapter_metrics);
        diagnostics.extend(output);
    }
    Ok((diagnostics, metrics))
}
