use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};
use athanor_core::{CanonicalSnapshot, CanonicalSnapshotStore};
use athanor_domain::{Diagnostic, DiagnosticStatus, Entity, EntityKind, Severity};
use serde_json::Value;

use crate::check::{DiagnosticScope, diagnostic_matches_scope};
use crate::composition::RuntimeComposition;
use crate::config::{CompletenessPolicy, DocsConfig, ProjectConfig, load_config};
use crate::docs::{
    DOCS_CHECK_SCHEMA, DOCS_PATCH_SCHEMA, DocsApplyPatchOptions, DocsApplyPatchReport,
    DocsCheckOptions, DocsCheckReport, DocsDriftOptions, DocsDriftReport,
    DocsFrontmatterChange, DocsPatchProposal, DocsPolicyViolation, build_docs_drift_report,
};
use crate::project_path::normalize_canonical_path;

pub(super) async fn check(
    options: DocsCheckOptions,
    composition: &RuntimeComposition,
) -> Result<DocsCheckReport> {
    let (canonical, config) = load_snapshot(&options.root, composition).await?;
    let snapshot = snapshot_id(&canonical);
    Ok(build_check_report(
        snapshot,
        &canonical.entities,
        &canonical.diagnostics,
        &config.docs,
    ))
}

pub(super) async fn drift(
    options: DocsDriftOptions,
    composition: &RuntimeComposition,
) -> Result<DocsDriftReport> {
    let (canonical, config) = load_snapshot(&options.root, composition).await?;
    Ok(build_docs_drift_report(
        snapshot_id(&canonical),
        None,
        &canonical.entities,
        &config.docs,
    ))
}

pub(super) async fn apply(
    options: DocsApplyPatchOptions,
    composition: &RuntimeComposition,
) -> Result<DocsApplyPatchReport> {
    let root = canonical_root(&options.root)?;
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

    let (canonical, _) = load_snapshot(&root, composition).await?;
    let current_snapshot = snapshot_id(&canonical);
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

async fn load_snapshot(
    root: &Path,
    composition: &RuntimeComposition,
) -> Result<(CanonicalSnapshot, ProjectConfig)> {
    let root = canonical_root(root)?;
    let config = load_config(&root)?;
    let store = composition.init_store(&root, &config).await?;
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

fn snapshot_id(canonical: &CanonicalSnapshot) -> String {
    canonical
        .snapshot
        .as_ref()
        .map_or_else(|| "unknown".to_string(), |snapshot| snapshot.0.clone())
}

fn build_check_report(
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
        if !policy.allowed_statuses.iter().any(|allowed| allowed == status) {
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

fn is_editable_page(entity: &Entity, editable_path: &str) -> bool {
    entity.kind == EntityKind::DocumentationPage
        && entity.payload["documentation_layer"].as_str() == Some("editable")
        && entity.source.as_ref().is_some_and(|source| {
            let path = source.path.replace('\\', "/");
            path == editable_path || path.starts_with(&format!("{editable_path}/"))
        })
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

fn resolve_project_output(root: &Path, output: &Path) -> Result<PathBuf> {
    if output.is_absolute() {
        return Ok(output.to_path_buf());
    }
    safe_project_path(root, output.to_string_lossy().as_ref())
}

fn safe_project_path(root: &Path, relative: &str) -> Result<PathBuf> {
    let path = Path::new(relative);
    if path.is_absolute() {
        anyhow::bail!("project-relative path expected, got `{relative}`");
    }
    for component in path.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
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

fn canonical_root(root: &Path) -> Result<PathBuf> {
    Ok(normalize_canonical_path(root.canonicalize().with_context(
        || format!("failed to canonicalize {}", root.display()),
    )?))
}
