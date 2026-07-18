use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::Value;

use super::DocsFrontmatterChange;

pub(super) fn docs_patch_id(snapshot: &str) -> String {
    let safe_snapshot = snapshot
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    format!("docs_patch_{safe_snapshot}")
}

pub(super) fn allocate_docs_patch_path(root: &Path, id: &str) -> Result<PathBuf> {
    let dir = root.join(".athanor/patches/docs");
    for index in 0.. {
        let suffix = if index == 0 {
            String::new()
        } else {
            format!("_{index}")
        };
        let path = dir.join(format!("{id}{suffix}.json"));
        if !path.exists() {
            return Ok(path);
        }
    }
    unreachable!("unbounded docs patch path allocation should always return")
}

pub(super) fn resolve_project_output(root: &Path, output: &Path) -> Result<PathBuf> {
    if output.is_absolute() {
        return Ok(output.to_path_buf());
    }
    safe_project_path(root, output.to_string_lossy().as_ref())
}

pub(super) fn resolve_docs_patch_path(root: &Path, patch: &str) -> Result<PathBuf> {
    let path = Path::new(patch);
    if path.is_absolute() || patch.contains('/') || patch.contains('\\') || patch.ends_with(".json")
    {
        return resolve_project_output(root, path);
    }
    Ok(root
        .join(".athanor/patches/docs")
        .join(format!("{patch}.json")))
}

pub(super) fn safe_project_path(root: &Path, relative: &str) -> Result<PathBuf> {
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

pub(super) fn apply_frontmatter_changes(
    content: &str,
    changes: &[DocsFrontmatterChange],
) -> Result<String> {
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

pub(super) fn split_frontmatter(content: &str) -> Result<(Option<&str>, &str)> {
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
