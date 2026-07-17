include!("docs/legacy_impl.rs");

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
