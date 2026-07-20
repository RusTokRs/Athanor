use anyhow::{Context, Result};
use std::fs;

use crate::composition::RuntimeComposition;
use crate::docs::frontmatter::{allocate_docs_patch_path, resolve_project_output};
use crate::docs::{
    DocsProposeFixOptions, DocsProposeFixReport, build_docs_patch_proposal_from_snapshot,
};

use super::snapshot;

pub(crate) async fn propose(
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
