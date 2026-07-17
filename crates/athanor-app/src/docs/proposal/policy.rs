use std::collections::BTreeMap;

use athanor_domain::{Diagnostic, Entity};
use serde_json::Value;

use crate::config::DocsConfig;

use super::shared::push_change;
use super::super::super::check::{build_docs_check_report, build_docs_drift_report};
use super::super::super::{DocsFrontmatterChange, DocsPatchOperation, DocsPolicyViolation};

pub(super) fn add(
    changes: &mut BTreeMap<String, DocsPatchOperation>,
    snapshot: &str,
    config: &DocsConfig,
    pages: &BTreeMap<String, &Entity>,
    entities: &[Entity],
    diagnostics: &[Diagnostic],
) {
    let check = build_docs_check_report(
        snapshot.to_string(),
        entities,
        diagnostics,
        config,
    );
    let drift = build_docs_drift_report(
        snapshot.to_string(),
        None,
        entities,
        config,
    );

    for violation in check.policy_violations {
        let Some(page) = pages.get(&violation.path) else {
            continue;
        };
        if let Some(change) = change_for_violation(page, snapshot, &violation, config) {
            push_change(changes, &violation.path, page, change);
        }
    }
    for document in drift.drifted_documents {
        let Some(page) = pages.get(&document.path) else {
            continue;
        };
        push_change(
            changes,
            &document.path,
            page,
            DocsFrontmatterChange {
                field: "last_verified_snapshot".to_string(),
                old_value: document.verified_snapshot.map(Value::String),
                new_value: Value::String(snapshot.to_string()),
                reason: document.reason,
            },
        );
    }
}

fn change_for_violation(
    page: &Entity,
    snapshot: &str,
    violation: &DocsPolicyViolation,
    config: &DocsConfig,
) -> Option<DocsFrontmatterChange> {
    let old_value = page.payload.get(&violation.field).cloned();
    let new_value = match violation.field.as_str() {
        "id" => Value::String(page.stable_key.0.clone()),
        "kind" => Value::String(
            page.payload["documentation_kind"]
                .as_str()
                .unwrap_or("project_overview")
                .to_string(),
        ),
        "language" => Value::String(
            page.language
                .as_ref()
                .map_or_else(|| "en".to_string(), |language| language.0.clone()),
        ),
        "source_language" => Value::String(
            page.payload["source_language"].as_str().map_or_else(
                || {
                    page.language
                        .as_ref()
                        .map_or_else(|| "en".to_string(), |language| language.0.clone())
                },
                str::to_string,
            ),
        ),
        "last_verified_snapshot" => Value::String(snapshot.to_string()),
        "status" => Value::String(
            config
                .completeness
                .allowed_statuses
                .first()
                .cloned()
                .unwrap_or_else(|| "verified".to_string()),
        ),
        _ => return None,
    };
    (old_value.as_ref() != Some(&new_value)).then(|| DocsFrontmatterChange {
        field: violation.field.clone(),
        old_value,
        new_value,
        reason: violation.message.clone(),
    })
}
