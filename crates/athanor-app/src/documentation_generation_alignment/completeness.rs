//! Deterministic documentation completeness reporting over one exact canonical snapshot.
//!
//! Slice 6A is intentionally pure: no Store access, filesystem reads, publication, transport,
//! daemon, MCP, network, or provider integration. The report uses the canonical baseline `file`
//! inventory plus canonical entity/evidence attribution to expose where future framework adapters
//! are still needed.

use std::collections::{BTreeMap, BTreeSet};

use athanor_core::CanonicalSnapshot;
use athanor_domain::{Diagnostic, EntityKind, Evidence, Fact, Relation};
use serde::Serialize;

use crate::documentation_evidence_location::{entity_evidence_locations, evidence_locations};

pub const DOCUMENTATION_COMPLETENESS_SCHEMA_V1: &str = "athanor.documentation_completeness.v1";
pub const DOCUMENTATION_COMPLETENESS_DEFAULT_LIMIT: usize = 50;
pub const DOCUMENTATION_COMPLETENESS_BASELINE_ADAPTER: &str = "file";
const UNKNOWN_LANGUAGE: &str = "unknown";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentationCompletenessRequest {
    pub snapshot: String,
    pub limit: usize,
}

impl DocumentationCompletenessRequest {
    pub fn new(snapshot: impl Into<String>, limit: usize) -> Self {
        Self {
            snapshot: snapshot.into(),
            limit,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.snapshot.trim().is_empty() || self.snapshot.trim() != self.snapshot {
            return Err(
                "documentation completeness snapshot must be non-empty and trimmed".to_string(),
            );
        }
        if self.limit == 0 {
            return Err("documentation completeness limit must be greater than zero".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DocumentationCompletenessReport {
    pub schema: &'static str,
    pub snapshot: String,
    pub baseline_adapter: &'static str,
    pub limit: usize,
    pub totals: DocumentationCompletenessTotals,
    pub languages: Vec<DocumentationLanguageCompleteness>,
    pub adapters: Vec<DocumentationAdapterCompleteness>,
    pub unprocessed_files: Vec<DocumentationUnprocessedFile>,
    pub omitted: DocumentationCompletenessOmitted,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct DocumentationCompletenessTotals {
    pub tracked_files: usize,
    pub processed_files: usize,
    pub unprocessed_files: usize,
    pub processed_ratio_basis_points: u16,
    pub languages: usize,
    pub adapters: usize,
    pub facts: usize,
    pub relations: usize,
    pub diagnostics: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DocumentationLanguageCompleteness {
    pub language: String,
    pub tracked_files: usize,
    pub processed_files: usize,
    pub unprocessed_files: usize,
    pub processed_ratio_basis_points: u16,
    pub adapters: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DocumentationAdapterCompleteness {
    pub adapter: String,
    pub files: usize,
    pub facts: usize,
    pub relations: usize,
    pub diagnostics: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DocumentationUnprocessedFile {
    pub path: String,
    pub language: String,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct DocumentationCompletenessOmitted {
    pub languages: usize,
    pub adapters: usize,
    pub unprocessed_files: usize,
}

#[derive(Debug, Clone, Default)]
struct LanguageAccumulator {
    tracked_files: usize,
    processed_files: usize,
    adapters: BTreeSet<String>,
}

#[derive(Debug, Clone, Default)]
struct AdapterAccumulator {
    files: BTreeSet<String>,
    facts: usize,
    relations: usize,
    diagnostics: usize,
}

pub fn build_documentation_completeness_report(
    request: &DocumentationCompletenessRequest,
    snapshot: &CanonicalSnapshot,
) -> Result<DocumentationCompletenessReport, String> {
    request.validate()?;
    let actual_snapshot = snapshot
        .snapshot
        .as_ref()
        .ok_or_else(|| "documentation completeness requires a canonical snapshot id".to_string())?;
    if actual_snapshot.0 != request.snapshot {
        return Err(format!(
            "documentation completeness snapshot mismatch: request {}, canonical {}",
            request.snapshot, actual_snapshot.0
        ));
    }

    let tracked_files = tracked_file_inventory(snapshot)?;
    if tracked_files.is_empty() {
        return Err(
            "documentation completeness requires baseline `file` entities in the canonical snapshot"
                .to_string(),
        );
    }

    let mut processed_paths = BTreeSet::<String>::new();
    for entity in &snapshot.entities {
        if entity.kind == EntityKind::File {
            continue;
        }
        processed_paths.extend(
            entity_evidence_locations(entity)
                .into_iter()
                .map(|location| location.path),
        );
    }

    let mut adapters = BTreeMap::<String, AdapterAccumulator>::new();
    let mut path_adapters = BTreeMap::<String, BTreeSet<String>>::new();

    for fact in &snapshot.facts {
        collect_fact(fact, &mut processed_paths, &mut adapters, &mut path_adapters);
    }
    for relation in &snapshot.relations {
        collect_relation(
            relation,
            &mut processed_paths,
            &mut adapters,
            &mut path_adapters,
        );
    }
    for diagnostic in &snapshot.diagnostics {
        collect_diagnostic(
            diagnostic,
            &mut processed_paths,
            &mut adapters,
            &mut path_adapters,
        );
    }

    let processed_paths = processed_paths
        .into_iter()
        .filter(|path| tracked_files.contains_key(path))
        .collect::<BTreeSet<_>>();

    let mut languages = BTreeMap::<String, LanguageAccumulator>::new();
    let mut unprocessed_files = Vec::<DocumentationUnprocessedFile>::new();
    for (path, language) in &tracked_files {
        let entry = languages.entry(language.clone()).or_default();
        entry.tracked_files += 1;
        if processed_paths.contains(path) {
            entry.processed_files += 1;
        } else {
            unprocessed_files.push(DocumentationUnprocessedFile {
                path: path.clone(),
                language: language.clone(),
            });
        }
        if let Some(names) = path_adapters.get(path) {
            entry.adapters.extend(names.iter().cloned());
        }
    }

    let tracked_count = tracked_files.len();
    let processed_count = processed_paths.len();
    let unprocessed_count = tracked_count.saturating_sub(processed_count);

    let mut language_rows = languages
        .into_iter()
        .map(|(language, accumulator)| DocumentationLanguageCompleteness {
            language,
            tracked_files: accumulator.tracked_files,
            processed_files: accumulator.processed_files,
            unprocessed_files: accumulator
                .tracked_files
                .saturating_sub(accumulator.processed_files),
            processed_ratio_basis_points: ratio_basis_points(
                accumulator.processed_files,
                accumulator.tracked_files,
            ),
            adapters: accumulator.adapters.len(),
        })
        .collect::<Vec<_>>();
    language_rows.sort_by(|left, right| {
        right
            .unprocessed_files
            .cmp(&left.unprocessed_files)
            .then_with(|| left.language.cmp(&right.language))
    });

    let mut adapter_rows = adapters
        .into_iter()
        .map(|(adapter, accumulator)| DocumentationAdapterCompleteness {
            adapter,
            files: accumulator.files.len(),
            facts: accumulator.facts,
            relations: accumulator.relations,
            diagnostics: accumulator.diagnostics,
        })
        .collect::<Vec<_>>();
    adapter_rows.sort_by(|left, right| {
        right
            .files
            .cmp(&left.files)
            .then_with(|| {
                let left_objects = left.facts + left.relations + left.diagnostics;
                let right_objects = right.facts + right.relations + right.diagnostics;
                right_objects.cmp(&left_objects)
            })
            .then_with(|| left.adapter.cmp(&right.adapter))
    });

    unprocessed_files.sort_by(|left, right| {
        left.language
            .cmp(&right.language)
            .then_with(|| left.path.cmp(&right.path))
    });

    let omitted = DocumentationCompletenessOmitted {
        languages: language_rows.len().saturating_sub(request.limit),
        adapters: adapter_rows.len().saturating_sub(request.limit),
        unprocessed_files: unprocessed_files.len().saturating_sub(request.limit),
    };
    let totals = DocumentationCompletenessTotals {
        tracked_files: tracked_count,
        processed_files: processed_count,
        unprocessed_files: unprocessed_count,
        processed_ratio_basis_points: ratio_basis_points(processed_count, tracked_count),
        languages: language_rows.len(),
        adapters: adapter_rows.len(),
        facts: snapshot.facts.len(),
        relations: snapshot.relations.len(),
        diagnostics: snapshot.diagnostics.len(),
    };

    language_rows.truncate(request.limit);
    adapter_rows.truncate(request.limit);
    unprocessed_files.truncate(request.limit);

    Ok(DocumentationCompletenessReport {
        schema: DOCUMENTATION_COMPLETENESS_SCHEMA_V1,
        snapshot: request.snapshot.clone(),
        baseline_adapter: DOCUMENTATION_COMPLETENESS_BASELINE_ADAPTER,
        limit: request.limit,
        totals,
        languages: language_rows,
        adapters: adapter_rows,
        unprocessed_files,
        omitted,
    })
}

fn tracked_file_inventory(snapshot: &CanonicalSnapshot) -> Result<BTreeMap<String, String>, String> {
    let mut files = BTreeMap::<String, String>::new();
    for entity in &snapshot.entities {
        if entity.kind != EntityKind::File {
            continue;
        }
        let language = entity
            .language
            .as_ref()
            .map_or_else(|| UNKNOWN_LANGUAGE.to_string(), |language| language.0.clone());
        let paths = entity_evidence_locations(entity)
            .into_iter()
            .map(|location| location.path)
            .collect::<BTreeSet<_>>();
        for path in paths {
            if let Some(existing) = files.get(&path) {
                if existing != &language {
                    return Err(format!(
                        "baseline file {path} has conflicting language hints `{existing}` and `{language}`"
                    ));
                }
            } else {
                files.insert(path, language.clone());
            }
        }
    }
    Ok(files)
}

fn collect_fact(
    fact: &Fact,
    processed_paths: &mut BTreeSet<String>,
    adapters: &mut BTreeMap<String, AdapterAccumulator>,
    path_adapters: &mut BTreeMap<String, BTreeSet<String>>,
) {
    if fact.extractor == DOCUMENTATION_COMPLETENESS_BASELINE_ADAPTER {
        return;
    }
    let locations = evidence_locations(
        &fact.evidence,
        fact.ownership.iter().map(|owner| &owner.source_file),
    );
    let paths = locations
        .into_iter()
        .map(|location| location.path)
        .collect::<BTreeSet<_>>();
    processed_paths.extend(paths.iter().cloned());
    let entry = adapters.entry(fact.extractor.clone()).or_default();
    entry.facts += 1;
    entry.files.extend(paths.iter().cloned());
    for path in paths {
        path_adapters
            .entry(path)
            .or_default()
            .insert(fact.extractor.clone());
    }
}

fn collect_relation(
    relation: &Relation,
    processed_paths: &mut BTreeSet<String>,
    adapters: &mut BTreeMap<String, AdapterAccumulator>,
    path_adapters: &mut BTreeMap<String, BTreeSet<String>>,
) {
    collect_non_baseline_evidence_paths(&relation.evidence, processed_paths);
    for adapter in named_content_adapters(&relation.evidence) {
        let paths = adapter_evidence_paths(&relation.evidence, &adapter);
        let entry = adapters.entry(adapter.clone()).or_default();
        entry.relations += 1;
        entry.files.extend(paths.iter().cloned());
        for path in paths {
            path_adapters.entry(path).or_default().insert(adapter.clone());
        }
    }
}

fn collect_diagnostic(
    diagnostic: &Diagnostic,
    processed_paths: &mut BTreeSet<String>,
    adapters: &mut BTreeMap<String, AdapterAccumulator>,
    path_adapters: &mut BTreeMap<String, BTreeSet<String>>,
) {
    collect_non_baseline_evidence_paths(&diagnostic.evidence, processed_paths);
    for adapter in named_content_adapters(&diagnostic.evidence) {
        let paths = adapter_evidence_paths(&diagnostic.evidence, &adapter);
        let entry = adapters.entry(adapter.clone()).or_default();
        entry.diagnostics += 1;
        entry.files.extend(paths.iter().cloned());
        for path in paths {
            path_adapters.entry(path).or_default().insert(adapter.clone());
        }
    }
}

fn collect_non_baseline_evidence_paths(
    evidence: &[Evidence],
    processed_paths: &mut BTreeSet<String>,
) {
    for item in evidence {
        if item
            .extractor
            .as_deref()
            .is_some_and(|adapter| adapter == DOCUMENTATION_COMPLETENESS_BASELINE_ADAPTER)
        {
            continue;
        }
        processed_paths.extend(
            evidence_locations(std::slice::from_ref(item), std::iter::empty::<&String>())
                .into_iter()
                .map(|location| location.path),
        );
    }
}

fn named_content_adapters(evidence: &[Evidence]) -> BTreeSet<String> {
    evidence
        .iter()
        .filter_map(|item| item.extractor.as_ref())
        .filter(|adapter| adapter.as_str() != DOCUMENTATION_COMPLETENESS_BASELINE_ADAPTER)
        .cloned()
        .collect()
}

fn adapter_evidence_paths(evidence: &[Evidence], adapter: &str) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    for item in evidence
        .iter()
        .filter(|item| item.extractor.as_deref() == Some(adapter))
    {
        paths.extend(
            evidence_locations(std::slice::from_ref(item), std::iter::empty::<&String>())
                .into_iter()
                .map(|location| location.path),
        );
    }
    paths
}

fn ratio_basis_points(part: usize, whole: usize) -> u16 {
    if whole == 0 {
        return 0;
    }
    let basis_points = part.saturating_mul(10_000) / whole;
    basis_points.min(10_000) as u16
}
