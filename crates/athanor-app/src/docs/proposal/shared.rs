use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use athanor_domain::Entity;

use super::super::frontmatter::safe_project_path;
use super::super::{DocsFrontmatterChange, DocsPatchOperation};

pub(super) fn push_change(
    changes: &mut BTreeMap<String, DocsPatchOperation>,
    path: &str,
    page: &Entity,
    change: DocsFrontmatterChange,
) {
    let operation = operation(changes, path, page);
    if let Some(existing) = operation
        .changes
        .iter_mut()
        .find(|existing| existing.field == change.field)
    {
        *existing = change;
    } else {
        operation.changes.push(change);
        operation.changes.sort_by(|left, right| left.field.cmp(&right.field));
    }
}

pub(super) fn operation<'a>(
    changes: &'a mut BTreeMap<String, DocsPatchOperation>,
    path: &str,
    page: &Entity,
) -> &'a mut DocsPatchOperation {
    changes.entry(path.to_string()).or_insert_with(|| DocsPatchOperation {
        path: path.to_string(),
        stable_key: page.stable_key.0.clone(),
        create: false,
        content: None,
        changes: Vec::new(),
    })
}

pub(super) fn read_project_file(root: &Path, relative: &str) -> Option<String> {
    fs::read_to_string(safe_project_path(root, relative).ok()?).ok()
}
