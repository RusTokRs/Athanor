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
use crate::local_source::read_source_file_at;
use crate::project_path::normalize_canonical_path;
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

pub async fn validate_changed(
    options: ChangedValidationOptions,
) -> Result<ChangedValidationReport> {
    validate_changed_inner(options, None).await
}

pub async fn validate_changed_with_composition(
    options: ChangedValidationOptions,
    composition: &RuntimeComposition,
) -> Result<ChangedValidationReport> {
    validate_changed_inner(options, Some(composition)).await
}

async fn validate_changed_inner(
    options: ChangedValidationOptions,
    composition: Option<&RuntimeComposition>,
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
        changed_paths_from_git(&root)
            .context("failed to inspect git changes")?
            .unwrap_or_else(|| changed_paths_from_index_state(&root, &previous_state))
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
    let builder = match composition {
        Some(composition) => RuntimeBuilder::from_composition(&root, composition),
        None => RuntimeBuilder::new(&root),
    };
    let pipeline = builder
        .allow_external_process(config.adapters.allow_external_process)
        .clear_external_process_environment(matches!(
            config.adapters.external_process_sandbox,
            crate::config::ExternalProcessSandboxProfile::CleanEnvironment
        ))
        .allowed_external_process_programs(
            config
                .adapters
                .external_process_allowlist
                .iter()
                .map(PathBuf::from),
        )
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
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("status")
        .arg("--porcelain=v1")
        .arg("-z")
        .arg("--untracked-files=all")
        .output();
    let Ok(output) = output else {
        return Ok(None);
    };
    if !output.status.success() {
        return Ok(None);
    }

    Ok(Some(parse_git_status_z(&output.stdout)))
}

fn changed_paths_from_index_state(root: &Path, state: &crate::IndexState) -> ChangedPaths {
    let mut changed = BTreeSet::new();
    let mut removed = BTreeSet::new();

    for (path, previous) in &state.files {
        let relative = Path::new(path);
        match read_source_file_at(root, relative) {
            Ok(Some(source))
                if source.content_hash == previous.content_hash
                    && source.language_hint == previous.language_hint => {}
            Ok(Some(_)) => {
                changed.insert(path.clone());
            }
            Ok(None) => {
                removed.insert(path.clone());
            }
            Err(_) => {
                changed.insert(path.clone());
            }
        }
    }

    ChangedPaths { changed, removed }
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

fn normalize_requested_file(root: &Path, file: &Path) -> Result<String> {
    let absolute = if file.is_absolute() {
        file.to_path_buf()
    } else {
        root.join(file)
    };
    let relative = absolute
        .strip_prefix(root)
        .with_context(|| format!("{} is outside {}", absolute.display(), root.display()))?;

    Ok(relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
        .replace('\\', "/"))
}

fn parse_git_status_z(output: &[u8]) -> ChangedPaths {
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
        let path = String::from_utf8_lossy(&entry[3..]).replace('\\', "/");
        if path.starts_with(".athanor/") {
            continue;
        }
        if x == 'R' || x == 'C' {
            let _ = entries.next();
        }
        if x == 'D' || y == 'D' {
            removed.insert(path);
        } else {
            changed.insert(path);
        }
    }

    ChangedPaths { changed, removed }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_git_status_z_changed_removed_and_renamed_paths() {
        let report = parse_git_status_z(
            b" M src/lib.rs\0D  docs/old.md\0?? apps/admin/app.tsx\0R  docs/new.md\0docs/old-name.md\0",
        );

        assert!(report.changed.contains("src/lib.rs"));
        assert!(report.changed.contains("apps/admin/app.tsx"));
        assert!(report.changed.contains("docs/new.md"));
        assert!(report.removed.contains("docs/old.md"));
        assert!(!report.changed.contains("docs/old-name.md"));
    }

    #[test]
    fn normalizes_requested_relative_and_absolute_files() {
        let root = std::env::current_dir().unwrap();
        let relative = PathBuf::from("docs/README.md");
        let absolute = root.join("docs/development/agent-workflow.md");

        assert_eq!(
            normalize_requested_file(&root, &relative).unwrap(),
            "docs/README.md"
        );
        assert_eq!(
            normalize_requested_file(&root, &absolute).unwrap(),
            "docs/development/agent-workflow.md"
        );
    }
}
