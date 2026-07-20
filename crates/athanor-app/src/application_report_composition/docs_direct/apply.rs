use std::fs;

use crate::composition::RuntimeComposition;
use crate::docs::frontmatter::{
    apply_frontmatter_changes, resolve_docs_patch_path, safe_project_path,
};
use crate::docs::{
    DOCS_PATCH_SCHEMA, DocsApplyPatchOptions, DocsApplyPatchReport, DocsPatchProposal,
};
use anyhow::{Context, Result};

use super::snapshot;

pub(crate) async fn apply(
    options: DocsApplyPatchOptions,
    composition: &RuntimeComposition,
) -> Result<DocsApplyPatchReport> {
    let root = snapshot::canonical_root(&options.root)?;
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

    let (canonical, _) = snapshot::load(&root, composition).await?;
    let current_snapshot = snapshot::id(&canonical);
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
