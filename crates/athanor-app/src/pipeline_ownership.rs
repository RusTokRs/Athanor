use std::collections::BTreeSet;

use athanor_domain::{Diagnostic, DiagnosticKind, Entity, Evidence, Fact, Ownership, Relation};

pub(crate) fn entity_owned_by_any_path(entity: &Entity, paths: &BTreeSet<String>) -> bool {
    owns(&entity.ownership, paths)
        || matches_path(
            entity.source.as_ref().map(|source| source.path.as_str()),
            paths,
        )
}
pub(crate) fn fact_owned_by_any_path(fact: &Fact, paths: &BTreeSet<String>) -> bool {
    owns(&fact.ownership, paths) || evidence_matches(&fact.evidence, paths)
}
pub(crate) fn relation_owned_by_any_path(relation: &Relation, paths: &BTreeSet<String>) -> bool {
    owns(&relation.ownership, paths) || evidence_matches(&relation.evidence, paths)
}
pub(crate) fn diagnostic_owned_by_any_path(
    diagnostic: &Diagnostic,
    paths: &BTreeSet<String>,
) -> bool {
    owns(&diagnostic.ownership, paths) || evidence_matches(&diagnostic.evidence, paths)
}
pub(crate) fn diagnostic_invalidated_by_changed_path(
    diagnostic: &Diagnostic,
    changed_paths: &BTreeSet<String>,
) -> bool {
    matches!(&diagnostic.kind, DiagnosticKind::Other(kind) if kind.starts_with("rustok_ffa_"))
        && changed_paths
            .iter()
            .any(|path| is_rustok_ffa_input_path(path))
}
fn is_rustok_ffa_input_path(path: &str) -> bool {
    path == "docs/modules/registry.md"
        || path.ends_with("/docs/modules/registry.md")
        || ((path.starts_with("crates/rustok-") || path.contains("/crates/rustok-"))
            && (path.ends_with("/docs/implementation-plan.md")
                || path.contains("/admin/")
                || path.contains("/storefront/")))
        || path.starts_with("apps/admin/")
        || path.starts_with("apps/storefront/")
        || path.contains("/apps/admin/")
        || path.contains("/apps/storefront/")
}
fn owns(ownership: &[Ownership], paths: &BTreeSet<String>) -> bool {
    ownership
        .iter()
        .any(|owner| paths.contains(&owner.source_file))
}
fn evidence_matches(evidence: &[Evidence], paths: &BTreeSet<String>) -> bool {
    evidence
        .iter()
        .any(|item| matches_path(item.source_file.as_deref(), paths))
}
fn matches_path(path: Option<&str>, paths: &BTreeSet<String>) -> bool {
    path.is_some_and(|path| paths.contains(path))
}
