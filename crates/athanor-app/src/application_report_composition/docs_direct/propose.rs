use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};

use crate::composition::RuntimeComposition;
use crate::docs::{
    DocsProposeFixOptions, DocsProposeFixReport, build_docs_patch_proposal_from_snapshot,
};

use super::snapshot;

pub(super) async fn propose(
    options: DocsProposeFixOptions,
    composition: &RuntimeComposition,
) -> Result<DocsProposeFixReport> {
    let root = snapshot::canonical_root(&options.root)?;
    let (canonical, config) = snapshot::load(&root, composition).await?;
    let proposal = build_docs_patch_proposal_from_snapshot(
        snapshot::id(&canonical),
        &canonical.entities,
        &canonical.relations,
        &canonical.diagnostics,
        &config.docs,
        Some(&root),
    );
    let path = match options.output {
        Some(output) => resolve_project_output(&root, &output)?,
        None => allocate_docs_patch_path(&root, &proposal.id),
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

fn allocate_docs_patch_path(root: &Path, id: &str) -> PathBuf {
    let dir = root.join(".athanor/patches/docs");
    for index in 0.. {
        let suffix = if index == 0 {
            String::new()
        } else {
            format!("_{index}")
        };
        let path = dir.join(format!("{id}{suffix}.json"));
        if !path.exists() {
            return path;
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
