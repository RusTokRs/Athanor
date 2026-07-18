use std::collections::BTreeMap;
use std::path::PathBuf;

use athanor_core::CanonicalSnapshot;
use athanor_domain::{
    Diagnostic, DiagnosticId, DiagnosticKind, DiagnosticStatus, Entity, EntityId, EntityKind,
    Evidence, EvidenceStatus, Fact, FactId, FactKind, LanguageCode, Ownership, Severity, SnapshotId,
    StableKey,
};
use serde_json::json;

use crate::index_state::IndexState;

use super::aggregation::build_coverage_report;
use super::model::COVERAGE_REPORT_SCHEMA;

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
