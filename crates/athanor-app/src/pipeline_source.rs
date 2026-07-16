use anyhow::{Context, Result};
use athanor_core::{OperationContext, SourceFile, SourceProvider};
use tracing::{Instrument, debug, debug_span};

use crate::pipeline_metrics::{adapter_run, elapsed_ms};
use crate::{AdapterRunMetrics, CancellationToken};

pub(crate) async fn discover(
    sources: &[Box<dyn SourceProvider>],
    operation: &OperationContext,
    cancellation: Option<CancellationToken>,
) -> Result<(Vec<SourceFile>, Vec<AdapterRunMetrics>)> {
    let mut files = Vec::new();
    let mut metrics = Vec::new();
    for source in sources {
        let source_name = source.name();
        let span = debug_span!("discover_source", source = source_name);
        let started = std::time::Instant::now();
        let discovered = crate::with_process_execution_context(
            operation.clone(),
            cancellation.clone(),
            async {
                crate::pipeline_support::within_operation_deadline(
                    operation,
                    source_name,
                    source.discover_with_context(operation),
                )
                .await
            },
        )
        .instrument(span)
        .await
        .with_context(|| format!("source {} failed", source_name))?;
        let mut adapter_metrics = adapter_run("source", source_name, elapsed_ms(started.elapsed()));
        adapter_metrics.output_files = discovered.len();
        debug!(
            source = source_name,
            file_count = discovered.len(),
            "source discovery produced files"
        );
        metrics.push(adapter_metrics);
        files.extend(discovered);
    }
    Ok((files, metrics))
}
