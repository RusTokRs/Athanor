use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use athanor_core::{CoreResult, SourceFile, SourceProvider};
use athanor_domain::{Diagnostic, RepoId, SnapshotBase, SnapshotId};
use serde::Serialize;

use crate::config::load_config;
use crate::index::repo_id_for_root;
use crate::index_state::{AffectedFileSet, IndexStateStore};
use crate::local_source::{discover_source_files, read_source_file_at};
use crate::project_path::{normalize_canonical_path, normalize_requested_file};
use crate::transient_store::TransientKnowledgeStore;
use crate::{IndexPipelineMetrics, RuntimeBuilder, RuntimeComposition};

pub const CHANGED_VALIDATION_SCHEMA: &str = "athanor.changed_validation.v1";

#[derive(Debug, Clone)]
pub struct ChangedValidationOptions {
    pub root: PathBuf,
    pub files: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChangedValidationReport {
    pub schema: &'static str,
    pub root: PathBuf,
    pub snapshot: String,
    pub files_checked: usize,
    pub changed_files: usize,
    pub removed_files: usize,
    pub diagnostics: Vec<Diagnostic>,
    pub metrics: IndexPipelineMetrics,
}

pub async fn validate_changed_with_composition(
    options: ChangedValidationOptions,
    composition: &RuntimeComposition,
) -> Result<ChangedValidationReport> {
    validate_changed_inner(options, composition).await
}

async fn validate_changed_inner(
    options: ChangedValidationOptions,
    composition: &RuntimeComposition,
) -> Result<ChangedValidationReport> {
    let root = normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );
    let state_store = IndexStateStore::new(root.join(".athanor/state/index-state.json"));
    let previous_state = state_store.load().context("failed to load index state")?;
    let changed = if options.files.is_empty() {
        match changed_paths_from_git(&root).context("failed to inspect git changes")? {
            Some(changed) => changed,
            None => changed_paths_from_index_state(&root, &previous_state)?,
        }
    } else {
        changed_paths_from_requested_files(&root, &options.files)?
    };
    let mut files = Vec::new();
    for path in &changed.changed {
        if let Some(source) = read_source_file_at(&root, Path::new(path))
            .with_context(|| format!("failed to read changed file {path}"))?
        {
            files.push(source);
        }
    }
    let config = load_config(&root)?;
    let pipeline = RuntimeBuilder::from_composition(&root, composition)
        .with_adapters_config(&config.adapters)
        .with_discovered_plugins()
        .context("failed to discover adapter plugins")?
        .build_extraction_pipeline(
            Box::new(SelectedFilesSource { files }),
            TransientKnowledgeStore::new(),
        );
    let output = pipeline
        .run_extraction_only(
            RepoId(repo_id_for_root(&root)),
            SnapshotBase {
                branch: None,
                commit: None,
                parent_snapshot: previous_state.snapshot.clone().map(SnapshotId),
                working_tree: true,
            },
            AffectedFileSet {
                changed: changed.changed,
                unchanged: BTreeSet::new(),
                removed: changed.removed,
            },
        )
        .await?;

    Ok(ChangedValidationReport {
        schema: CHANGED_VALIDATION_SCHEMA,
        root,
        snapshot: output.snapshot.0,
        files_checked: output.files.len(),
        changed_files: output.affected_files.changed.len(),
        removed_files: output.affected_files.removed.len(),
        diagnostics: output.diagnostics,
        metrics: output.metrics,
    })
}

#[derive(Debug, Clone)]
struct ChangedPaths {
    changed: BTreeSet<String>,
    removed: BTreeSet<String>,
}

#[derive(Debug, Clone)]
struct SelectedFilesSource {
    files: Vec<SourceFile>,
}

#[async_trait::async_trait]
impl SourceProvider for SelectedFilesSource {
    fn name(&self) -> &str {
        "selected-files"
    }

    async fn discover(&self) -> CoreResult<Vec<SourceFile>> {
        Ok(self.files.clone())
    }
}

fn changed_paths_from_git(root: &Path) -> Result<Option<ChangedPaths>> {
    let prefix = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--show-prefix"])
        .output();
    let Ok(prefix) = prefix else {
        return Ok(None);
    };
    if !prefix.status.success() {
        return Ok(None);
    }
    let mut prefix = prefix.stdout;
    if prefix.last() == Some(&b'\n') {
        prefix.pop();
    }

    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("status")
        .arg("--porcelain=v1")
        .arg("-z")
        .arg("--untracked-files=all")
        .arg("--")
        .arg(".")
        .output();
    let Ok(output) = output else {
        return Ok(None);
    };
    if !output.status.success() {
        return Ok(None);
    }

    let paths = parse_git_status_z(&output.stdout, &prefix)?;
    normalize_changed_paths(root, paths).map(Some)
}

fn changed_paths_from_index_state(root: &Path, state: &crate::IndexState) -> Result<ChangedPaths> {
    let current = discover_source_files(root)?;
    let affected = state.affected_files(&current);
    normalize_changed_paths(
        root,
        ChangedPaths {
            changed: affected.changed,
            removed: affected.removed,
        },
    )
}

fn changed_paths_from_requested_files(root: &Path, files: &[PathBuf]) -> Result<ChangedPaths> {
    let mut changed = BTreeSet::new();
    let mut removed = BTreeSet::new();

    for file in files {
        let path = normalize_requested_file(root, file)?;
        if root.join(&path).is_file() {
            changed.insert(path);
        } else {
            removed.insert(path);
        }
    }

    Ok(ChangedPaths { changed, removed })
}

fn parse_git_status_z(output: &[u8], prefix: &[u8]) -> Result<ChangedPaths> {
    let mut changed = BTreeSet::new();
    let mut removed = BTreeSet::new();
    let mut entries = output
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty());

    while let Some(entry) = entries.next() {
        if entry.len() < 4 {
            continue;
        }
        let x = entry[0] as char;
        let y = entry[1] as char;
        let renamed = x == 'R' || y == 'R';
        let copied = x == 'C' || y == 'C';
        let source = if renamed || copied {
            entries.next()
        } else {
            None
        };
        if let Some(path) = project_git_path(&entry[3..], prefix)? {
            if !path.starts_with(".athanor/") {
                if x == 'D' || y == 'D' {
                    removed.insert(path);
                } else {
                    changed.insert(path);
                }
            }
        }
        if renamed
            && let Some(source) = source
            && let Some(source) = project_git_path(source, prefix)?
            && !source.starts_with(".athanor/")
        {
            removed.insert(source);
        }
    }

    Ok(ChangedPaths { changed, removed })
}

fn project_git_path(path: &[u8], prefix: &[u8]) -> Result<Option<String>> {
    let Some(path) = path.strip_prefix(prefix).filter(|path| !path.is_empty()) else {
        return Ok(None);
    };
    let path = std::str::from_utf8(path).context("Git reported a non-UTF-8 project path")?;
    Ok(Some(path.to_string()))
}

fn normalize_changed_paths(root: &Path, paths: ChangedPaths) -> Result<ChangedPaths> {
    let normalize = |path: String| normalize_requested_file(root, Path::new(&path));
    Ok(ChangedPaths {
        changed: paths
            .changed
            .into_iter()
            .map(normalize)
            .collect::<Result<_>>()?,
        removed: paths
            .removed
            .into_iter()
            .map(normalize)
            .collect::<Result<_>>()?,
    })
}

#[cfg(test)]
mod tests;
