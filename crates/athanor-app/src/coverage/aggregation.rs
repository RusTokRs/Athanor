use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use athanor_core::CanonicalSnapshot;
use athanor_domain::{Diagnostic, Entity, Fact};

use crate::index_state::IndexState;

use super::model::{
    AdapterCoverage, COVERAGE_REPORT_SCHEMA, CoverageFilters, CoverageLimits, CoverageOmitted,
    CoverageReport, CoverageTotals, DiagnosticCoverage, FileCoverage,
};

#[derive(Debug, Clone, Default)]
struct FileAccumulator {
    language_hint: Option<String>,
    entities: usize,
    facts: usize,
    relations: usize,
    diagnostics: usize,
    open_diagnostics: usize,
    adapters: BTreeSet<String>,
}

#[derive(Debug, Clone, Default)]
struct AdapterAccumulator {
    files: BTreeSet<String>,
    facts: usize,
    evidence_items: usize,
    diagnostics: usize,
}

#[derive(Debug, Clone, Default)]
struct DiagnosticAccumulator {
    total: usize,
    open: usize,
    files: BTreeSet<String>,
}

pub(super) fn build_coverage_report(
    root: PathBuf,
    snapshot: CanonicalSnapshot,
    state: IndexState,
    adapter_filter: Option<String>,
    file_filter: Option<String>,
    limit: usize,
) -> CoverageReport {
    let snapshot_id = snapshot
        .snapshot
        .as_ref()
        .map_or_else(|| "unknown".to_string(), |snapshot| snapshot.0.clone());
    let mut files = state
        .files
        .iter()
        .filter(|(path, _)| file_filter.as_ref().is_none_or(|filter| filter == *path))
        .map(|(path, file)| {
            (
                path.clone(),
                FileAccumulator {
                    language_hint: file.language_hint.clone(),
                    ..FileAccumulator::default()
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut adapters = BTreeMap::<String, AdapterAccumulator>::new();
    let mut diagnostics = BTreeMap::<String, DiagnosticAccumulator>::new();

    for entity in &snapshot.entities {
        for path in entity_paths(entity) {
            if !path_matches(&path, &file_filter) {
                continue;
            }
            files.entry(path).or_default().entities += 1;
        }
    }

    for fact in &snapshot.facts {
        let paths = fact_paths(fact);
        let adapter = fact.extractor.clone();
        if !adapter_matches(&adapter, &adapter_filter) {
            continue;
        }
        let matched_paths = paths
            .into_iter()
            .filter(|path| path_matches(path, &file_filter))
            .collect::<Vec<_>>();
        if matched_paths.is_empty() {
            continue;
        }
        let entry = adapters.entry(adapter.clone()).or_default();
        entry.facts += 1;
        for path in matched_paths {
            entry.files.insert(path.clone());
            let file = files.entry(path).or_default();
            file.facts += 1;
            file.adapters.insert(adapter.clone());
        }
    }

    for relation in &snapshot.relations {
        for evidence in &relation.evidence {
            let Some(adapter) = evidence.extractor.as_deref() else {
                continue;
            };
            if !adapter_matches(adapter, &adapter_filter) {
                continue;
            }
            if let Some(path) = &evidence.source_file
                && path_matches(path, &file_filter)
            {
                let adapter_entry = adapters.entry(adapter.to_string()).or_default();
                adapter_entry.evidence_items += 1;
                adapter_entry.files.insert(path.clone());
                let file = files.entry(path.clone()).or_default();
                file.relations += 1;
                file.adapters.insert(adapter.to_string());
            }
        }
    }

    for diagnostic in &snapshot.diagnostics {
        let paths = diagnostic_paths(diagnostic);
        let adapter_names = diagnostic
            .evidence
            .iter()
            .filter_map(|evidence| evidence.extractor.clone())
            .collect::<BTreeSet<_>>();
        if adapter_filter
            .as_ref()
            .is_some_and(|adapter| !adapter_names.contains(adapter))
        {
            continue;
        }
        let paths = paths
            .into_iter()
            .filter(|path| path_matches(path, &file_filter))
            .collect::<Vec<_>>();
        if paths.is_empty() && file_filter.is_some() {
            continue;
        }
        let kind = diagnostic_kind_name(diagnostic);
        let diagnostic_entry = diagnostics.entry(kind).or_default();
        diagnostic_entry.total += 1;
        if diagnostic.status == athanor_domain::DiagnosticStatus::Open {
            diagnostic_entry.open += 1;
        }
        for adapter in &adapter_names {
            adapters.entry(adapter.clone()).or_default().diagnostics += 1;
        }
        for path in paths {
            diagnostic_entry.files.insert(path.clone());
            for adapter in &adapter_names {
                adapters
                    .entry(adapter.clone())
                    .or_default()
                    .files
                    .insert(path.clone());
            }
            let file = files.entry(path).or_default();
            file.diagnostics += 1;
            if diagnostic.status == athanor_domain::DiagnosticStatus::Open {
                file.open_diagnostics += 1;
            }
            file.adapters.extend(adapter_names.iter().cloned());
        }
    }

    let mut file_rows = files
        .into_iter()
        .filter(|(_, file)| has_file_material(file))
        .map(|(path, file)| FileCoverage {
            path,
            language_hint: file.language_hint,
            entities: file.entities,
            facts: file.facts,
            relations: file.relations,
            diagnostics: file.diagnostics,
            open_diagnostics: file.open_diagnostics,
            adapters: file.adapters.into_iter().collect(),
        })
        .collect::<Vec<_>>();
    file_rows.sort_by_key(|file| {
        (
            std::cmp::Reverse(file.open_diagnostics),
            std::cmp::Reverse(file.entities + file.facts + file.relations + file.diagnostics),
            file.path.clone(),
        )
    });

    let mut adapter_rows = adapters
        .into_iter()
        .map(|(adapter, coverage)| AdapterCoverage {
            adapter,
            files: coverage.files.len(),
            facts: coverage.facts,
            evidence_items: coverage.evidence_items,
            diagnostics: coverage.diagnostics,
        })
        .collect::<Vec<_>>();
    adapter_rows.sort_by_key(|adapter| {
        (
            std::cmp::Reverse(adapter.files),
            std::cmp::Reverse(adapter.facts + adapter.evidence_items + adapter.diagnostics),
            adapter.adapter.clone(),
        )
    });

    let mut diagnostic_rows = diagnostics
        .into_iter()
        .map(|(kind, coverage)| DiagnosticCoverage {
            kind,
            total: coverage.total,
            open: coverage.open,
            files: coverage.files.len(),
        })
        .collect::<Vec<_>>();
    diagnostic_rows.sort_by_key(|diagnostic| {
        (
            std::cmp::Reverse(diagnostic.open),
            std::cmp::Reverse(diagnostic.total),
            diagnostic.kind.clone(),
        )
    });

    let totals = CoverageTotals {
        tracked_files: state.files.len(),
        files_with_canonical_objects: file_rows.len(),
        files_with_open_diagnostics: file_rows
            .iter()
            .filter(|file| file.open_diagnostics > 0)
            .count(),
        entities: file_rows.iter().map(|file| file.entities).sum(),
        facts: file_rows.iter().map(|file| file.facts).sum(),
        relations: file_rows.iter().map(|file| file.relations).sum(),
        diagnostics: file_rows.iter().map(|file| file.diagnostics).sum(),
        open_diagnostics: file_rows.iter().map(|file| file.open_diagnostics).sum(),
        adapter_count: adapter_rows.len(),
    };

    let effective_limit = limit.max(1);
    let omitted = CoverageOmitted {
        files: file_rows.len().saturating_sub(effective_limit),
        adapters: adapter_rows.len().saturating_sub(effective_limit),
        diagnostics: diagnostic_rows.len().saturating_sub(effective_limit),
    };
    file_rows.truncate(effective_limit);
    adapter_rows.truncate(effective_limit);
    diagnostic_rows.truncate(effective_limit);

    CoverageReport {
        schema: COVERAGE_REPORT_SCHEMA,
        snapshot: snapshot_id,
        root,
        filters: CoverageFilters {
            adapter: adapter_filter,
            file: file_filter,
        },
        limits: CoverageLimits {
            limit: effective_limit,
        },
        totals,
        files: file_rows,
        adapters: adapter_rows,
        diagnostics: diagnostic_rows,
        omitted,
    }
}

fn has_file_material(file: &FileAccumulator) -> bool {
    file.entities > 0
        || file.facts > 0
        || file.relations > 0
        || file.diagnostics > 0
        || file.open_diagnostics > 0
}

fn path_matches(path: &str, filter: &Option<String>) -> bool {
    filter.as_ref().is_none_or(|filter| filter == path)
}

fn adapter_matches(adapter: &str, filter: &Option<String>) -> bool {
    filter.as_ref().is_none_or(|filter| filter == adapter)
}

fn entity_paths(entity: &Entity) -> BTreeSet<String> {
    let mut paths = entity
        .ownership
        .iter()
        .map(|ownership| ownership.source_file.clone())
        .collect::<BTreeSet<_>>();
    if let Some(source) = &entity.source {
        paths.insert(source.path.clone());
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

fn diagnostic_kind_name(diagnostic: &Diagnostic) -> String {
    serde_json::to_value(&diagnostic.kind)
        .ok()
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .unwrap_or_else(|| format!("{:?}", diagnostic.kind))
}
