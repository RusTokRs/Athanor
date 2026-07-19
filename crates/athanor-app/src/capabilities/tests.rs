use std::collections::BTreeMap;
use std::path::PathBuf;

use athanor_core::CanonicalSnapshot;
use athanor_domain::{
    Entity, EntityId, EntityKind, Evidence, EvidenceStatus, Fact, FactId, FactKind, LanguageCode,
    Ownership, SnapshotId, StableKey,
};
use serde_json::json;

use crate::index_state::IndexState;

use super::aggregation::build_capabilities_report;
use super::model::{CAPABILITIES_REPORT_SCHEMA, DEFAULT_CONFIDENCE_THRESHOLD};

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
