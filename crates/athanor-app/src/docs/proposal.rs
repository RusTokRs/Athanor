mod api;
mod operations;
mod policy;
mod shared;

use std::collections::BTreeMap;
use std::path::Path;

use athanor_domain::{Diagnostic, Entity, Relation};

use crate::config::DocsConfig;

use super::check::{is_editable_page, normalize_policy_path, page_path};
use super::frontmatter::docs_patch_id;
use super::{DOCS_PATCH_SCHEMA, DocsPatchOperation, DocsPatchProposal};

pub(crate) fn build_docs_patch_proposal_from_snapshot(
    snapshot: String,
    entities: &[Entity],
    relations: &[Relation],
    diagnostics: &[Diagnostic],
    config: &DocsConfig,
    project_root: Option<&Path>,
) -> DocsPatchProposal {
    build_docs_patch_proposal(
        snapshot,
        entities,
        relations,
        diagnostics,
        config,
        project_root,
    )
}

pub(super) fn build_docs_patch_proposal(
    snapshot: String,
    entities: &[Entity],
    relations: &[Relation],
    diagnostics: &[Diagnostic],
    config: &DocsConfig,
    project_root: Option<&Path>,
) -> DocsPatchProposal {
    let editable_path = normalize_policy_path(&config.editable_path);
    let pages = entities
        .iter()
        .filter(|entity| is_editable_page(entity, &editable_path))
        .collect::<Vec<_>>();
    let pages_by_path = pages
        .iter()
        .filter_map(|page| page_path(page).map(|path| (path, *page)))
        .collect::<BTreeMap<_, _>>();
    let mut changes = BTreeMap::<String, DocsPatchOperation>::new();

    policy::add(
        &mut changes,
        &snapshot,
        config,
        &pages_by_path,
        entities,
        diagnostics,
    );
    api::add_missing(
        &mut changes,
        &snapshot,
        config,
        &pages_by_path,
        entities,
        relations,
        diagnostics,
    );
    if let Some(root) = project_root {
        api::update_existing(
            &mut changes,
            root,
            &pages,
            &pages_by_path,
            entities,
            relations,
        );
    }
    operations::add_missing(
        &mut changes,
        &snapshot,
        config,
        &pages_by_path,
        entities,
        diagnostics,
    );

    DocsPatchProposal {
        schema: DOCS_PATCH_SCHEMA.to_string(),
        id: docs_patch_id(&snapshot),
        snapshot,
        operations: changes.into_values().collect(),
    }
}
