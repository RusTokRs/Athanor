use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use anyhow::{Context, Result};
use athanor_core::CanonicalSnapshot;
use athanor_domain::{Diagnostic, Fact};
use serde::Serialize;

use crate::config::load_config;
use crate::index_state::{IndexState, IndexStateStore};
use crate::project_path::normalize_canonical_path;
use crate::store::init_store;
use athanor_core::CanonicalSnapshotStore;

pub const CAPABILITIES_REPORT_SCHEMA: &str = "athanor.capabilities.v1";

pub const DEFAULT_CAPABILITIES_LIMIT: usize = 50;
pub const DEFAULT_CONFIDENCE_THRESHOLD: f32 = 1.0;

const UNKNOWN_LANGUAGE: &str = "unknown";

/// Adapter that only records a baseline file inventory entry for every discovered
/// file. A file covered by this adapter alone has not received content extraction,
/// so completeness is measured against adapters other than this one.
const BASELINE_ADAPTER: &str = "file";

#[derive(Debug, Clone)]
pub struct CapabilitiesOptions {
    pub root: PathBuf,
    pub limit: usize,
    pub confidence_threshold: f32,
}

/// Bounded analysis-completeness report.
///
/// It answers where the knowledge graph is incomplete before agents rely on
/// query, graph, or daemon answers: which discovered files produced no canonical
/// objects, how completeness breaks down per language and per extractor adapter,
/// and which extracted facts carry below-threshold confidence.
#[derive(Debug, Clone, Serialize)]
pub struct CapabilitiesReport {
    pub schema: &'static str,
    pub snapshot: String,
    pub root: PathBuf,
    pub baseline_adapter: &'static str,
    pub limits: CapabilitiesLimits,
    pub totals: CapabilitiesTotals,
    pub languages: Vec<LanguageCapability>,
    pub adapters: Vec<AdapterCapability>,
    pub low_confidence_facts: Vec<LowConfidenceFact>,
    pub unprocessed_files: Vec<UnprocessedFile>,
    pub omitted: CapabilitiesOmitted,
}

#[derive(Debug, Clone, Serialize)]
pub struct CapabilitiesLimits {
    pub limit: usize,
    pub confidence_threshold: f32,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct CapabilitiesTotals {
    pub tracked_files: usize,
    pub processed_files: usize,
    pub unprocessed_files: usize,
    pub processed_ratio_percent: u8,
    pub languages: usize,
    pub adapters: usize,
    pub facts: usize,
    pub low_confidence_facts: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct LanguageCapability {
    pub language_hint: String,
    pub tracked_files: usize,
    pub processed_files: usize,
    pub unprocessed_files: usize,
    pub processed_ratio_percent: u8,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdapterCapability {
    pub adapter: String,
    pub processed_files: usize,
    pub facts: usize,
    pub low_confidence_facts: usize,
    pub min_confidence: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct LowConfidenceFact {
    pub fact_id: String,
    pub adapter: String,
    pub kind: String,
    pub confidence: f32,
    pub path: Option<String>,
    pub line_start: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UnprocessedFile {
    pub path: String,
    pub language_hint: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct CapabilitiesOmitted {
    pub languages: usize,
    pub adapters: usize,
    pub low_confidence_facts: usize,
    pub unprocessed_files: usize,
}

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

pub async fn capabilities_project(options: CapabilitiesOptions) -> Result<CapabilitiesReport> {
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

    Ok(build_capabilities_report(
        root,
        snapshot,
        state,
        options.limit,
        options.confidence_threshold,
    ))
}

fn build_capabilities_report(
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

#[cfg(test)]
mod tests {
    use athanor_domain::{
        Entity, EntityId, EntityKind, Evidence, EvidenceStatus, Fact, FactId, FactKind,
        LanguageCode, Ownership, SnapshotId, StableKey,
    };
    use serde_json::json;
    use std::collections::BTreeMap;

    use super::*;

    fn evidence(path: &str, extractor: &str, confidence: f32) -> Evidence {
        Evidence {
            source_file: Some(path.to_string()),
            line_start: Some(1),
            line_end: Some(1),
            extractor: Some(extractor.to_string()),
            commit_hash: None,
            confidence,
            status: EvidenceStatus::Verified,
        }
    }

    fn entity(id: &str, path: &str) -> Entity {
        Entity {
            id: EntityId(id.to_string()),
            stable_key: StableKey(format!("stable://{id}")),
            kind: EntityKind::DocumentationPage,
            name: id.to_string(),
            title: None,
            source: None,
            language: Some(LanguageCode("en".to_string())),
            aliases: Vec::new(),
            ownership: vec![Ownership {
                source_file: path.to_string(),
            }],
            payload: json!({}),
        }
    }

    fn fact(id: &str, path: &str, extractor: &str, confidence: f32) -> Fact {
        Fact {
            id: FactId(id.to_string()),
            kind: FactKind::DocSectionFound,
            subject: EntityId(format!("ent_{id}")),
            object: None,
            value: json!({}),
            evidence: vec![evidence(path, extractor, confidence)],
            ownership: vec![Ownership {
                source_file: path.to_string(),
            }],
            snapshot: SnapshotId("snap_caps".to_string()),
            extractor: extractor.to_string(),
            confidence,
        }
    }

    fn file_state(language: &str) -> crate::index_state::FileState {
        crate::index_state::FileState {
            content_hash: Some("hash".to_string()),
            language_hint: Some(language.to_string()),
        }
    }

    #[test]
    fn reports_unprocessed_files_and_low_confidence_facts() {
        let snapshot = CanonicalSnapshot {
            snapshot: Some(SnapshotId("snap_caps".to_string())),
            entities: vec![entity("ent_docs", "docs/api.md")],
            facts: vec![
                fact("fact_full", "docs/api.md", "MarkdownExtractor", 1.0),
                fact("fact_low", "docs/api.md", "MarkdownExtractor", 0.4),
            ],
            relations: Vec::new(),
            diagnostics: Vec::new(),
        };
        let state = IndexState {
            schema: crate::index_state::INDEX_STATE_SCHEMA.to_string(),
            snapshot: Some("snap_caps".to_string()),
            files: BTreeMap::from([
                ("docs/api.md".to_string(), file_state("markdown")),
                ("assets/logo.png".to_string(), file_state("binary")),
                ("scripts/build.fish".to_string(), file_state("unknown")),
            ]),
        };

        let report = build_capabilities_report(
            PathBuf::from("."),
            snapshot,
            state,
            50,
            DEFAULT_CONFIDENCE_THRESHOLD,
        );

        assert_eq!(report.schema, CAPABILITIES_REPORT_SCHEMA);
        assert_eq!(report.totals.tracked_files, 3);
        assert_eq!(report.totals.processed_files, 1);
        assert_eq!(report.totals.unprocessed_files, 2);
        assert_eq!(report.totals.processed_ratio_percent, 33);
        assert_eq!(report.totals.facts, 2);
        assert_eq!(report.totals.low_confidence_facts, 1);

        let unprocessed_paths = report
            .unprocessed_files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>();
        assert!(unprocessed_paths.contains(&"assets/logo.png"));
        assert!(unprocessed_paths.contains(&"scripts/build.fish"));
        assert!(!unprocessed_paths.contains(&"docs/api.md"));

        assert_eq!(report.low_confidence_facts.len(), 1);
        let low = &report.low_confidence_facts[0];
        assert_eq!(low.fact_id, "fact_low");
        assert_eq!(low.adapter, "MarkdownExtractor");
        assert_eq!(low.path.as_deref(), Some("docs/api.md"));

        let adapter = &report.adapters[0];
        assert_eq!(adapter.adapter, "MarkdownExtractor");
        assert_eq!(adapter.facts, 2);
        assert_eq!(adapter.low_confidence_facts, 1);
        assert!((adapter.min_confidence - 0.4).abs() < 1e-6);
    }

    #[test]
    fn applies_bounded_limits_and_reports_omitted_counts() {
        let mut files = BTreeMap::new();
        for index in 0..5 {
            files.insert(format!("src/skipped_{index}.zig"), file_state("unknown"));
        }
        let state = IndexState {
            schema: crate::index_state::INDEX_STATE_SCHEMA.to_string(),
            snapshot: Some("snap_caps".to_string()),
            files,
        };
        let snapshot = CanonicalSnapshot {
            snapshot: Some(SnapshotId("snap_caps".to_string())),
            entities: Vec::new(),
            facts: Vec::new(),
            relations: Vec::new(),
            diagnostics: Vec::new(),
        };

        let report = build_capabilities_report(
            PathBuf::from("."),
            snapshot,
            state,
            2,
            DEFAULT_CONFIDENCE_THRESHOLD,
        );

        assert_eq!(report.totals.unprocessed_files, 5);
        assert_eq!(report.unprocessed_files.len(), 2);
        assert_eq!(report.omitted.unprocessed_files, 3);
        assert_eq!(report.totals.processed_ratio_percent, 0);
    }
}
