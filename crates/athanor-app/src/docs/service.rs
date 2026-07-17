use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use athanor_core::CanonicalSnapshotStore;

use crate::config::{ProjectConfig, load_config};
use crate::project_path::normalize_canonical_path;
use crate::store::init_store;

use super::check::{build_docs_check_report, build_docs_drift_report};
use super::frontmatter::{
    allocate_docs_patch_path, apply_frontmatter_changes, resolve_docs_patch_path,
    resolve_project_output, safe_project_path,
};
use super::proposal::build_docs_patch_proposal;
use super::{
    DOCS_PATCH_SCHEMA, DocsApplyPatchOptions, DocsApplyPatchReport, DocsCheckOptions,
    DocsCheckReport, DocsDriftOptions, DocsDriftReport, DocsPatchProposal,
    DocsProposeFixOptions, DocsProposeFixReport,
};

pub async fn check_docs(options: DocsCheckOptions) -> Result<DocsCheckReport> {
    let (canonical, config) = load_docs_snapshot(&options.root).await?;
    let snapshot = snapshot_id(&canonical);
    Ok(build_docs_check_report(
        snapshot,
        &canonical.entities,
        &canonical.diagnostics,
        &config.docs,
    ))
}

pub async fn docs_drift(options: DocsDriftOptions) -> Result<DocsDriftReport> {
    let (canonical, config) = load_docs_snapshot(&options.root).await?;
    Ok(build_docs_drift_report(
        snapshot_id(&canonical),
        None,
        &canonical.entities,
        &config.docs,
    ))
}

pub async fn docs_propose_fix(options: DocsProposeFixOptions) -> Result<DocsProposeFixReport> {
    let root = canonical_root(&options.root)?;
    let (canonical, config) = load_docs_snapshot(&root).await?;
    let proposal = build_docs_patch_proposal(
        snapshot_id(&canonical),
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

    let (canonical, _) = load_docs_snapshot(&root).await?;
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

fn canonical_root(root: &Path) -> Result<std::path::PathBuf> {
    Ok(normalize_canonical_path(root.canonicalize().with_context(
        || format!("failed to canonicalize {}", root.display()),
    )?))
}

async fn load_docs_snapshot(
    root: &Path,
) -> Result<(athanor_core::CanonicalSnapshot, ProjectConfig)> {
    let root = canonical_root(root)?;
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

fn snapshot_id(canonical: &athanor_core::CanonicalSnapshot) -> String {
    canonical
        .snapshot
        .as_ref()
        .map_or_else(|| "unknown".to_string(), |snapshot| snapshot.0.clone())
}
