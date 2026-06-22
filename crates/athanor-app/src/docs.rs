use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use athanor_core::CanonicalSnapshotStore;
use athanor_domain::{Diagnostic, DiagnosticStatus, Entity, EntityKind, Severity};
use athanor_store_jsonl::JsonlKnowledgeStore;
use serde::{Deserialize, Serialize};

use crate::check::{DiagnosticScope, diagnostic_matches_scope};
use crate::project_path::normalize_canonical_path;

pub const DOCS_CHECK_SCHEMA: &str = "athanor.docs_check.v1";

#[derive(Debug, Clone)]
pub struct DocsCheckOptions {
    pub root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DocsPolicyViolation {
    pub path: String,
    pub stable_key: String,
    pub field: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DocsCheckReport {
    pub schema: String,
    pub snapshot: String,
    pub passed: bool,
    pub editable_documents: usize,
    pub policy_violations: Vec<DocsPolicyViolation>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct ProjectConfig {
    docs: DocsConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
struct DocsConfig {
    editable_path: String,
    completeness: CompletenessPolicy,
}

impl Default for DocsConfig {
    fn default() -> Self {
        Self {
            editable_path: "docs".to_string(),
            completeness: CompletenessPolicy::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
struct CompletenessPolicy {
    required_fields: Vec<String>,
    allowed_statuses: Vec<String>,
    minimum_diagnostic_severity: Severity,
    require_current_snapshot: bool,
}

impl Default for CompletenessPolicy {
    fn default() -> Self {
        Self {
            required_fields: vec![
                "id".to_string(),
                "kind".to_string(),
                "language".to_string(),
                "source_language".to_string(),
                "last_verified_snapshot".to_string(),
                "status".to_string(),
            ],
            allowed_statuses: vec!["verified".to_string()],
            minimum_diagnostic_severity: Severity::Medium,
            require_current_snapshot: false,
        }
    }
}

pub async fn check_docs(options: DocsCheckOptions) -> Result<DocsCheckReport> {
    let root = normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );
    let config = load_config(&root)?;
    let store = JsonlKnowledgeStore::new(root.join(".athanor/store/canonical/jsonl"));
    let canonical = store
        .load_latest_snapshot()
        .await
        .context("failed to load latest canonical snapshot")?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "no canonical snapshot found; run `ath index {}` first",
                root.display()
            )
        })?;
    let snapshot = canonical
        .snapshot
        .as_ref()
        .map_or_else(|| "unknown".to_string(), |snapshot| snapshot.0.clone());

    Ok(build_docs_check_report(
        snapshot,
        &canonical.entities,
        &canonical.diagnostics,
        &config.docs,
    ))
}

fn load_config(root: &Path) -> Result<ProjectConfig> {
    let path = root.join("athanor.toml");
    if !path.exists() {
        return Ok(ProjectConfig::default());
    }
    let content =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    toml::from_str(&content).with_context(|| format!("failed to parse {}", path.display()))
}

fn build_docs_check_report(
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

fn is_editable_page(entity: &Entity, editable_path: &str) -> bool {
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
        .filter_map(serde_json::Value::as_str)
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

fn normalize_policy_path(path: &str) -> String {
    path.replace('\\', "/").trim_matches('/').to_string()
}

fn severity_rank(severity: Severity) -> u8 {
    match severity {
        Severity::Critical => 0,
        Severity::High => 1,
        Severity::Medium => 2,
        Severity::Low => 3,
    }
}

#[cfg(test)]
mod tests {
    use athanor_domain::{EntityId, LanguageCode, Ownership, SourceLocation, StableKey};
    use serde_json::json;

    use super::*;

    #[test]
    fn reports_missing_required_fields_for_editable_docs_only() {
        let editable = page(
            "docs/auth.md",
            "editable",
            json!({
                "documentation_layer": "editable",
                "frontmatter_fields": ["id", "language", "status"],
                "status": "draft"
            }),
        );
        let generated = page(
            "docs/generated.md",
            "generated",
            json!({
                "documentation_layer": "generated",
                "frontmatter_fields": []
            }),
        );

        let report = build_docs_check_report(
            "snap_current".to_string(),
            &[editable, generated],
            &[],
            &DocsConfig::default(),
        );

        assert_eq!(report.editable_documents, 1);
        assert!(!report.passed);
        assert!(
            report
                .policy_violations
                .iter()
                .any(|issue| issue.field == "kind")
        );
        assert!(
            report
                .policy_violations
                .iter()
                .any(|issue| issue.field == "status")
        );
    }

    #[test]
    fn passes_complete_document_under_configured_path() {
        let page = page(
            "docs/auth.md",
            "editable",
            json!({
                "documentation_layer": "editable",
                "frontmatter_fields": ["id", "kind", "language", "source_language", "last_verified_snapshot", "status"],
                "status": "verified",
                "last_verified_snapshot": "snap_previous"
            }),
        );
        let report = build_docs_check_report(
            "snap_current".to_string(),
            &[page],
            &[],
            &DocsConfig::default(),
        );
        assert!(report.passed);
    }

    fn page(path: &str, layer: &str, payload: serde_json::Value) -> Entity {
        Entity {
            id: EntityId(format!("ent_{path}")),
            stable_key: StableKey(format!("doc://{path}")),
            kind: EntityKind::DocumentationPage,
            name: path.to_string(),
            title: Some("Title".to_string()),
            source: Some(SourceLocation {
                path: path.to_string(),
                line_start: Some(1),
                line_end: Some(1),
            }),
            language: Some(LanguageCode("en".to_string())),
            aliases: Vec::new(),
            ownership: vec![Ownership {
                source_file: path.to_string(),
            }],
            payload: payload.as_object().map_or(payload.clone(), |object| {
                let mut object = object.clone();
                object.insert("documentation_layer".to_string(), json!(layer));
                serde_json::Value::Object(object)
            }),
        }
    }
}
