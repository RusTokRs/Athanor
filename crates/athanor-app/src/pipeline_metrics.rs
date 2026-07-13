use std::collections::BTreeMap;
use std::time::Duration;

use crate::AdapterRunMetrics;

pub(crate) fn adapter_run(
    phase: &'static str,
    adapter: impl Into<String>,
    duration_ms: u64,
) -> AdapterRunMetrics {
    AdapterRunMetrics {
        phase,
        adapter: adapter.into(),
        runs: 1,
        duration_ms,
        ..AdapterRunMetrics::default()
    }
}

pub(crate) fn elapsed_ms(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

pub(crate) fn aggregate_adapter_metrics(metrics: Vec<AdapterRunMetrics>) -> Vec<AdapterRunMetrics> {
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
