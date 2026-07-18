use std::collections::BTreeSet;

use athanor_domain::{Diagnostic, DiagnosticStatus, Entity, EntityKind, Severity};
use serde_json::Value;

use crate::check::{DiagnosticScope, diagnostic_matches_scope};
use crate::config::{CompletenessPolicy, DocsConfig};

use super::{
    DOCS_CHECK_SCHEMA, DOCS_DRIFT_SCHEMA, DocsCheckReport, DocsDriftReport, DocsPolicyViolation,
    DriftedDocument,
};

pub(super) fn build_docs_check_report(
    snapshot: String,
    entities: &[Entity],
    diagnostics: &[Diagnostic],
    config: &DocsConfig,
) -> DocsCheckReport {
    let editable_path = normalize_policy_path(&config.editable_path);
    let pages = entities
        .iter()
        .filter(|entity| is_editable_page(entity, &editable_path))
        .collect::<Vec<_>>();
    let page_ids = pages
        .iter()
        .map(|page| page.id.0.as_str())
        .collect::<BTreeSet<_>>();
    let mut policy_violations = pages
        .iter()
        .flat_map(|page| policy_violations(page, &snapshot, &config.completeness))
        .collect::<Vec<_>>();
    policy_violations.sort_by(|left, right| {
        (&left.path, &left.field, &left.stable_key).cmp(&(
            &right.path,
            &right.field,
            &right.stable_key,
        ))
    });

    let mut diagnostics = diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic.status == DiagnosticStatus::Open
                && diagnostic_matches_scope(&diagnostic.kind, DiagnosticScope::Docs)
                && diagnostic
                    .payload
                    .get("scope")
                    .and_then(Value::as_str)
                    .is_none()
                && severity_rank(diagnostic.severity)
                    <= severity_rank(config.completeness.minimum_diagnostic_severity)
                && diagnostic
                    .entities
                    .iter()
                    .any(|id| page_ids.contains(id.0.as_str()))
        })
        .cloned()
        .collect::<Vec<_>>();
    diagnostics
        .sort_by_key(|diagnostic| (severity_rank(diagnostic.severity), diagnostic.id.0.clone()));

    DocsCheckReport {
        schema: DOCS_CHECK_SCHEMA.to_string(),
        snapshot,
        passed: policy_violations.is_empty() && diagnostics.is_empty(),
        editable_documents: pages.len(),
        policy_violations,
        diagnostics,
    }
}

pub(crate) fn build_docs_drift_report(
    snapshot: String,
    accepted_previous_snapshot: Option<String>,
    entities: &[Entity],
    config: &DocsConfig,
) -> DocsDriftReport {
    let accepted_snapshots = accepted_verification_snapshots(&snapshot, accepted_previous_snapshot);
    let editable_path = normalize_policy_path(&config.editable_path);
    let pages = entities
        .iter()
        .filter(|entity| is_editable_page(entity, &editable_path))
        .collect::<Vec<_>>();
    let mut drifted_documents = pages
        .iter()
        .filter_map(|page| {
            let verified_snapshot = page.payload["last_verified_snapshot"]
                .as_str()
                .map(str::to_string);
            if verified_snapshot
                .as_deref()
                .is_some_and(|verified| accepted_snapshots.contains(verified))
            {
                return None;
            }
            Some(DriftedDocument {
                path: page
                    .source
                    .as_ref()
                    .map_or_else(|| page.name.clone(), |source| source.path.clone()),
                stable_key: page.stable_key.0.clone(),
                reason: if verified_snapshot.is_some() {
                    "verified_against_older_snapshot".to_string()
                } else {
                    "missing_verification_snapshot".to_string()
                },
                verified_snapshot,
            })
        })
        .collect::<Vec<_>>();
    drifted_documents.sort_by(|left, right| {
        (&left.path, &left.stable_key).cmp(&(&right.path, &right.stable_key))
    });

    DocsDriftReport {
        schema: DOCS_DRIFT_SCHEMA.to_string(),
        snapshot,
        editable_documents: pages.len(),
        current_documents: pages.len() - drifted_documents.len(),
        drifted_documents,
    }
}

pub(super) fn is_editable_page(entity: &Entity, editable_path: &str) -> bool {
    if entity.kind != EntityKind::DocumentationPage
        || entity.payload["documentation_layer"].as_str() != Some("editable")
    {
        return false;
    }
    entity.source.as_ref().is_some_and(|source| {
        let path = source.path.replace('\\', "/");
        path == editable_path || path.starts_with(&format!("{editable_path}/"))
    })
}

pub(super) fn page_path(page: &Entity) -> Option<String> {
    page.source.as_ref().map(|source| source.path.clone())
}

pub(super) fn normalize_policy_path(path: &str) -> String {
    path.replace('\\', "/").trim_matches('/').to_string()
}

fn accepted_verification_snapshots(
    snapshot: &str,
    accepted_previous_snapshot: Option<String>,
) -> BTreeSet<String> {
    let mut accepted = BTreeSet::from([snapshot.to_string()]);
    if let Some(previous) = accepted_previous_snapshot {
        accepted.insert(previous);
    } else if let Some(previous) = immediate_previous_snapshot(snapshot) {
        accepted.insert(previous);
    }
    accepted
}

fn immediate_previous_snapshot(snapshot: &str) -> Option<String> {
    let split_at = snapshot
        .char_indices()
        .rev()
        .find(|(_, character)| !character.is_ascii_digit())
        .map_or(0, |(index, character)| index + character.len_utf8());
    let (prefix, suffix) = snapshot.split_at(split_at);
    if prefix.is_empty() || suffix.is_empty() {
        return None;
    }
    let previous = suffix.parse::<u64>().ok()?.checked_sub(1)?;
    Some(format!("{prefix}{previous:0width$}", width = suffix.len()))
}

fn policy_violations(
    page: &Entity,
    snapshot: &str,
    policy: &CompletenessPolicy,
) -> Vec<DocsPolicyViolation> {
    let path = page
        .source
        .as_ref()
        .map_or_else(|| page.name.clone(), |source| source.path.clone());
    let present = page.payload["frontmatter_fields"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    let mut violations = policy
        .required_fields
        .iter()
        .filter(|field| !present.contains(field.as_str()))
        .map(|field| DocsPolicyViolation {
            path: path.clone(),
            stable_key: page.stable_key.0.clone(),
            field: field.clone(),
            message: format!("required frontmatter field `{field}` is missing"),
        })
        .collect::<Vec<_>>();

    if present.contains("status") {
        let status = page.payload["status"].as_str().unwrap_or_default();
        if !policy
            .allowed_statuses
            .iter()
            .any(|allowed| allowed == status)
        {
            violations.push(DocsPolicyViolation {
                path: path.clone(),
                stable_key: page.stable_key.0.clone(),
                field: "status".to_string(),
                message: format!("status `{status}` is not allowed by the completeness policy"),
            });
        }
    }
    if policy.require_current_snapshot
        && page.payload["last_verified_snapshot"].as_str() != Some(snapshot)
    {
        violations.push(DocsPolicyViolation {
            path,
            stable_key: page.stable_key.0.clone(),
            field: "last_verified_snapshot".to_string(),
            message: format!("document is not verified against current snapshot `{snapshot}`"),
        });
    }
    violations
}

fn severity_rank(severity: Severity) -> u8 {
    match severity {
        Severity::Critical => 0,
        Severity::High => 1,
        Severity::Medium => 2,
        Severity::Low => 3,
    }
}
