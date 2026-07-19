use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use athanor_core::CanonicalSnapshot;
use athanor_domain::{Diagnostic, Fact};

use crate::index_state::IndexState;

use super::model::{
    AdapterCapability, BASELINE_ADAPTER, CAPABILITIES_REPORT_SCHEMA, CapabilitiesLimits,
    CapabilitiesOmitted, CapabilitiesReport, CapabilitiesTotals, LanguageCapability,
    LowConfidenceFact, UNKNOWN_LANGUAGE, UnprocessedFile,
};

#[derive(Debug, Clone, Default)]
struct LanguageAccumulator {
    tracked_files: usize,
    processed_files: usize,
}

#[derive(Debug, Clone)]
struct AdapterAccumulator {
    processed_files: BTreeSet<String>,
    facts: usize,
    low_confidence_facts: usize,
    min_confidence: f32,
}

impl Default for AdapterAccumulator {
    fn default() -> Self {
        Self {
            processed_files: BTreeSet::new(),
            facts: 0,
            low_confidence_facts: 0,
            min_confidence: 1.0,
        }
    }
}

pub(super) fn build_capabilities_report(
    root: PathBuf,
    snapshot: CanonicalSnapshot,
    state: IndexState,
    limit: usize,
    confidence_threshold: f32,
) -> CapabilitiesReport {
    let snapshot_id = snapshot
        .snapshot
        .as_ref()
        .map_or_else(|| "unknown".to_string(), |snapshot| snapshot.0.clone());

    let processed_paths = content_processed_paths(&snapshot);

    let mut languages = BTreeMap::<String, LanguageAccumulator>::new();
    let mut unprocessed = Vec::<UnprocessedFile>::new();
    for (path, file) in &state.files {
        let language = file
            .language_hint
            .clone()
            .unwrap_or_else(|| UNKNOWN_LANGUAGE.to_string());
        let accumulator = languages.entry(language.clone()).or_default();
        accumulator.tracked_files += 1;
        if processed_paths.contains(path) {
            accumulator.processed_files += 1;
        } else {
            unprocessed.push(UnprocessedFile {
                path: path.clone(),
                language_hint: language,
            });
        }
    }

    let mut adapters = BTreeMap::<String, AdapterAccumulator>::new();
    let mut low_confidence = Vec::<LowConfidenceFact>::new();
    let mut total_facts = 0usize;
    let mut total_low_confidence = 0usize;
    for fact in &snapshot.facts {
        total_facts += 1;
        let adapter = fact.extractor.clone();
        let entry = adapters.entry(adapter.clone()).or_default();
        entry.facts += 1;
        entry.min_confidence = entry.min_confidence.min(fact.confidence);
        for path in fact_paths(fact) {
            entry.processed_files.insert(path);
        }
        if fact.confidence < confidence_threshold {
            total_low_confidence += 1;
            entry.low_confidence_facts += 1;
            let evidence = fact.evidence.first();
            low_confidence.push(LowConfidenceFact {
                fact_id: fact.id.0.clone(),
                adapter,
                kind: fact_kind_name(fact),
                confidence: fact.confidence,
                path: evidence.and_then(|evidence| evidence.source_file.clone()),
                line_start: evidence.and_then(|evidence| evidence.line_start),
            });
        }
    }

    let tracked_files = state.files.len();
    let unprocessed_total = unprocessed.len();
    let processed_files = tracked_files.saturating_sub(unprocessed_total);

    let mut language_rows = languages
        .into_iter()
        .map(|(language_hint, accumulator)| {
            let unprocessed_files = accumulator
                .tracked_files
                .saturating_sub(accumulator.processed_files);
            LanguageCapability {
                language_hint,
                tracked_files: accumulator.tracked_files,
                processed_files: accumulator.processed_files,
                unprocessed_files,
                processed_ratio_percent: ratio_percent(
                    accumulator.processed_files,
                    accumulator.tracked_files,
                ),
            }
        })
        .collect::<Vec<_>>();
    language_rows.sort_by(|left, right| {
        right
            .unprocessed_files
            .cmp(&left.unprocessed_files)
            .then_with(|| right.tracked_files.cmp(&left.tracked_files))
            .then_with(|| left.language_hint.cmp(&right.language_hint))
    });

    let mut adapter_rows = adapters
        .into_iter()
        .map(|(adapter, accumulator)| AdapterCapability {
            adapter,
            processed_files: accumulator.processed_files.len(),
            facts: accumulator.facts,
            low_confidence_facts: accumulator.low_confidence_facts,
            min_confidence: accumulator.min_confidence,
        })
        .collect::<Vec<_>>();
    adapter_rows.sort_by(|left, right| {
        right
            .low_confidence_facts
            .cmp(&left.low_confidence_facts)
            .then_with(|| right.processed_files.cmp(&left.processed_files))
            .then_with(|| right.facts.cmp(&left.facts))
            .then_with(|| left.adapter.cmp(&right.adapter))
    });

    low_confidence.sort_by(|left, right| {
        left.confidence
            .total_cmp(&right.confidence)
            .then_with(|| left.adapter.cmp(&right.adapter))
            .then_with(|| left.fact_id.cmp(&right.fact_id))
    });

    unprocessed.sort_by(|left, right| {
        left.language_hint
            .cmp(&right.language_hint)
            .then_with(|| left.path.cmp(&right.path))
    });

    let totals = CapabilitiesTotals {
        tracked_files,
        processed_files,
        unprocessed_files: unprocessed_total,
        processed_ratio_percent: ratio_percent(processed_files, tracked_files),
        languages: language_rows.len(),
        adapters: adapter_rows.len(),
        facts: total_facts,
        low_confidence_facts: total_low_confidence,
    };

    let effective_limit = limit.max(1);
    let omitted = CapabilitiesOmitted {
        languages: language_rows.len().saturating_sub(effective_limit),
        adapters: adapter_rows.len().saturating_sub(effective_limit),
        low_confidence_facts: low_confidence.len().saturating_sub(effective_limit),
        unprocessed_files: unprocessed.len().saturating_sub(effective_limit),
    };
    language_rows.truncate(effective_limit);
    adapter_rows.truncate(effective_limit);
    low_confidence.truncate(effective_limit);
    unprocessed.truncate(effective_limit);

    CapabilitiesReport {
        schema: CAPABILITIES_REPORT_SCHEMA,
        snapshot: snapshot_id,
        root,
        baseline_adapter: BASELINE_ADAPTER,
        limits: CapabilitiesLimits {
            limit: effective_limit,
            confidence_threshold,
        },
        totals,
        languages: language_rows,
        adapters: adapter_rows,
        low_confidence_facts: low_confidence,
        unprocessed_files: unprocessed,
        omitted,
    }
}

fn ratio_percent(part: usize, whole: usize) -> u8 {
    if whole == 0 {
        return 0;
    }
    let percent = (part * 100) / whole;
    percent.min(100) as u8
}

/// Paths that received content extraction beyond the baseline file inventory.
///
/// A path counts as processed when at least one canonical object attributed to an
/// adapter other than [`BASELINE_ADAPTER`] references it. The baseline adapter is
/// excluded because it emits one inventory fact for every discovered file, which
/// would otherwise mark every tracked file processed and hide real gaps.
fn content_processed_paths(snapshot: &CanonicalSnapshot) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    for fact in &snapshot.facts {
        if fact.extractor == BASELINE_ADAPTER {
            continue;
        }
        paths.extend(fact_paths(fact));
    }
    for relation in &snapshot.relations {
        for evidence in &relation.evidence {
            let is_baseline = evidence
                .extractor
                .as_deref()
                .is_some_and(|extractor| extractor == BASELINE_ADAPTER);
            if is_baseline {
                continue;
            }
            if let Some(path) = &evidence.source_file {
                paths.insert(path.clone());
            }
        }
    }
    for diagnostic in &snapshot.diagnostics {
        let has_content_adapter = diagnostic.evidence.iter().any(|evidence| {
            evidence
                .extractor
                .as_deref()
                .is_none_or(|extractor| extractor != BASELINE_ADAPTER)
        });
        if has_content_adapter {
            paths.extend(diagnostic_paths(diagnostic));
        }
    }
    paths
}

fn fact_paths(fact: &Fact) -> BTreeSet<String> {
    let mut paths = fact
        .ownership
        .iter()
        .map(|ownership| ownership.source_file.clone())
        .collect::<BTreeSet<_>>();
    for evidence in &fact.evidence {
        if let Some(path) = &evidence.source_file {
            paths.insert(path.clone());
        }
    }
    paths
}

fn diagnostic_paths(diagnostic: &Diagnostic) -> BTreeSet<String> {
    let mut paths = diagnostic
        .ownership
        .iter()
        .map(|ownership| ownership.source_file.clone())
        .collect::<BTreeSet<_>>();
    for evidence in &diagnostic.evidence {
        if let Some(path) = &evidence.source_file {
            paths.insert(path.clone());
        }
    }
    paths
}

fn fact_kind_name(fact: &Fact) -> String {
    serde_json::to_value(&fact.kind)
        .ok()
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .unwrap_or_else(|| format!("{:?}", fact.kind))
}
