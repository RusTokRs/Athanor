use serde_json::json;

use crate::config::DocsConfig;

use super::super::check::{build_docs_check_report, build_docs_drift_report};
use super::fixtures::page;

#[test]
fn completeness_reports_only_editable_policy_gaps() {
    let incomplete = page(
        "docs/auth.md",
        json!({
            "frontmatter_fields": ["id", "language", "status"],
            "status": "draft"
        }),
    );
    let report = build_docs_check_report(
        "snap_current".to_string(),
        &[incomplete],
        &[],
        &DocsConfig::default(),
    );
    assert_eq!(report.editable_documents, 1);
    assert!(!report.passed);
    assert!(report.policy_violations.iter().any(|issue| issue.field == "kind"));
    assert!(report.policy_violations.iter().any(|issue| issue.field == "status"));
}

#[test]
fn drift_accepts_current_and_immediate_previous_snapshot() {
    let current = page(
        "docs/current.md",
        json!({"last_verified_snapshot": "snap_jsonl_00000042"}),
    );
    let previous = page(
        "docs/previous.md",
        json!({"last_verified_snapshot": "snap_jsonl_00000041"}),
    );
    let stale = page(
        "docs/stale.md",
        json!({"last_verified_snapshot": "snap_jsonl_00000040"}),
    );
    let report = build_docs_drift_report(
        "snap_jsonl_00000042".to_string(),
        None,
        &[current, previous, stale],
        &DocsConfig::default(),
    );
    assert_eq!(report.current_documents, 2);
    assert_eq!(report.drifted_documents.len(), 1);
    assert_eq!(report.drifted_documents[0].path, "docs/stale.md");
}
