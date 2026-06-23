use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::store::init_store;
use anyhow::{Context, Result};
use athanor_core::CanonicalSnapshotStore;
use athanor_domain::{
    Diagnostic, DiagnosticKind, DiagnosticStatus, Entity, EntityKind, Relation, RelationKind,
    Severity,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::check::{DiagnosticScope, diagnostic_matches_scope};
use crate::project_path::normalize_canonical_path;

pub const DOCS_CHECK_SCHEMA: &str = "athanor.docs_check.v1";
pub const DOCS_DRIFT_SCHEMA: &str = "athanor.docs_drift.v1";
pub const DOCS_PATCH_SCHEMA: &str = "athanor.docs_patch.v1";
const API_DOC_MANAGED_START_PREFIX: &str = "<!-- athanor:api-doc:start ";
const API_DOC_MANAGED_END: &str = "<!-- athanor:api-doc:end -->";
const API_DOC_COORDINATION_START_PREFIX: &str = "<!-- athanor:api-docs-coordination:start ";
const API_DOC_COORDINATION_END: &str = "<!-- athanor:api-docs-coordination:end -->";
const API_DOC_NARRATIVE_REVIEW_START: &str = "<!-- athanor:api-narrative-review:start -->";
const API_DOC_NARRATIVE_REVIEW_END: &str = "<!-- athanor:api-narrative-review:end -->";

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
        &canonical.relations,
        &canonical.diagnostics,
        &config.docs,
        Some(&root),
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
            let content = if operation.changes.is_empty() {
                content.clone()
            } else {
                apply_frontmatter_changes(content, &operation.changes).with_context(|| {
                    format!(
                        "failed to update frontmatter in proposed {}",
                        operation.path
                    )
                })?
            };
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
                && diagnostic.payload.get("scope").and_then(Value::as_str) != Some("scripts")
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
    relations: &[Relation],
    diagnostics: &[Diagnostic],
    config: &DocsConfig,
    project_root: Option<&Path>,
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
                content: Some(api_doc_content(
                    &snapshot, endpoint, diagnostic, &path, entities, relations,
                )),
                changes: Vec::new(),
            },
        );
    }

    if let Some(project_root) = project_root {
        for endpoint in entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::ApiEndpoint)
        {
            let documented_pages =
                documented_api_pages(endpoint, &pages_by_path, entities, relations);
            let has_split_documentation = documented_pages.len() > 1;
            for page in &documented_pages {
                let Some(path) = page_path(page) else {
                    continue;
                };
                if changes_by_path
                    .get(&path)
                    .is_some_and(|operation| operation.create)
                {
                    continue;
                }
                if let Some(change) = explicit_api_entity_reference_change(page, endpoint) {
                    push_patch_change(&mut changes_by_path, &path, page, change);
                }
                let Ok(source_path) = safe_project_path(project_root, &path) else {
                    continue;
                };
                let Ok(existing) = fs::read_to_string(source_path) else {
                    continue;
                };
                let base_content = changes_by_path
                    .get(&path)
                    .and_then(|operation| operation.content.as_deref())
                    .unwrap_or(existing.as_str());
                let mut updated =
                    upsert_api_doc_managed_section(base_content, endpoint, entities, relations);
                if has_split_documentation {
                    updated =
                        upsert_api_docs_coordination_section(&updated, endpoint, &documented_pages);
                }
                if updated == base_content {
                    continue;
                }
                let operation =
                    changes_by_path
                        .entry(path.clone())
                        .or_insert_with(|| DocsPatchOperation {
                            path: path.clone(),
                            stable_key: page.stable_key.0.clone(),
                            create: false,
                            content: None,
                            changes: Vec::new(),
                        });
                operation.content = Some(updated);
            }
        }

        let endpoints = entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::ApiEndpoint)
            .collect::<Vec<_>>();
        for page in &pages {
            if !is_api_documentation_page(page) {
                continue;
            }
            let Some(path) = page_path(page) else {
                continue;
            };
            if changes_by_path
                .get(&path)
                .is_some_and(|operation| operation.create)
            {
                continue;
            }
            let page_endpoints = endpoints
                .iter()
                .copied()
                .filter(|endpoint| {
                    documented_api_pages(endpoint, &pages_by_path, entities, relations)
                        .iter()
                        .any(|documented_page| documented_page.id == page.id)
                })
                .collect::<Vec<_>>();
            if page_endpoints.is_empty() {
                continue;
            }
            let Ok(source_path) = safe_project_path(project_root, &path) else {
                continue;
            };
            let Ok(existing) = fs::read_to_string(source_path) else {
                continue;
            };
            let base_content = changes_by_path
                .get(&path)
                .and_then(|operation| operation.content.as_deref())
                .unwrap_or(existing.as_str());
            let stale_mentions = stale_api_route_mentions(base_content, &page_endpoints);
            if stale_mentions.is_empty() {
                continue;
            }
            let updated =
                upsert_api_narrative_review_section(base_content, &page_endpoints, &stale_mentions);
            if updated == base_content {
                continue;
            }
            let operation =
                changes_by_path
                    .entry(path.clone())
                    .or_insert_with(|| DocsPatchOperation {
                        path: path.clone(),
                        stable_key: page.stable_key.0.clone(),
                        create: false,
                        content: None,
                        changes: Vec::new(),
                    });
            operation.content = Some(updated);
        }
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
    entities: &[Entity],
    relations: &[Relation],
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
    let tags = string_array(&endpoint.payload["tags"]);
    let responses = string_array(&endpoint.payload["responses"]);
    let security = endpoint
        .payload
        .get("security")
        .filter(|value| !value.is_null());
    let context = api_doc_context(endpoint, entities, relations);
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
    if !tags.is_empty() {
        content.push_str(&format!("- Tags: {}\n", inline_code_list(&tags)));
    }
    if !responses.is_empty() {
        content.push_str(&format!(
            "- Declared responses: {}\n",
            inline_code_list(&responses)
        ));
    }
    if let Some(security) = security {
        content.push_str(&format!("- Security: `{}`\n", compact_json(security)));
    }
    content.push_str(&format!(
        "- Canonical entity: `{}`\n\n",
        endpoint.stable_key.0
    ));
    if let Some(handler) = &context.handler {
        content.push_str("## Implementation\n\n");
        content.push_str(&format!("- Handler: `{}`\n", handler.stable_key.0));
        if let Some(source) = &handler.source {
            content.push_str(&format!("- Source: `{}`", source.path));
            if let Some(line) = source.line_start {
                content.push_str(&format!(":{line}"));
            }
            content.push('\n');
        }
        content.push('\n');
    }
    if !context.request_schemas.is_empty() || !context.response_schemas.is_empty() {
        content.push_str("## Schemas\n\n");
        for schema in &context.request_schemas {
            content.push_str(&format!(
                "- Request: `{}` ({})\n",
                schema.schema.stable_key.0,
                schema.metadata()
            ));
        }
        for schema in &context.response_schemas {
            content.push_str(&format!(
                "- Response: `{}` ({})\n",
                schema.schema.stable_key.0,
                schema.metadata()
            ));
        }
        content.push('\n');
    }
    if !context.examples.is_empty() {
        content.push_str("## Examples\n\n");
        for example in &context.examples {
            let direction = example
                .payload
                .get("direction")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("example");
            let media_type = example
                .payload
                .get("media_type")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown media type");
            let name = example
                .payload
                .get("example_name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(example.name.as_str());
            content.push_str(&format!(
                "- `{name}`: {direction} `{media_type}` via `{}`\n",
                example.stable_key.0
            ));
        }
        content.push('\n');
    }
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

fn documented_api_pages<'a>(
    endpoint: &Entity,
    pages_by_path: &'a BTreeMap<String, &'a Entity>,
    entities: &'a [Entity],
    relations: &[Relation],
) -> Vec<&'a Entity> {
    let by_id = entities
        .iter()
        .map(|entity| (&entity.id, entity))
        .collect::<HashMap<_, _>>();
    let mut pages = BTreeMap::<String, &'a Entity>::new();

    for page in pages_by_path.values() {
        if !is_api_documentation_page(page) {
            continue;
        }
        if page.payload["entities"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .any(|stable_key| stable_key == endpoint.stable_key.0)
            && let Some(path) = page_path(page)
        {
            pages.insert(path, *page);
        }
    }

    for relation in relations.iter().filter(|relation| {
        relation.to == endpoint.id
            && matches!(
                relation.kind,
                RelationKind::Documents
                    | RelationKind::DocumentsApi
                    | RelationKind::DocumentsOperation
            )
    }) {
        let Some(document) = by_id.get(&relation.from).copied() else {
            continue;
        };
        if !matches!(
            document.kind,
            EntityKind::DocumentationPage | EntityKind::DocumentationSection
        ) {
            continue;
        }
        let Some(source_path) = document.source.as_ref().map(|source| source.path.clone()) else {
            continue;
        };
        let Some(page) = pages_by_path.get(&source_path).copied() else {
            continue;
        };
        if !is_api_documentation_page(page) {
            continue;
        }
        pages.insert(source_path, page);
    }

    pages.into_values().collect()
}

fn is_api_documentation_page(page: &Entity) -> bool {
    page.payload["documentation_kind"].as_str() == Some("api_documentation")
        || page.payload["kind"].as_str() == Some("api_documentation")
}

fn explicit_api_entity_reference_change(
    page: &Entity,
    endpoint: &Entity,
) -> Option<DocsFrontmatterChange> {
    let mut entities = page
        .payload
        .get("entities")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();
    if entities
        .iter()
        .any(|stable_key| stable_key == endpoint.stable_key.0.as_str())
    {
        return None;
    }
    entities.push(endpoint.stable_key.0.clone());
    entities.sort();
    entities.dedup();
    Some(DocsFrontmatterChange {
        field: "entities".to_string(),
        old_value: page.payload.get("entities").cloned(),
        new_value: Value::Array(entities.into_iter().map(Value::String).collect()),
        reason: format!(
            "API documentation page is linked to `{}` but does not declare it in frontmatter",
            endpoint.stable_key.0
        ),
    })
}

fn upsert_api_doc_managed_section(
    content: &str,
    endpoint: &Entity,
    entities: &[Entity],
    relations: &[Relation],
) -> String {
    let section = api_doc_managed_section(endpoint, entities, relations);
    let start_marker = api_doc_managed_start_marker(endpoint);
    let Some(start) = content.find(&start_marker) else {
        let mut updated = content.trim_end().to_string();
        updated.push_str("\n\n");
        updated.push_str(&section);
        return updated;
    };
    let Some(relative_end) = content[start..].find(API_DOC_MANAGED_END) else {
        let mut updated = content.trim_end().to_string();
        updated.push_str("\n\n");
        updated.push_str(&section);
        return updated;
    };
    let end = start + relative_end + API_DOC_MANAGED_END.len();
    let mut updated = String::new();
    updated.push_str(content[..start].trim_end());
    updated.push_str("\n\n");
    updated.push_str(&section);
    updated.push_str(content[end..].trim_start_matches(['\r', '\n']));
    updated
}

fn upsert_api_docs_coordination_section(
    content: &str,
    endpoint: &Entity,
    documented_pages: &[&Entity],
) -> String {
    let section = api_docs_coordination_section(endpoint, documented_pages);
    let start_marker = api_docs_coordination_start_marker(endpoint);
    let Some(start) = content.find(&start_marker) else {
        let mut updated = content.trim_end().to_string();
        updated.push_str("\n\n");
        updated.push_str(&section);
        return updated;
    };
    let Some(relative_end) = content[start..].find(API_DOC_COORDINATION_END) else {
        let mut updated = content.trim_end().to_string();
        updated.push_str("\n\n");
        updated.push_str(&section);
        return updated;
    };
    let end = start + relative_end + API_DOC_COORDINATION_END.len();
    let mut updated = String::new();
    updated.push_str(content[..start].trim_end());
    updated.push_str("\n\n");
    updated.push_str(&section);
    updated.push_str(content[end..].trim_start_matches(['\r', '\n']));
    updated
}

fn upsert_api_narrative_review_section(
    content: &str,
    endpoints: &[&Entity],
    stale_mentions: &[String],
) -> String {
    let rewrite_drafts = api_narrative_rewrite_drafts(content, endpoints, stale_mentions);
    let section = api_narrative_review_section(endpoints, stale_mentions, &rewrite_drafts);
    let Some(start) = content.find(API_DOC_NARRATIVE_REVIEW_START) else {
        let mut updated = content.trim_end().to_string();
        updated.push_str("\n\n");
        updated.push_str(&section);
        return updated;
    };
    let Some(relative_end) = content[start..].find(API_DOC_NARRATIVE_REVIEW_END) else {
        let mut updated = content.trim_end().to_string();
        updated.push_str("\n\n");
        updated.push_str(&section);
        return updated;
    };
    let end = start + relative_end + API_DOC_NARRATIVE_REVIEW_END.len();
    let mut updated = String::new();
    updated.push_str(content[..start].trim_end());
    updated.push_str("\n\n");
    updated.push_str(&section);
    updated.push_str(content[end..].trim_start_matches(['\r', '\n']));
    updated
}

fn api_doc_managed_start_marker(endpoint: &Entity) -> String {
    format!(
        "{API_DOC_MANAGED_START_PREFIX}{} -->",
        endpoint.stable_key.0
    )
}

fn api_docs_coordination_start_marker(endpoint: &Entity) -> String {
    format!(
        "{API_DOC_COORDINATION_START_PREFIX}{} -->",
        endpoint.stable_key.0
    )
}

fn api_docs_coordination_section(endpoint: &Entity, documented_pages: &[&Entity]) -> String {
    let mut pages = documented_pages
        .iter()
        .filter_map(|page| page_path(page).map(|path| (*page, path)))
        .collect::<Vec<_>>();
    pages.sort_by(|(left_page, left_path), (right_page, right_path)| {
        (&left_page.stable_key.0, left_path).cmp(&(&right_page.stable_key.0, right_path))
    });

    let mut content = String::new();
    content.push_str(&format!(
        "{API_DOC_COORDINATION_START_PREFIX}{} -->\n",
        endpoint.stable_key.0
    ));
    content.push('\n');
    content.push_str("## Athanor API Documentation Map\n\n");
    content.push_str(&format!("- Endpoint: `{}`\n", endpoint.stable_key.0));
    content.push_str("- Related editable API pages:\n");
    for (page, path) in pages {
        content.push_str(&format!("- `{}` at `{path}`\n", page.stable_key.0));
    }
    content.push('\n');
    content.push_str(API_DOC_COORDINATION_END);
    content.push('\n');
    content
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ApiNarrativeRewriteDraft {
    stale_mention: String,
    suggested_mention: String,
    current_line: String,
    draft_line: String,
}

fn api_narrative_review_section(
    endpoints: &[&Entity],
    stale_mentions: &[String],
    rewrite_drafts: &[ApiNarrativeRewriteDraft],
) -> String {
    let mut expected_routes = endpoints
        .iter()
        .filter_map(|endpoint| api_route_signature(endpoint))
        .collect::<Vec<_>>();
    expected_routes.sort();
    expected_routes.dedup();

    let mut stale_mentions = stale_mentions.to_vec();
    stale_mentions.sort();
    stale_mentions.dedup();

    let mut content = String::new();
    content.push_str(API_DOC_NARRATIVE_REVIEW_START);
    content.push('\n');
    content.push('\n');
    content.push_str("## Athanor API Narrative Review\n\n");
    content
        .push_str("Potentially stale API route mentions found outside Athanor-managed blocks:\n");
    for mention in stale_mentions {
        content.push_str(&format!("- `{mention}`\n"));
    }
    content.push('\n');
    content.push_str("Current linked endpoint routes for this page:\n");
    for route in expected_routes {
        content.push_str(&format!("- `{route}`\n"));
    }
    if !rewrite_drafts.is_empty() {
        content.push('\n');
        content.push_str("Suggested narrative rewrite drafts:\n");
        for draft in rewrite_drafts {
            content.push_str(&format!(
                "- Replace `{}` with `{}`\n",
                draft.stale_mention, draft.suggested_mention
            ));
            content.push_str(&format!("  - Current: {}\n", draft.current_line));
            content.push_str(&format!("  - Draft: {}\n", draft.draft_line));
        }
    }
    content.push('\n');
    content.push_str(API_DOC_NARRATIVE_REVIEW_END);
    content.push('\n');
    content
}

fn api_doc_managed_section(
    endpoint: &Entity,
    entities: &[Entity],
    relations: &[Relation],
) -> String {
    let mut content = String::new();
    content.push_str(&format!(
        "{API_DOC_MANAGED_START_PREFIX}{} -->\n",
        endpoint.stable_key.0
    ));
    content.push('\n');
    content.push_str("## Athanor API Contract\n\n");
    push_api_contract_lines(&mut content, endpoint, entities, relations);
    content.push_str(API_DOC_MANAGED_END);
    content.push('\n');
    content
}

fn push_api_contract_lines(
    content: &mut String,
    endpoint: &Entity,
    entities: &[Entity],
    relations: &[Relation],
) {
    let method = endpoint.payload["method"]
        .as_str()
        .unwrap_or("UNKNOWN")
        .to_ascii_uppercase();
    let route = endpoint
        .payload
        .get("path")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(endpoint.name.as_str());
    let operation_id = endpoint
        .payload
        .get("operation_id")
        .and_then(serde_json::Value::as_str);
    let tags = string_array(&endpoint.payload["tags"]);
    let responses = string_array(&endpoint.payload["responses"]);
    let security = endpoint
        .payload
        .get("security")
        .filter(|value| !value.is_null());
    let context = api_doc_context(endpoint, entities, relations);

    content.push_str(&format!("- Method: `{method}`\n"));
    content.push_str(&format!("- Path: `{route}`\n"));
    if let Some(operation_id) = operation_id {
        content.push_str(&format!("- Operation ID: `{operation_id}`\n"));
    }
    if !tags.is_empty() {
        content.push_str(&format!("- Tags: {}\n", inline_code_list(&tags)));
    }
    if !responses.is_empty() {
        content.push_str(&format!(
            "- Declared responses: {}\n",
            inline_code_list(&responses)
        ));
    }
    if let Some(security) = security {
        content.push_str(&format!("- Security: `{}`\n", compact_json(security)));
    }
    content.push_str(&format!(
        "- Canonical entity: `{}`\n",
        endpoint.stable_key.0
    ));
    if let Some(handler) = &context.handler {
        content.push_str(&format!("- Handler: `{}`", handler.stable_key.0));
        if let Some(source) = &handler.source {
            content.push_str(&format!(" from `{}`", source.path));
            if let Some(line) = source.line_start {
                content.push_str(&format!(":{line}"));
            }
        }
        content.push('\n');
    }
    for schema in &context.request_schemas {
        content.push_str(&format!(
            "- Request schema: `{}` ({})\n",
            schema.schema.stable_key.0,
            schema.metadata()
        ));
    }
    for schema in &context.response_schemas {
        content.push_str(&format!(
            "- Response schema: `{}` ({})\n",
            schema.schema.stable_key.0,
            schema.metadata()
        ));
    }
    for example in &context.examples {
        let direction = example
            .payload
            .get("direction")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("example");
        let media_type = example
            .payload
            .get("media_type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown media type");
        let name = example
            .payload
            .get("example_name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(example.name.as_str());
        content.push_str(&format!(
            "- Example `{name}`: {direction} `{media_type}` via `{}`\n",
            example.stable_key.0
        ));
    }
}

struct ApiDocContext<'a> {
    handler: Option<&'a Entity>,
    request_schemas: Vec<ApiSchemaDocLink<'a>>,
    response_schemas: Vec<ApiSchemaDocLink<'a>>,
    examples: Vec<&'a Entity>,
}

struct ApiSchemaDocLink<'a> {
    schema: &'a Entity,
    media_type: Option<String>,
    status_code: Option<String>,
}

impl ApiSchemaDocLink<'_> {
    fn metadata(&self) -> String {
        let mut parts = Vec::new();
        if let Some(status_code) = &self.status_code {
            parts.push(format!("status `{status_code}`"));
        }
        if let Some(media_type) = &self.media_type {
            parts.push(format!("media `{media_type}`"));
        }
        if parts.is_empty() {
            "linked schema".to_string()
        } else {
            parts.join(", ")
        }
    }
}

fn api_doc_context<'a>(
    endpoint: &Entity,
    entities: &'a [Entity],
    relations: &[Relation],
) -> ApiDocContext<'a> {
    let by_id = entities
        .iter()
        .map(|entity| (&entity.id, entity))
        .collect::<HashMap<_, _>>();

    let mut handler = None;
    let mut request_schemas = Vec::new();
    let mut response_schemas = Vec::new();
    let mut examples = Vec::new();

    for relation in relations {
        match relation.kind {
            RelationKind::ImplementedBy if relation.from == endpoint.id => {
                handler = by_id.get(&relation.to).copied();
            }
            RelationKind::SchemaForRequest if relation.from == endpoint.id => {
                if let Some(schema) = by_id.get(&relation.to).copied() {
                    request_schemas.push(ApiSchemaDocLink {
                        schema,
                        media_type: relation
                            .payload
                            .get("media_type")
                            .and_then(serde_json::Value::as_str)
                            .map(str::to_string),
                        status_code: None,
                    });
                }
            }
            RelationKind::SchemaForResponse if relation.from == endpoint.id => {
                if let Some(schema) = by_id.get(&relation.to).copied() {
                    response_schemas.push(ApiSchemaDocLink {
                        schema,
                        media_type: relation
                            .payload
                            .get("media_type")
                            .and_then(serde_json::Value::as_str)
                            .map(str::to_string),
                        status_code: relation
                            .payload
                            .get("status_code")
                            .and_then(serde_json::Value::as_str)
                            .map(str::to_string),
                    });
                }
            }
            RelationKind::ExampleFor if relation.to == endpoint.id => {
                if let Some(example) = by_id.get(&relation.from).copied() {
                    examples.push(example);
                }
            }
            _ => {}
        }
    }

    request_schemas.sort_by(|left, right| {
        (
            &left.schema.stable_key.0,
            &left.status_code,
            &left.media_type,
        )
            .cmp(&(
                &right.schema.stable_key.0,
                &right.status_code,
                &right.media_type,
            ))
    });
    response_schemas.sort_by(|left, right| {
        (
            &left.status_code,
            &left.schema.stable_key.0,
            &left.media_type,
        )
            .cmp(&(
                &right.status_code,
                &right.schema.stable_key.0,
                &right.media_type,
            ))
    });
    examples.sort_by(|left, right| left.stable_key.0.cmp(&right.stable_key.0));

    ApiDocContext {
        handler,
        request_schemas,
        response_schemas,
        examples,
    }
}

fn string_array(value: &Value) -> Vec<String> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .map(str::to_string)
        .collect()
}

fn inline_code_list(values: &[String]) -> String {
    values
        .iter()
        .map(|value| format!("`{value}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn compact_json(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| value.to_string())
}

fn stale_api_route_mentions(content: &str, endpoints: &[&Entity]) -> Vec<String> {
    let expected = endpoints
        .iter()
        .filter_map(|endpoint| api_route_signature(endpoint))
        .collect::<BTreeSet<_>>();
    if expected.is_empty() {
        return Vec::new();
    }

    let narrative = strip_generated_api_sections(strip_frontmatter_text(content));
    let mut stale = api_route_mentions(&narrative)
        .into_iter()
        .filter(|mention| !expected.contains(mention))
        .collect::<Vec<_>>();
    stale.sort();
    stale.dedup();
    stale
}

fn api_narrative_rewrite_drafts(
    content: &str,
    endpoints: &[&Entity],
    stale_mentions: &[String],
) -> Vec<ApiNarrativeRewriteDraft> {
    let expected_routes = endpoints
        .iter()
        .filter_map(|endpoint| api_route_signature(endpoint))
        .collect::<Vec<_>>();
    let suggested_mention = match expected_routes.as_slice() {
        [route] => route,
        _ => return Vec::new(),
    };
    let stale_mentions = stale_mentions.iter().collect::<BTreeSet<_>>();
    let narrative = strip_generated_api_sections(strip_frontmatter_text(content));
    let mut drafts = Vec::new();
    let mut seen = BTreeSet::new();
    for line in narrative.lines() {
        let mentions = api_route_mentions(line);
        for stale_mention in mentions
            .iter()
            .filter(|mention| stale_mentions.contains(mention))
        {
            let current_line = line.trim().to_string();
            if current_line.is_empty() {
                continue;
            }
            let draft_line = current_line.replace(stale_mention, suggested_mention);
            if draft_line == current_line {
                continue;
            }
            let key = (stale_mention.clone(), current_line.clone());
            if !seen.insert(key) {
                continue;
            }
            drafts.push(ApiNarrativeRewriteDraft {
                stale_mention: stale_mention.clone(),
                suggested_mention: suggested_mention.clone(),
                current_line,
                draft_line,
            });
        }
    }
    drafts.sort_by(|left, right| {
        (&left.stale_mention, &left.current_line).cmp(&(&right.stale_mention, &right.current_line))
    });
    drafts
}

fn api_route_signature(endpoint: &Entity) -> Option<String> {
    let method = endpoint.payload["method"].as_str()?.to_ascii_uppercase();
    let path = endpoint.payload["path"].as_str()?;
    Some(format!("{method} {path}"))
}

fn api_route_mentions(content: &str) -> Vec<String> {
    const METHODS: &[&str] = &[
        "GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS", "TRACE",
    ];
    let mut mentions = Vec::new();
    for line in content.lines() {
        let words = line
            .split_whitespace()
            .map(|word| word.trim_matches(route_token_trim_chars))
            .filter(|word| !word.is_empty())
            .collect::<Vec<_>>();
        for pair in words.windows(2) {
            let method = pair[0].to_ascii_uppercase();
            let route = pair[1].trim_end_matches(['.', ',', ';', ':']);
            if METHODS.contains(&method.as_str()) && route.starts_with('/') {
                mentions.push(format!("{method} {route}"));
            }
        }
    }
    mentions
}

fn route_token_trim_chars(character: char) -> bool {
    matches!(
        character,
        '`' | '*' | '_' | '[' | ']' | '(' | ')' | '"' | '\'' | ':' | ','
    )
}

fn strip_frontmatter_text(content: &str) -> &str {
    split_frontmatter(content).map_or(content, |(_, body)| body)
}

fn strip_generated_api_sections(content: &str) -> String {
    let mut stripped = content.to_string();
    for (start, end) in [
        (API_DOC_MANAGED_START_PREFIX, API_DOC_MANAGED_END),
        (API_DOC_COORDINATION_START_PREFIX, API_DOC_COORDINATION_END),
        (API_DOC_NARRATIVE_REVIEW_START, API_DOC_NARRATIVE_REVIEW_END),
    ] {
        stripped = strip_generated_sections_with_markers(&stripped, start, end);
    }
    stripped
}

fn strip_generated_sections_with_markers(
    content: &str,
    start_marker: &str,
    end_marker: &str,
) -> String {
    let mut remaining = content;
    let mut stripped = String::new();
    while let Some(start) = remaining.find(start_marker) {
        stripped.push_str(&remaining[..start]);
        let after_start = &remaining[start..];
        let Some(relative_end) = after_start.find(end_marker) else {
            break;
        };
        let end = start + relative_end + end_marker.len();
        remaining = &remaining[end..];
    }
    stripped.push_str(remaining);
    stripped
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
            &[],
            &DocsConfig::default(),
            None,
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
            &[],
            &[diagnostic],
            &DocsConfig::default(),
            None,
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
    fn enriches_api_documentation_draft_from_api_graph() {
        let endpoint = api_endpoint();
        let handler = handler_entity();
        let request_schema = api_schema("LoginRequest");
        let response_schema = api_schema("LoginResponse");
        let example = api_example(&endpoint);
        let diagnostic = api_missing_docs_diagnostic(&endpoint);
        let relations = vec![
            relation(
                "rel_impl",
                RelationKind::ImplementedBy,
                &endpoint,
                &handler,
                json!({}),
            ),
            relation(
                "rel_request_schema",
                RelationKind::SchemaForRequest,
                &endpoint,
                &request_schema,
                json!({"media_type": "application/json"}),
            ),
            relation(
                "rel_response_schema",
                RelationKind::SchemaForResponse,
                &endpoint,
                &response_schema,
                json!({"status_code": "200", "media_type": "application/json"}),
            ),
            relation(
                "rel_example",
                RelationKind::ExampleFor,
                &example,
                &endpoint,
                json!({}),
            ),
        ];

        let proposal = build_docs_patch_proposal(
            "snap_current".to_string(),
            &[
                endpoint.clone(),
                handler,
                request_schema,
                response_schema,
                example,
            ],
            &relations,
            &[diagnostic],
            &DocsConfig::default(),
            None,
        );

        let content = proposal.operations[0].content.as_deref().unwrap();
        assert!(content.contains("## Implementation\n\n"));
        assert!(content.contains("- Handler: `symbol://rust:auth::login`\n"));
        assert!(content.contains("## Schemas\n\n"));
        assert!(content.contains("- Request: `api-schema://openapi.yaml#LoginRequest`"));
        assert!(content.contains("- Response: `api-schema://openapi.yaml#LoginResponse`"));
        assert!(content.contains("status `200`, media `application/json`"));
        assert!(content.contains("## Examples\n\n"));
        assert!(content.contains("`success`: response `application/json`"));
    }

    #[test]
    fn proposes_existing_api_documentation_update_with_managed_block() {
        let root = temp_project_root("athanor-docs-existing-api-test");
        let doc_path = root.join("docs/api/login.md");
        fs::create_dir_all(doc_path.parent().unwrap()).unwrap();
        fs::write(
            &doc_path,
            "---\nid: doc://docs/api/login.md\nkind: api_documentation\nlanguage: en\nsource_language: en\nentities:\n  - api://POST:/login\nlast_verified_snapshot: snap_previous\nstatus: verified\n---\n\n# Login\n\nHuman-authored notes stay here.\n",
        )
        .unwrap();

        let endpoint = api_endpoint();
        let page = page(
            "docs/api/login.md",
            "editable",
            json!({
                "documentation_kind": "api_documentation",
                "frontmatter_fields": ["id", "kind", "language", "source_language", "entities", "last_verified_snapshot", "status"],
                "entities": ["api://POST:/login"],
                "last_verified_snapshot": "snap_previous",
                "status": "verified"
            }),
        );
        let handler = handler_entity();
        let relations = vec![relation(
            "rel_impl",
            RelationKind::ImplementedBy,
            &endpoint,
            &handler,
            json!({}),
        )];

        let proposal = build_docs_patch_proposal(
            "snap_current".to_string(),
            &[endpoint, page, handler],
            &relations,
            &[],
            &DocsConfig::default(),
            Some(&root),
        );

        assert_eq!(proposal.operations.len(), 1);
        let operation = &proposal.operations[0];
        assert_eq!(operation.path, "docs/api/login.md");
        assert!(!operation.create);
        let content = operation.content.as_deref().unwrap();
        assert!(content.contains("Human-authored notes stay here."));
        assert!(content.contains("<!-- athanor:api-doc:start api://POST:/login -->"));
        assert!(content.contains("## Athanor API Contract\n\n"));
        assert!(content.contains("- Handler: `symbol://rust:auth::login` from `src/auth.rs`:42"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn proposes_multiple_api_blocks_for_one_existing_api_page() {
        let root = temp_project_root("athanor-docs-multi-api-test");
        let doc_path = root.join("docs/api/auth.md");
        fs::create_dir_all(doc_path.parent().unwrap()).unwrap();
        fs::write(
            &doc_path,
            "---\nid: doc://docs/api/auth.md\nkind: api_documentation\nlanguage: en\nsource_language: en\nentities:\n  - api://POST:/login\n  - api://POST:/logout\nlast_verified_snapshot: snap_current\nstatus: verified\n---\n\n# Auth API\n",
        )
        .unwrap();

        let login = api_endpoint();
        let logout = api_endpoint_with(
            "ent_endpoint_logout",
            "api://POST:/logout",
            "logout",
            "/logout",
        );
        let page = page(
            "docs/api/auth.md",
            "editable",
            json!({
                "documentation_kind": "api_documentation",
                "frontmatter_fields": ["id", "kind", "language", "source_language", "entities", "last_verified_snapshot", "status"],
                "entities": ["api://POST:/login", "api://POST:/logout"],
                "last_verified_snapshot": "snap_current",
                "status": "verified"
            }),
        );

        let proposal = build_docs_patch_proposal(
            "snap_current".to_string(),
            &[login, logout, page],
            &[],
            &[],
            &DocsConfig::default(),
            Some(&root),
        );

        assert_eq!(proposal.operations.len(), 1);
        let content = proposal.operations[0].content.as_deref().unwrap();
        assert!(content.contains("<!-- athanor:api-doc:start api://POST:/login -->"));
        assert!(content.contains("<!-- athanor:api-doc:start api://POST:/logout -->"));
        assert!(content.contains("- Path: `/login`"));
        assert!(content.contains("- Path: `/logout`"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn proposes_coordination_blocks_for_split_api_documentation() {
        let root = temp_project_root("athanor-docs-split-api-test");
        let overview_path = root.join("docs/api/auth.md");
        let operation_path = root.join("docs/api/post-login.md");
        fs::create_dir_all(overview_path.parent().unwrap()).unwrap();
        fs::write(
            &overview_path,
            "---\nid: doc://docs/api/auth.md\nkind: api_documentation\nlanguage: en\nsource_language: en\nentities:\n  - api://POST:/login\nlast_verified_snapshot: snap_current\nstatus: verified\n---\n\n# Auth API\n",
        )
        .unwrap();
        fs::write(
            &operation_path,
            "---\nid: doc://docs/api/post-login.md\nkind: api_documentation\nlanguage: en\nsource_language: en\nentities:\n  - api://POST:/login\nlast_verified_snapshot: snap_current\nstatus: verified\n---\n\n# POST /login\n",
        )
        .unwrap();

        let endpoint = api_endpoint();
        let overview = page(
            "docs/api/auth.md",
            "editable",
            json!({
                "documentation_kind": "api_documentation",
                "frontmatter_fields": ["id", "kind", "language", "source_language", "entities", "last_verified_snapshot", "status"],
                "entities": ["api://POST:/login"],
                "last_verified_snapshot": "snap_current",
                "status": "verified"
            }),
        );
        let operation = page(
            "docs/api/post-login.md",
            "editable",
            json!({
                "documentation_kind": "api_documentation",
                "frontmatter_fields": ["id", "kind", "language", "source_language", "entities", "last_verified_snapshot", "status"],
                "entities": ["api://POST:/login"],
                "last_verified_snapshot": "snap_current",
                "status": "verified"
            }),
        );

        let proposal = build_docs_patch_proposal(
            "snap_current".to_string(),
            &[endpoint, overview, operation],
            &[],
            &[],
            &DocsConfig::default(),
            Some(&root),
        );

        assert_eq!(proposal.operations.len(), 2);
        for operation in &proposal.operations {
            let content = operation.content.as_deref().unwrap();
            assert!(
                content.contains("<!-- athanor:api-docs-coordination:start api://POST:/login -->")
            );
            assert!(content.contains("## Athanor API Documentation Map\n\n"));
            assert!(content.contains("`doc://docs/api/auth.md` at `docs/api/auth.md`"));
            assert!(content.contains("`doc://docs/api/post-login.md` at `docs/api/post-login.md`"));
        }

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn proposes_narrative_review_for_stale_api_route_mentions() {
        let root = temp_project_root("athanor-docs-stale-narrative-test");
        let doc_path = root.join("docs/api/login.md");
        fs::create_dir_all(doc_path.parent().unwrap()).unwrap();
        fs::write(
            &doc_path,
            "---\nid: doc://docs/api/login.md\nkind: api_documentation\nlanguage: en\nsource_language: en\nentities:\n  - api://POST:/login\nlast_verified_snapshot: snap_current\nstatus: verified\n---\n\n# Login API\n\nClients still call GET /login during the legacy login flow.\n",
        )
        .unwrap();

        let endpoint = api_endpoint();
        let page = page(
            "docs/api/login.md",
            "editable",
            json!({
                "documentation_kind": "api_documentation",
                "frontmatter_fields": ["id", "kind", "language", "source_language", "entities", "last_verified_snapshot", "status"],
                "entities": ["api://POST:/login"],
                "last_verified_snapshot": "snap_current",
                "status": "verified"
            }),
        );

        let proposal = build_docs_patch_proposal(
            "snap_current".to_string(),
            &[endpoint, page],
            &[],
            &[],
            &DocsConfig::default(),
            Some(&root),
        );

        assert_eq!(proposal.operations.len(), 1);
        let content = proposal.operations[0].content.as_deref().unwrap();
        assert!(content.contains(API_DOC_NARRATIVE_REVIEW_START));
        assert!(content.contains("- `GET /login`"));
        assert!(content.contains("- `POST /login`"));
        assert!(content.contains("Suggested narrative rewrite drafts:\n"));
        assert!(content.contains("- Replace `GET /login` with `POST /login`\n"));
        assert!(content.contains(
            "  - Current: Clients still call GET /login during the legacy login flow.\n"
        ));
        assert!(
            content.contains(
                "  - Draft: Clients still call POST /login during the legacy login flow.\n"
            )
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn skips_narrative_rewrite_draft_when_current_route_is_ambiguous() {
        let root = temp_project_root("athanor-docs-ambiguous-narrative-test");
        let doc_path = root.join("docs/api/auth.md");
        fs::create_dir_all(doc_path.parent().unwrap()).unwrap();
        fs::write(
            &doc_path,
            "---\nid: doc://docs/api/auth.md\nkind: api_documentation\nlanguage: en\nsource_language: en\nentities:\n  - api://POST:/login\n  - api://POST:/logout\nlast_verified_snapshot: snap_current\nstatus: verified\n---\n\n# Auth API\n\nLegacy clients still call GET /session in older examples.\n",
        )
        .unwrap();

        let login = api_endpoint();
        let logout = api_endpoint_with(
            "ent_endpoint_logout",
            "api://POST:/logout",
            "logout",
            "/logout",
        );
        let page = page(
            "docs/api/auth.md",
            "editable",
            json!({
                "documentation_kind": "api_documentation",
                "frontmatter_fields": ["id", "kind", "language", "source_language", "entities", "last_verified_snapshot", "status"],
                "entities": ["api://POST:/login", "api://POST:/logout"],
                "last_verified_snapshot": "snap_current",
                "status": "verified"
            }),
        );

        let proposal = build_docs_patch_proposal(
            "snap_current".to_string(),
            &[login, logout, page],
            &[],
            &[],
            &DocsConfig::default(),
            Some(&root),
        );

        assert_eq!(proposal.operations.len(), 1);
        let content = proposal.operations[0].content.as_deref().unwrap();
        assert!(content.contains(API_DOC_NARRATIVE_REVIEW_START));
        assert!(content.contains("- `GET /session`"));
        assert!(!content.contains("Suggested narrative rewrite drafts:\n"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn does_not_insert_api_block_into_non_api_documentation_page() {
        let root = temp_project_root("athanor-docs-non-api-test");
        let doc_path = root.join("docs/overview.md");
        fs::create_dir_all(doc_path.parent().unwrap()).unwrap();
        fs::write(
            &doc_path,
            "---\nid: doc://docs/overview.md\nkind: project_overview\nlanguage: en\nsource_language: en\nentities:\n  - api://POST:/login\nlast_verified_snapshot: snap_current\nstatus: verified\n---\n\n# Overview\n",
        )
        .unwrap();

        let endpoint = api_endpoint();
        let page = page(
            "docs/overview.md",
            "editable",
            json!({
                "documentation_kind": "project_overview",
                "frontmatter_fields": ["id", "kind", "language", "source_language", "entities", "last_verified_snapshot", "status"],
                "entities": ["api://POST:/login"],
                "last_verified_snapshot": "snap_current",
                "status": "verified"
            }),
        );

        let proposal = build_docs_patch_proposal(
            "snap_current".to_string(),
            &[endpoint, page],
            &[],
            &[],
            &DocsConfig::default(),
            Some(&root),
        );

        assert!(proposal.operations.is_empty());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn proposes_explicit_entity_reference_for_linked_api_page() {
        let root = temp_project_root("athanor-docs-api-reference-test");
        let doc_path = root.join("docs/api/login.md");
        fs::create_dir_all(doc_path.parent().unwrap()).unwrap();
        fs::write(
            &doc_path,
            "---\nid: doc://docs/api/login.md\nkind: api_documentation\nlanguage: en\nsource_language: en\nlast_verified_snapshot: snap_current\nstatus: verified\n---\n\n# Login API\n",
        )
        .unwrap();

        let endpoint = api_endpoint();
        let page = page(
            "docs/api/login.md",
            "editable",
            json!({
                "documentation_kind": "api_documentation",
                "frontmatter_fields": ["id", "kind", "language", "source_language", "last_verified_snapshot", "status"],
                "last_verified_snapshot": "snap_current",
                "status": "verified"
            }),
        );
        let relations = vec![relation(
            "rel_doc",
            RelationKind::DocumentsApi,
            &page,
            &endpoint,
            json!({}),
        )];

        let proposal = build_docs_patch_proposal(
            "snap_current".to_string(),
            &[endpoint, page],
            &relations,
            &[],
            &DocsConfig::default(),
            Some(&root),
        );

        assert_eq!(proposal.operations.len(), 1);
        let operation = &proposal.operations[0];
        assert_eq!(operation.path, "docs/api/login.md");
        assert!(operation.changes.iter().any(|change| {
            change.field == "entities"
                && change.new_value == json!(["api://POST:/login"])
                && change.reason.contains("does not declare it")
        }));
        let content = operation.content.as_deref().unwrap();
        assert!(content.contains("<!-- athanor:api-doc:start api://POST:/login -->"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn proposes_environment_documentation_creation_for_missing_env_docs() {
        let env_var = env_var();
        let diagnostic = missing_env_var_diagnostic(&env_var);

        let proposal = build_docs_patch_proposal(
            "snap_current".to_string(),
            &[env_var],
            &[],
            &[diagnostic],
            &DocsConfig::default(),
            None,
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
        api_endpoint_with("ent_endpoint", "api://POST:/login", "login", "/login")
    }

    fn api_endpoint_with(id: &str, stable_key: &str, operation_id: &str, path: &str) -> Entity {
        Entity {
            id: EntityId(id.to_string()),
            stable_key: StableKey(stable_key.to_string()),
            kind: EntityKind::ApiEndpoint,
            name: format!("POST {path}"),
            title: Some(operation_id.to_string()),
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
                "path": path,
                "operation_id": operation_id,
                "summary": format!("{} endpoint.", operation_id)
            }),
        }
    }

    fn handler_entity() -> Entity {
        Entity {
            id: EntityId("ent_handler_login".to_string()),
            stable_key: StableKey("symbol://rust:auth::login".to_string()),
            kind: EntityKind::Function,
            name: "login".to_string(),
            title: None,
            source: Some(SourceLocation {
                path: "src/auth.rs".to_string(),
                line_start: Some(42),
                line_end: Some(52),
            }),
            language: Some(LanguageCode("rust".to_string())),
            aliases: Vec::new(),
            ownership: vec![Ownership {
                source_file: "src/auth.rs".to_string(),
            }],
            payload: json!({}),
        }
    }

    fn api_schema(name: &str) -> Entity {
        Entity {
            id: EntityId(format!("ent_schema_{name}")),
            stable_key: StableKey(format!("api-schema://openapi.yaml#{name}")),
            kind: EntityKind::ApiSchema,
            name: name.to_string(),
            title: None,
            source: Some(SourceLocation {
                path: "openapi.yaml".to_string(),
                line_start: Some(80),
                line_end: Some(90),
            }),
            language: Some(LanguageCode("openapi".to_string())),
            aliases: Vec::new(),
            ownership: vec![Ownership {
                source_file: "openapi.yaml".to_string(),
            }],
            payload: json!({}),
        }
    }

    fn api_example(endpoint: &Entity) -> Entity {
        Entity {
            id: EntityId("ent_example_login_success".to_string()),
            stable_key: StableKey("api-example://openapi.yaml#success".to_string()),
            kind: EntityKind::ApiExample,
            name: "login response application/json success".to_string(),
            title: None,
            source: Some(SourceLocation {
                path: "openapi.yaml".to_string(),
                line_start: Some(100),
                line_end: Some(110),
            }),
            language: Some(LanguageCode("openapi".to_string())),
            aliases: Vec::new(),
            ownership: vec![Ownership {
                source_file: "openapi.yaml".to_string(),
            }],
            payload: json!({
                "endpoint": endpoint.stable_key.0,
                "direction": "response",
                "status_code": "200",
                "media_type": "application/json",
                "example_name": "success"
            }),
        }
    }

    fn relation(
        id: &str,
        kind: RelationKind,
        from: &Entity,
        to: &Entity,
        payload: serde_json::Value,
    ) -> Relation {
        Relation {
            id: athanor_domain::RelationId(id.to_string()),
            kind,
            from: from.id.clone(),
            to: to.id.clone(),
            status: athanor_domain::RelationStatus::Verified,
            confidence: 1.0,
            evidence: Vec::new(),
            ownership: Vec::new(),
            snapshot: athanor_domain::SnapshotId("snap_current".to_string()),
            payload,
        }
    }

    fn temp_project_root(prefix: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "{prefix}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
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
