use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::Value;

use crate::composition::RuntimeComposition;
use crate::docs::{
    DOCS_PATCH_SCHEMA, DocsApplyPatchOptions, DocsApplyPatchReport, DocsFrontmatterChange,
    DocsPatchProposal,
};

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
