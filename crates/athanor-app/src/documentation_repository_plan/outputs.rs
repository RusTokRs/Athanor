//! Snapshot-only conflicts. This cannot replace live-file and byte-level
//! preflight at publication time, and grants no filesystem write permission.

use std::collections::BTreeMap;

use super::{
    DocumentationPackagePlan, DocumentationPlanIssue, DocumentationTargetPlan, FileInventory,
    foreign_root, is_internal_path, package_root, portable_fold, portable_path, source_kind,
};

pub(super) fn preflight_candidates(
    files: &FileInventory,
    packages: &mut [DocumentationPackagePlan],
    targets: &mut [DocumentationTargetPlan],
    issues: &mut Vec<DocumentationPlanIssue>,
) {
    let mut proposed = BTreeMap::<String, usize>::new();
    for path in packages
        .iter()
        .filter_map(|package| package.index_candidate.as_ref())
        .chain(
            targets
                .iter()
                .filter_map(|target| target.output_candidate.as_ref()),
        )
    {
        *proposed.entry(portable_fold(path)).or_default() += 1;
    }
    for package in packages {
        if let Some(path) = package.index_candidate.as_deref()
            && let Some((kind, detail)) = conflict(files, path, &package.manifest, &proposed)
        {
            issues.push(DocumentationPlanIssue::new(kind, path, detail));
            package.index_candidate = None;
        }
    }
    for target in targets {
        if let Some(path) = target.output_candidate.as_deref()
            && let Some((kind, detail)) = conflict(files, path, &target.manifest, &proposed)
        {
            issues.push(DocumentationPlanIssue::new(kind, path, detail));
            target.output_candidate = None;
            target.output_blocker = Some(kind.to_string());
        }
    }
    for path in &files.paths {
        if source_kind(path) == Some("ecosystem_manifest") {
            issues.push(DocumentationPlanIssue::new(
                "unsupported_ecosystem_owner",
                path,
                "this snapshot has a non-Cargo project signal, not a verified package owner",
            ));
        }
    }
}

fn conflict(
    files: &FileInventory,
    candidate: &str,
    owner_manifest: &str,
    proposed: &BTreeMap<String, usize>,
) -> Option<(&'static str, &'static str)> {
    if !portable_path(candidate) || is_internal_path(candidate) {
        return Some((
            "invalid_output_path",
            "candidate is not a portable package-local path",
        ));
    }
    if foreign_root(candidate, &files.foreign_roots)
        .is_some_and(|root| root.len() >= package_root(owner_manifest).len())
    {
        return Some((
            "foreign_output_project",
            "a non-Cargo project manifest owns or shares this output directory",
        ));
    }
    if proposed
        .get(&portable_fold(candidate))
        .copied()
        .unwrap_or(0)
        > 1
    {
        return Some((
            "duplicate_output_candidate",
            "several owners propose the same portable path",
        ));
    }
    if files.folded.contains_key(&portable_fold(candidate)) {
        return Some((
            "occupied_output",
            "snapshot already contains a file at this path or case variant",
        ));
    }
    if files.directories.contains_key(&portable_fold(candidate)) {
        return Some((
            "occupied_output_directory",
            "snapshot already contains a directory at this path or portable alias",
        ));
    }
    let mut prefix = String::new();
    let mut components = candidate.split('/').peekable();
    while let Some(component) = components.next() {
        if !prefix.is_empty() {
            prefix.push('/');
        }
        prefix.push_str(component);
        if components.peek().is_none() {
            break;
        }
        if files.folded.contains_key(&portable_fold(&prefix)) {
            return Some((
                "occupied_output_parent",
                "candidate parent is a snapshot file",
            ));
        }
        let owner_root = package_root(owner_manifest);
        if (owner_root.is_empty() || prefix.starts_with(&format!("{owner_root}/")))
            && files
                .folded
                .contains_key(&portable_fold(&format!("{prefix}/Cargo.toml")))
        {
            return Some((
                "nested_output_package",
                "candidate would enter a nested Cargo manifest directory",
            ));
        }
        // A real `Docs/` directory containing human files is not the same
        // portable output directory as `docs/`. Snapshot File and rejected
        // manifest claims only supply spelling evidence; publishers must recheck.
        if files
            .directories
            .get(&portable_fold(&prefix))
            .is_some_and(|spellings| spellings.iter().any(|spelling| spelling != &prefix))
        {
            return Some((
                "output_parent_case_collision",
                "existing directory spelling conflicts with the proposed parent",
            ));
        }
    }
    None
}
