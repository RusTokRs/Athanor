use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::store::init_store;
use anyhow::{Context, Result};
use athanor_core::CanonicalSnapshotStore;
use athanor_domain::{Diagnostic, DiagnosticKind, DiagnosticStatus, Entity, EntityKind, Severity};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::check::{DiagnosticScope, diagnostic_matches_scope};
use crate::project_path::normalize_canonical_path;

pub const DOCS_CHECK_SCHEMA: &str = "athanor.docs_check.v1";
pub const DOCS_DRIFT_SCHEMA: &str = "athanor.docs_drift.v1";
pub const DOCS_PATCH_SCHEMA: &str = "athanor.docs_patch.v1";

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

#[derive(Debug, Clone)]
pub struct DocsDriftOptions {
    pub root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DriftedDocument {
    pub path: String,
    pub stable_key: String,
    pub verified_snapshot: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DocsDriftReport {
    pub schema: String,
    pub snapshot: String,
    pub editable_documents: usize,
    pub current_documents: usize,
    pub drifted_documents: Vec<DriftedDocument>,
}

#[derive(Debug, Clone)]
pub struct DocsProposeFixOptions {
    pub root: PathBuf,
    pub output: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct DocsApplyPatchOptions {
    pub root: PathBuf,
    pub patch: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocsFrontmatterChange {
    pub field: String,
    pub old_value: Option<Value>,
    pub new_value: Value,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocsPatchOperation {
    pub path: String,
    pub stable_key: String,
    #[serde(default)]
    pub create: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    pub changes: Vec<DocsFrontmatterChange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocsPatchProposal {
    pub schema: String,
    pub id: String,
    pub snapshot: String,
    pub operations: Vec<DocsPatchOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DocsProposeFixReport {
    pub proposal: DocsPatchProposal,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DocsApplyPatchReport {
    pub schema: String,
    pub id: String,
    pub snapshot: String,
    pub files_changed: usize,
    pub changes_applied: usize,
}

use crate::config::{CompletenessPolicy, DocsConfig, ProjectConfig, load_config};

pub async fn check_docs(options: DocsCheckOptions) -> Result<DocsCheckReport> {
    let (canonical, config) = load_docs_snapshot(&options.root).await?;
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

pub async fn docs_drift(options: DocsDriftOptions) -> Result<DocsDriftReport> {
    let (canonical, config) = load_docs_snapshot(&options.root).await?;
    let snapshot = canonical
        .snapshot
        .as_ref()
        .map_or_else(|| "unknown".to_string(), |snapshot| snapshot.0.clone());
    Ok(build_docs_drift_report(
        snapshot,
        &canonical.entities,
        &config.docs,
    ))
}

pub async fn docs_propose_fix(options: DocsProposeFixOptions) -> Result<DocsProposeFixReport> {
    let root = normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );
    let (canonical, config) = load_docs_snapshot(&root).await?;
    let snapshot = canonical
        .snapshot
        .as_ref()
        .map_or_else(|| "unknown".to_string(), |snapshot| snapshot.0.clone());
    let proposal = build_docs_patch_proposal(
        snapshot,
        &canonical.entities,
        &canonical.diagnostics,
        &config.docs,
    );
    let path = match options.output {
        Some(output) => resolve_project_output(&root, &output)?,
        None => allocate_docs_patch_path(&root, &proposal.id)?,
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(
        &path,
        serde_json::to_string_pretty(&proposal).context("failed to serialize docs patch")?,
    )
    .with_context(|| format!("failed to write {}", path.display()))?;

    Ok(DocsProposeFixReport { proposal, path })
}

pub async fn docs_apply_patch(options: DocsApplyPatchOptions) -> Result<DocsApplyPatchReport> {
    let root = normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );
    let patch_path = resolve_docs_patch_path(&root, &options.patch)?;
    let content = fs::read_to_string(&patch_path)
        .with_context(|| format!("failed to read {}", patch_path.display()))?;
    let proposal: DocsPatchProposal =
        serde_json::from_str(&content).context("failed to parse docs patch proposal")?;
    if proposal.schema != DOCS_PATCH_SCHEMA {
        anyhow::bail!(
            "unsupported docs patch schema `{}`; expected `{}`",
            proposal.schema,
            DOCS_PATCH_SCHEMA
        );
    }

    let (canonical, _) = load_docs_snapshot(&root).await?;
    let current_snapshot = canonical
        .snapshot
        .as_ref()
        .map_or_else(|| "unknown".to_string(), |snapshot| snapshot.0.clone());
    if proposal.snapshot != current_snapshot {
        anyhow::bail!(
            "docs patch targets snapshot `{}`, but latest snapshot is `{}`; regenerate the proposal",
            proposal.snapshot,
            current_snapshot
        );
    }

    let mut files_changed = 0;
    let mut changes_applied = 0;
    for operation in &proposal.operations {
        let path = safe_project_path(&root, &operation.path)?;
        if let Some(content) = &operation.content {
            if operation.create && path.exists() {
                anyhow::bail!(
                    "refusing to create `{}` because the file already exists",
                    operation.path
                );
            }
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            fs::write(&path, content)
                .with_context(|| format!("failed to write {}", path.display()))?;
            files_changed += 1;
        } else {
            let content = fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            let updated = apply_frontmatter_changes(&content, &operation.changes)
                .with_context(|| format!("failed to update frontmatter in {}", operation.path))?;
            if updated != content {
                fs::write(&path, updated)
                    .with_context(|| format!("failed to write {}", path.display()))?;
                files_changed += 1;
            }
        }
        changes_applied += operation.changes.len() + usize::from(operation.content.is_some());
    }

    Ok(DocsApplyPatchReport {
        schema: "athanor.docs_apply_patch.v1".to_string(),
        id: proposal.id,
        snapshot: proposal.snapshot,
        files_changed,
        changes_applied,
    })
}

async fn load_docs_snapshot(
    root: &Path,
) -> Result<(athanor_core::CanonicalSnapshot, ProjectConfig)> {
    let root = normalize_canonical_path(
        root.canonicalize()
            .with_context(|| format!("failed to canonicalize {}", root.display()))?,
    );
    let config = load_config(&root)?;
    let store = init_store(&root, &config).await?;
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
    Ok((canonical, config))
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

fn build_docs_drift_report(
    snapshot: String,
    entities: &[Entity],
    config: &DocsConfig,
) -> DocsDriftReport {
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
            if verified_snapshot.as_deref() == Some(snapshot.as_str()) {
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

fn build_docs_patch_proposal(
    snapshot: String,
    entities: &[Entity],
    diagnostics: &[Diagnostic],
    config: &DocsConfig,
) -> DocsPatchProposal {
    let check = build_docs_check_report(snapshot.clone(), entities, diagnostics, config);
    let drift = build_docs_drift_report(snapshot.clone(), entities, config);
    let editable_path = normalize_policy_path(&config.editable_path);
    let pages = entities
        .iter()
        .filter(|entity| is_editable_page(entity, &editable_path))
        .collect::<Vec<_>>();
    let pages_by_path = pages
        .iter()
        .filter_map(|page| page_path(page).map(|path| (path, *page)))
        .collect::<BTreeMap<_, _>>();

    let mut changes_by_path = BTreeMap::<String, DocsPatchOperation>::new();

    for violation in check.policy_violations {
        let Some(page) = pages_by_path.get(&violation.path) else {
            continue;
        };
        let Some(change) = change_for_policy_violation(page, &snapshot, &violation, config) else {
            continue;
        };
        push_patch_change(&mut changes_by_path, &violation.path, page, change);
    }

    for document in drift.drifted_documents {
        let Some(page) = pages_by_path.get(&document.path) else {
            continue;
        };
        let change = DocsFrontmatterChange {
            field: "last_verified_snapshot".to_string(),
            old_value: document.verified_snapshot.map(Value::String),
            new_value: Value::String(snapshot.clone()),
            reason: document.reason,
        };
        push_patch_change(&mut changes_by_path, &document.path, page, change);
    }

    for diagnostic in diagnostics.iter().filter(|diagnostic| {
        diagnostic.status == DiagnosticStatus::Open
            && diagnostic.kind == DiagnosticKind::ApiEndpointImplementedButNotDocumented
    }) {
        let Some(endpoint_key) = diagnostic
            .payload
            .get("endpoint")
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        let Some(endpoint) = entities.iter().find(|entity| {
            entity.kind == EntityKind::ApiEndpoint && entity.stable_key.0 == endpoint_key
        }) else {
            continue;
        };
        let path = api_doc_path(&config.editable_path, endpoint);
        if pages_by_path.contains_key(&path) || changes_by_path.contains_key(&path) {
            continue;
        }
        changes_by_path.insert(
            path.clone(),
            DocsPatchOperation {
                path: path.clone(),
                stable_key: format!("doc://{path}"),
                create: true,
                content: Some(api_doc_content(&snapshot, endpoint, diagnostic, &path)),
                changes: Vec::new(),
            },
        );
    }

    for diagnostic in diagnostics.iter().filter(|diagnostic| {
        diagnostic.status == DiagnosticStatus::Open
            && diagnostic.kind == DiagnosticKind::MissingEnvVar
    }) {
        let Some(env_key) = diagnostic
            .payload
            .get("env_var")
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        let Some(env_var) = entities
            .iter()
            .find(|entity| entity.kind == EntityKind::EnvVar && entity.stable_key.0 == env_key)
        else {
            continue;
        };
        let path = env_doc_path(&config.editable_path, env_var);
        if pages_by_path.contains_key(&path) || changes_by_path.contains_key(&path) {
            continue;
        }
        changes_by_path.insert(
            path.clone(),
            DocsPatchOperation {
                path: path.clone(),
                stable_key: format!("doc://{path}"),
                create: true,
                content: Some(env_doc_content(&snapshot, env_var, diagnostic, &path)),
                changes: Vec::new(),
            },
        );
    }

    let operations = changes_by_path.into_values().collect::<Vec<_>>();
    DocsPatchProposal {
        schema: DOCS_PATCH_SCHEMA.to_string(),
        id: docs_patch_id(&snapshot),
        snapshot,
        operations,
    }
}

fn change_for_policy_violation(
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
        "source_language" => Value::String(page.payload["source_language"].as_str().map_or_else(
            || {
                page.language
                    .as_ref()
                    .map_or_else(|| "en".to_string(), |language| language.0.clone())
            },
            str::to_string,
        )),
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
    if old_value.as_ref() == Some(&new_value) {
        return None;
    }
    Some(DocsFrontmatterChange {
        field: violation.field.clone(),
        old_value,
        new_value,
        reason: violation.message.clone(),
    })
}

fn push_patch_change(
    changes_by_path: &mut BTreeMap<String, DocsPatchOperation>,
    path: &str,
    page: &Entity,
    change: DocsFrontmatterChange,
) {
    let operation = changes_by_path
        .entry(path.to_string())
        .or_insert_with(|| DocsPatchOperation {
            path: path.to_string(),
            stable_key: page.stable_key.0.clone(),
            create: false,
            content: None,
            changes: Vec::new(),
        });
    if let Some(existing) = operation
        .changes
        .iter_mut()
        .find(|existing| existing.field == change.field)
    {
        *existing = change;
    } else {
        operation.changes.push(change);
        operation
            .changes
            .sort_by(|left, right| left.field.cmp(&right.field));
    }
}

fn api_doc_path(editable_path: &str, endpoint: &Entity) -> String {
    let method = endpoint.payload["method"]
        .as_str()
        .unwrap_or("endpoint")
        .to_ascii_lowercase();
    let route = endpoint
        .payload
        .get("path")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(endpoint.name.as_str());
    let slug = route
        .trim_matches('/')
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let slug = if slug.is_empty() {
        "root".to_string()
    } else {
        slug
    };
    format!(
        "{}/api/{}-{}.md",
        normalize_policy_path(editable_path),
        method,
        slug
    )
}

fn api_doc_content(
    snapshot: &str,
    endpoint: &Entity,
    diagnostic: &Diagnostic,
    path: &str,
) -> String {
    let method = endpoint.payload["method"]
        .as_str()
        .unwrap_or("UNKNOWN")
        .to_ascii_uppercase();
    let route = endpoint
        .payload
        .get("path")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(endpoint.name.as_str());
    let summary = endpoint
        .payload
        .get("summary")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("API endpoint");
    let operation_id = endpoint
        .payload
        .get("operation_id")
        .and_then(serde_json::Value::as_str);
    let title = format!("{method} {route}");

    let mut content = String::new();
    content.push_str("---\n");
    content.push_str(&format!("id: doc://{path}\n"));
    content.push_str("kind: api_documentation\n");
    content.push_str("language: en\n");
    content.push_str("source_language: en\n");
    content.push_str("entities:\n");
    content.push_str(&format!("  - {}\n", endpoint.stable_key.0));
    content.push_str(&format!("last_verified_snapshot: {snapshot}\n"));
    content.push_str("status: verified\n");
    content.push_str("---\n\n");
    content.push_str(&format!("# {title}\n\n"));
    content.push_str(summary);
    content.push_str("\n\n## Contract\n\n");
    content.push_str(&format!("- Method: `{method}`\n"));
    content.push_str(&format!("- Path: `{route}`\n"));
    if let Some(operation_id) = operation_id {
        content.push_str(&format!("- Operation ID: `{operation_id}`\n"));
    }
    content.push_str(&format!(
        "- Canonical entity: `{}`\n\n",
        endpoint.stable_key.0
    ));
    content.push_str("## Evidence\n\n");
    if diagnostic.evidence.is_empty() {
        content.push_str("- `unknown source`\n");
    } else {
        for evidence in &diagnostic.evidence {
            let source = evidence.source_file.as_deref().unwrap_or("unknown source");
            let line = evidence
                .line_start
                .map_or_else(String::new, |line| format!(":{line}"));
            content.push_str(&format!("- `{source}{line}`\n"));
        }
    }
    content.push_str("\n## Notes\n\n");
    content.push_str(&format!(
        "Generated from diagnostic `{}`. Review this page before relying on it as user-facing documentation.\n",
        diagnostic.id.0
    ));
    content
}

fn env_doc_path(editable_path: &str, env_var: &Entity) -> String {
    let name = env_var
        .stable_key
        .0
        .strip_prefix("env://")
        .unwrap_or(env_var.name.as_str());
    let slug = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let slug = if slug.is_empty() {
        "variable".to_string()
    } else {
        slug
    };
    format!(
        "{}/operations/env-{}.md",
        normalize_policy_path(editable_path),
        slug
    )
}

fn env_doc_content(
    snapshot: &str,
    env_var: &Entity,
    diagnostic: &Diagnostic,
    path: &str,
) -> String {
    let name = env_var
        .stable_key
        .0
        .strip_prefix("env://")
        .unwrap_or(env_var.name.as_str());
    let mut content = String::new();
    content.push_str("---\n");
    content.push_str(&format!("id: doc://{path}\n"));
    content.push_str("kind: operations_documentation\n");
    content.push_str("language: en\n");
    content.push_str("source_language: en\n");
    content.push_str("entities:\n");
    content.push_str(&format!("  - {}\n", env_var.stable_key.0));
    content.push_str(&format!("last_verified_snapshot: {snapshot}\n"));
    content.push_str("status: verified\n");
    content.push_str("---\n\n");
    content.push_str(&format!("# Environment Variable `{name}`\n\n"));
    content.push_str("## Purpose\n\n");
    content.push_str("Document the runtime purpose, expected format, and default behavior for this environment variable.\n\n");
    content.push_str("## Contract\n\n");
    content.push_str(&format!("- Variable: `{name}`\n"));
    content.push_str(&format!(
        "- Canonical entity: `{}`\n\n",
        env_var.stable_key.0
    ));
    content.push_str("## Evidence\n\n");
    if diagnostic.evidence.is_empty() {
        content.push_str("- `unknown source`\n");
    } else {
        for evidence in &diagnostic.evidence {
            let source = evidence.source_file.as_deref().unwrap_or("unknown source");
            let line = evidence
                .line_start
                .map_or_else(String::new, |line| format!(":{line}"));
            content.push_str(&format!("- `{source}{line}`\n"));
        }
    }
    content.push_str("\n## Notes\n\n");
    content.push_str(&format!(
        "Generated from diagnostic `{}`. Review this page before relying on it as operational documentation.\n",
        diagnostic.id.0
    ));
    content
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

fn page_path(page: &Entity) -> Option<String> {
    page.source.as_ref().map(|source| source.path.clone())
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

fn docs_patch_id(snapshot: &str) -> String {
    let safe_snapshot = snapshot
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    format!("docs_patch_{safe_snapshot}")
}

fn allocate_docs_patch_path(root: &Path, id: &str) -> Result<PathBuf> {
    let dir = root.join(".athanor/patches/docs");
    for index in 0.. {
        let suffix = if index == 0 {
            String::new()
        } else {
            format!("_{index}")
        };
        let path = dir.join(format!("{id}{suffix}.json"));
        if !path.exists() {
            return Ok(path);
        }
    }
    unreachable!("unbounded docs patch path allocation should always return")
}

fn resolve_project_output(root: &Path, output: &Path) -> Result<PathBuf> {
    if output.is_absolute() {
        return Ok(output.to_path_buf());
    }
    safe_project_path(root, output.to_string_lossy().as_ref())
}

fn resolve_docs_patch_path(root: &Path, patch: &str) -> Result<PathBuf> {
    let path = Path::new(patch);
    if path.is_absolute() || patch.contains('/') || patch.contains('\\') || patch.ends_with(".json")
    {
        return resolve_project_output(root, path);
    }
    Ok(root
        .join(".athanor/patches/docs")
        .join(format!("{patch}.json")))
}

fn safe_project_path(root: &Path, relative: &str) -> Result<PathBuf> {
    let path = Path::new(relative);
    if path.is_absolute() {
        anyhow::bail!("project-relative path expected, got `{relative}`");
    }
    for component in path.components() {
        match component {
            Component::Normal(_) => {}
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                anyhow::bail!("unsafe project path `{relative}`")
            }
        }
    }
    Ok(root.join(path))
}

fn apply_frontmatter_changes(content: &str, changes: &[DocsFrontmatterChange]) -> Result<String> {
    let (frontmatter, body) = split_frontmatter(content)?;
    let mut frontmatter = frontmatter
        .map(|yaml| {
            if yaml.trim().is_empty() {
                Ok(BTreeMap::<String, Value>::new())
            } else {
                serde_yaml_ng::from_str::<BTreeMap<String, Value>>(yaml)
                    .context("invalid existing frontmatter YAML")
            }
        })
        .transpose()?
        .unwrap_or_default();

    for change in changes {
        frontmatter.insert(change.field.clone(), change.new_value.clone());
    }

    let yaml = serde_yaml_ng::to_string(&frontmatter).context("failed to serialize frontmatter")?;
    let mut updated = String::new();
    updated.push_str("---\n");
    updated.push_str(&yaml);
    if !yaml.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str("---\n");
    updated.push_str(body.strip_prefix(['\r', '\n']).unwrap_or(body));
    Ok(updated)
}

fn split_frontmatter(content: &str) -> Result<(Option<&str>, &str)> {
    let mut lines = content.split_inclusive('\n');
    let Some(first) = lines.next() else {
        return Ok((None, content));
    };
    if first.trim_end_matches(['\r', '\n']) != "---" {
        return Ok((None, content));
    }

    let yaml_start = first.len();
    let mut cursor = yaml_start;
    for line in lines {
        let line_start = cursor;
        cursor += line.len();
        if line.trim_end_matches(['\r', '\n']) == "---" {
            return Ok((Some(&content[yaml_start..line_start]), &content[cursor..]));
        }
    }
    anyhow::bail!("Markdown frontmatter is missing its closing `---` delimiter")
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

    #[test]
    fn reports_missing_and_stale_document_verification() {
        let current = page(
            "docs/current.md",
            "editable",
            json!({"last_verified_snapshot": "snap_current"}),
        );
        let stale = page(
            "docs/stale.md",
            "editable",
            json!({"last_verified_snapshot": "snap_previous"}),
        );
        let missing = page("docs/missing.md", "editable", json!({}));

        let report = build_docs_drift_report(
            "snap_current".to_string(),
            &[current, stale, missing],
            &DocsConfig::default(),
        );

        assert_eq!(report.editable_documents, 3);
        assert_eq!(report.current_documents, 1);
        assert_eq!(report.drifted_documents.len(), 2);
        assert_eq!(
            report.drifted_documents[0].reason,
            "missing_verification_snapshot"
        );
        assert_eq!(
            report.drifted_documents[1].reason,
            "verified_against_older_snapshot"
        );
    }

    #[test]
    fn proposes_frontmatter_patch_for_policy_and_drift() {
        let page = page(
            "docs/auth.md",
            "editable",
            json!({
                "documentation_layer": "editable",
                "frontmatter_fields": ["id", "language", "status"],
                "status": "draft",
                "last_verified_snapshot": "snap_previous"
            }),
        );

        let proposal = build_docs_patch_proposal(
            "snap_current".to_string(),
            &[page],
            &[],
            &DocsConfig::default(),
        );

        assert_eq!(proposal.schema, DOCS_PATCH_SCHEMA);
        assert_eq!(proposal.operations.len(), 1);
        let operation = &proposal.operations[0];
        assert_eq!(operation.path, "docs/auth.md");
        assert!(operation.changes.iter().any(|change| {
            change.field == "kind" && change.new_value == Value::String("project_overview".into())
        }));
        assert!(operation.changes.iter().any(|change| {
            change.field == "last_verified_snapshot"
                && change.new_value == Value::String("snap_current".into())
        }));
        assert!(
            operation
                .changes
                .iter()
                .any(|change| change.field == "status"
                    && change.new_value == Value::String("verified".into()))
        );
    }

    #[test]
    fn proposes_api_documentation_creation_for_missing_api_docs() {
        let endpoint = api_endpoint();
        let diagnostic = api_missing_docs_diagnostic(&endpoint);

        let proposal = build_docs_patch_proposal(
            "snap_current".to_string(),
            &[endpoint],
            &[diagnostic],
            &DocsConfig::default(),
        );

        assert_eq!(proposal.operations.len(), 1);
        let operation = &proposal.operations[0];
        assert_eq!(operation.path, "docs/api/post-login.md");
        assert!(operation.create);
        assert_eq!(operation.changes.len(), 0);
        let content = operation.content.as_deref().unwrap();
        assert!(content.contains("kind: api_documentation\n"));
        assert!(content.contains("  - api://POST:/login\n"));
        assert!(content.contains("# POST /login\n"));
    }

    #[test]
    fn proposes_environment_documentation_creation_for_missing_env_docs() {
        let env_var = env_var();
        let diagnostic = missing_env_var_diagnostic(&env_var);

        let proposal = build_docs_patch_proposal(
            "snap_current".to_string(),
            &[env_var],
            &[diagnostic],
            &DocsConfig::default(),
        );

        assert_eq!(proposal.operations.len(), 1);
        let operation = &proposal.operations[0];
        assert_eq!(operation.path, "docs/operations/env-database-url.md");
        assert!(operation.create);
        assert_eq!(operation.changes.len(), 0);
        let content = operation.content.as_deref().unwrap();
        assert!(content.contains("kind: operations_documentation\n"));
        assert!(content.contains("  - env://DATABASE_URL\n"));
        assert!(content.contains("# Environment Variable `DATABASE_URL`\n"));
    }

    #[test]
    fn applies_frontmatter_changes_without_touching_body() {
        let content = "---\nid: doc://docs/auth.md\nstatus: draft\n---\n# Auth\n";
        let updated = apply_frontmatter_changes(
            content,
            &[
                DocsFrontmatterChange {
                    field: "status".to_string(),
                    old_value: Some(Value::String("draft".into())),
                    new_value: Value::String("verified".into()),
                    reason: "status is not allowed".to_string(),
                },
                DocsFrontmatterChange {
                    field: "last_verified_snapshot".to_string(),
                    old_value: None,
                    new_value: Value::String("snap_current".into()),
                    reason: "missing verification".to_string(),
                },
            ],
        )
        .unwrap();

        assert!(updated.contains("status: verified\n"));
        assert!(updated.contains("last_verified_snapshot: snap_current\n"));
        assert!(updated.ends_with("# Auth\n"));
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

    fn api_endpoint() -> Entity {
        Entity {
            id: EntityId("ent_endpoint".to_string()),
            stable_key: StableKey("api://POST:/login".to_string()),
            kind: EntityKind::ApiEndpoint,
            name: "POST /login".to_string(),
            title: Some("Login".to_string()),
            source: Some(SourceLocation {
                path: "openapi.yaml".to_string(),
                line_start: Some(10),
                line_end: Some(20),
            }),
            language: Some(LanguageCode("openapi".to_string())),
            aliases: Vec::new(),
            ownership: vec![Ownership {
                source_file: "openapi.yaml".to_string(),
            }],
            payload: json!({
                "method": "POST",
                "path": "/login",
                "operation_id": "login",
                "summary": "Authenticate a user."
            }),
        }
    }

    fn api_missing_docs_diagnostic(endpoint: &Entity) -> Diagnostic {
        Diagnostic {
            id: athanor_domain::DiagnosticId("diag_missing_docs".to_string()),
            kind: DiagnosticKind::ApiEndpointImplementedButNotDocumented,
            severity: Severity::Medium,
            status: DiagnosticStatus::Open,
            title: "Implemented API endpoint has no linked documentation".to_string(),
            message: "The implemented OpenAPI operation is not linked to documentation."
                .to_string(),
            entities: vec![endpoint.id.clone()],
            evidence: vec![athanor_domain::Evidence {
                source_file: Some("openapi.yaml".to_string()),
                line_start: Some(10),
                line_end: Some(20),
                extractor: Some("api-consistency".to_string()),
                commit_hash: None,
                confidence: 1.0,
                status: athanor_domain::EvidenceStatus::Missing,
            }],
            ownership: endpoint.ownership.clone(),
            snapshot: athanor_domain::SnapshotId("snap_current".to_string()),
            suggested_fix: Some("Add an API documentation heading.".to_string()),
            payload: json!({
                "endpoint": endpoint.stable_key.0,
                "missing_relation": "documents_api_or_operation"
            }),
        }
    }

    fn env_var() -> Entity {
        Entity {
            id: EntityId("ent_env_database_url".to_string()),
            stable_key: StableKey("env://DATABASE_URL".to_string()),
            kind: EntityKind::EnvVar,
            name: "DATABASE_URL".to_string(),
            title: None,
            source: Some(SourceLocation {
                path: "src/config.rs".to_string(),
                line_start: Some(12),
                line_end: Some(12),
            }),
            language: Some(LanguageCode("rust".to_string())),
            aliases: Vec::new(),
            ownership: vec![Ownership {
                source_file: "src/config.rs".to_string(),
            }],
            payload: json!({}),
        }
    }

    fn missing_env_var_diagnostic(env_var: &Entity) -> Diagnostic {
        Diagnostic {
            id: athanor_domain::DiagnosticId("diag_missing_env".to_string()),
            kind: DiagnosticKind::MissingEnvVar,
            severity: Severity::Medium,
            status: DiagnosticStatus::Open,
            title: "Environment variable is not documented".to_string(),
            message: "Environment variable is used but not documented.".to_string(),
            entities: vec![env_var.id.clone()],
            evidence: vec![athanor_domain::Evidence {
                source_file: Some("src/config.rs".to_string()),
                line_start: Some(12),
                line_end: Some(12),
                extractor: Some("env-docs".to_string()),
                commit_hash: None,
                confidence: 1.0,
                status: athanor_domain::EvidenceStatus::Missing,
            }],
            ownership: env_var.ownership.clone(),
            snapshot: athanor_domain::SnapshotId("snap_current".to_string()),
            suggested_fix: Some("Document the environment variable.".to_string()),
            payload: json!({
                "env_var": env_var.stable_key.0,
                "missing_relation": "documents"
            }),
        }
    }
}
