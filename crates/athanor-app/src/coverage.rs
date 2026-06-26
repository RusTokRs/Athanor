use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use athanor_core::CanonicalSnapshot;
use athanor_domain::{Diagnostic, Entity, Fact};
use serde::Serialize;

use crate::config::load_config;
use crate::index_state::{IndexState, IndexStateStore};
use crate::project_path::normalize_canonical_path;
use crate::store::init_store;
use athanor_core::CanonicalSnapshotStore;

pub const COVERAGE_REPORT_SCHEMA: &str = "athanor.coverage.v1";

#[derive(Debug, Clone)]
pub struct CoverageOptions {
    pub root: PathBuf,
    pub adapter: Option<String>,
    pub file: Option<PathBuf>,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct CoverageReport {
    pub schema: &'static str,
    pub snapshot: String,
    pub root: PathBuf,
    pub filters: CoverageFilters,
    pub limits: CoverageLimits,
    pub totals: CoverageTotals,
    pub files: Vec<FileCoverage>,
    pub adapters: Vec<AdapterCoverage>,
    pub diagnostics: Vec<DiagnosticCoverage>,
    pub omitted: CoverageOmitted,
}

#[derive(Debug, Clone, Serialize)]
pub struct CoverageFilters {
    pub adapter: Option<String>,
    pub file: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CoverageLimits {
    pub limit: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct CoverageTotals {
    pub tracked_files: usize,
    pub files_with_canonical_objects: usize,
    pub files_with_open_diagnostics: usize,
    pub entities: usize,
    pub facts: usize,
    pub relations: usize,
    pub diagnostics: usize,
    pub open_diagnostics: usize,
    pub adapter_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileCoverage {
    pub path: String,
    pub language_hint: Option<String>,
    pub entities: usize,
    pub facts: usize,
    pub relations: usize,
    pub diagnostics: usize,
    pub open_diagnostics: usize,
    pub adapters: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdapterCoverage {
    pub adapter: String,
    pub files: usize,
    pub facts: usize,
    pub evidence_items: usize,
    pub diagnostics: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticCoverage {
    pub kind: String,
    pub total: usize,
    pub open: usize,
    pub files: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct CoverageOmitted {
    pub files: usize,
    pub adapters: usize,
    pub diagnostics: usize,
}

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

pub async fn coverage_project(options: CoverageOptions) -> Result<CoverageReport> {
    let root = normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );
    let config = load_config(&root)?;
    let store = init_store(&root, &config).await?;
    let snapshot = store
        .load_latest_snapshot()
        .await
        .context("failed to load latest canonical snapshot")?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "no canonical snapshot found; run `ath index {}` first",
                root.display()
            )
        })?;
    let state = IndexStateStore::new(root.join(".athanor/state/index-state.json"))
        .load()
        .context("failed to load index state")?;
    let file_filter = options
        .file
        .as_deref()
        .map(|path| relative_filter_path(&root, path))
        .transpose()?;

    Ok(build_coverage_report(
        root,
        snapshot,
        state,
        options.adapter,
        file_filter,
        options.limit,
    ))
}

fn build_coverage_report(
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
        let entry = adapters.entry(adapter.clone()).or_default();
        entry.facts += 1;
        for path in paths {
            if !path_matches(&path, &file_filter) {
                continue;
            }
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
            let adapter_entry = adapters.entry(adapter.to_string()).or_default();
            adapter_entry.evidence_items += 1;
            if let Some(path) = &evidence.source_file {
                if path_matches(path, &file_filter) {
                    adapter_entry.files.insert(path.clone());
                    let file = files.entry(path.clone()).or_default();
                    file.relations += 1;
                    file.adapters.insert(adapter.to_string());
                }
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
            if !path_matches(&path, &file_filter) {
                continue;
            }
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

fn relative_filter_path(root: &Path, path: &Path) -> Result<String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let normalized = normalize_canonical_path(absolute);
    let relative = normalized
        .strip_prefix(root)
        .with_context(|| format!("coverage file filter must stay under {}", root.display()))?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
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

#[cfg(test)]
mod tests {
    use athanor_domain::{
        DiagnosticId, DiagnosticKind, DiagnosticStatus, EntityId, EntityKind, Evidence,
        EvidenceStatus, FactId, FactKind, LanguageCode, Ownership, Severity, SnapshotId, StableKey,
    };
    use serde_json::json;

    use super::*;

    #[test]
    fn builds_bounded_file_and_adapter_coverage() {
        let snapshot = CanonicalSnapshot {
            snapshot: Some(SnapshotId("snap_coverage".to_string())),
            entities: vec![Entity {
                id: EntityId("ent_docs".to_string()),
                stable_key: StableKey("doc://docs/api.md".to_string()),
                kind: EntityKind::DocumentationPage,
                name: "api".to_string(),
                title: Some("API".to_string()),
                source: None,
                language: Some(LanguageCode("en".to_string())),
                aliases: Vec::new(),
                ownership: vec![Ownership {
                    source_file: "docs/api.md".to_string(),
                }],
                payload: json!({}),
            }],
            facts: vec![Fact {
                id: FactId("fact_docs".to_string()),
                kind: FactKind::DocSectionFound,
                subject: EntityId("ent_docs".to_string()),
                object: None,
                value: json!({}),
                evidence: vec![Evidence {
                    source_file: Some("docs/api.md".to_string()),
                    line_start: Some(1),
                    line_end: Some(1),
                    extractor: Some("MarkdownExtractor".to_string()),
                    commit_hash: None,
                    confidence: 1.0,
                    status: EvidenceStatus::Verified,
                }],
                ownership: vec![Ownership {
                    source_file: "docs/api.md".to_string(),
                }],
                snapshot: SnapshotId("snap_coverage".to_string()),
                extractor: "MarkdownExtractor".to_string(),
                confidence: 1.0,
            }],
            relations: Vec::new(),
            diagnostics: vec![Diagnostic {
                id: DiagnosticId("diag_docs".to_string()),
                kind: DiagnosticKind::DocumentationPageMissingTitle,
                severity: Severity::Low,
                status: DiagnosticStatus::Open,
                title: "Missing title".to_string(),
                message: "Missing title".to_string(),
                entities: vec![EntityId("ent_docs".to_string())],
                evidence: vec![Evidence {
                    source_file: Some("docs/api.md".to_string()),
                    line_start: Some(1),
                    line_end: Some(1),
                    extractor: Some("MarkdownStructureChecker".to_string()),
                    commit_hash: None,
                    confidence: 1.0,
                    status: EvidenceStatus::Verified,
                }],
                ownership: vec![Ownership {
                    source_file: "docs/api.md".to_string(),
                }],
                snapshot: SnapshotId("snap_coverage".to_string()),
                suggested_fix: None,
                payload: json!({}),
            }],
        };
        let state = IndexState {
            schema: crate::index_state::INDEX_STATE_SCHEMA.to_string(),
            snapshot: Some("snap_coverage".to_string()),
            files: BTreeMap::from([(
                "docs/api.md".to_string(),
                crate::index_state::FileState {
                    content_hash: Some("hash".to_string()),
                    language_hint: Some("markdown".to_string()),
                },
            )]),
        };

        let report = build_coverage_report(PathBuf::from("."), snapshot, state, None, None, 1);

        assert_eq!(report.schema, COVERAGE_REPORT_SCHEMA);
        assert_eq!(report.totals.tracked_files, 1);
        assert_eq!(report.totals.open_diagnostics, 1);
        assert_eq!(report.files[0].path, "docs/api.md");
        assert_eq!(report.adapters.len(), 1);
        assert_eq!(report.omitted.adapters, 1);
    }
}
