use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use athanor_core::CanonicalSnapshotStore;
use athanor_domain::{RepoId, SnapshotBase};
use athanor_store_jsonl::JsonlKnowledgeStore;
use athanor_store_memory::MemoryKnowledgeStore;
use serde::Serialize;

use crate::project_path::normalize_canonical_path;
use crate::{
    AdapterValidationReport, IncrementalIndexContext, IndexState, IndexStateStore,
    JsonlReadModelWriter, RuntimeBuilder,
};

#[derive(Debug, Clone)]
pub struct IndexOptions {
    pub root: PathBuf,
    pub validation_report: Option<PathBuf>,
    pub validation_result: Option<PathBuf>,
    pub validate_only: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexReport {
    pub root: PathBuf,
    pub snapshot: String,
    pub files_indexed: usize,
    pub output_dir: PathBuf,
    pub changed_files: usize,
    pub unchanged_files: usize,
    pub removed_files: usize,
    pub validation_report: PathBuf,
    pub validation_result: Option<PathBuf>,
    pub validate_only: bool,
}

pub const VALIDATION_RESULT_SCHEMA: &str = "athanor.validation_result.v1";

#[derive(Debug, Clone, Serialize)]
struct ValidationResultFile<'a> {
    schema: &'static str,
    status: &'static str,
    snapshot: &'a str,
    files_indexed: usize,
    changed_files: usize,
    unchanged_files: usize,
    removed_files: usize,
    entities: usize,
    facts: usize,
    relations: usize,
    diagnostics: usize,
}

pub async fn index_project(options: IndexOptions) -> Result<IndexReport> {
    let root = normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );

    let state_store = IndexStateStore::new(root.join(".athanor/state/index-state.json"));
    let previous_state = state_store.load().context("failed to load index state")?;
    let output_dir = root.join(".athanor/generated/current/jsonl");
    let validation_report = validation_report_path(&root, options.validation_report.as_deref());
    let validation_result = validation_result_path(&root, options.validation_result.as_deref());
    let canonical_store = JsonlKnowledgeStore::new(root.join(".athanor/store/canonical/jsonl"));
    let previous_snapshot = match previous_state.snapshot.as_ref() {
        Some(snapshot) => canonical_store
            .load_snapshot(&athanor_domain::SnapshotId(snapshot.clone()))
            .await
            .context("failed to load previous canonical snapshot")?,
        None => None,
    };

    let output_result = if options.validate_only {
        RuntimeBuilder::new(&root)
            .with_discovered_plugins()
            .context("failed to discover adapter plugins")?
            .build_index_pipeline(MemoryKnowledgeStore::new())
            .run_with_incremental(
                RepoId(repo_id_for_root(&root)),
                SnapshotBase {
                    branch: None,
                    commit: None,
                    parent_snapshot: None,
                    working_tree: true,
                },
                IncrementalIndexContext {
                    previous_state: previous_state.clone(),
                    previous_snapshot,
                },
            )
            .await
    } else {
        RuntimeBuilder::new(&root)
            .with_discovered_plugins()
            .context("failed to discover adapter plugins")?
            .build_index_pipeline(canonical_store.clone())
            .run_with_incremental(
                RepoId(repo_id_for_root(&root)),
                SnapshotBase {
                    branch: None,
                    commit: None,
                    parent_snapshot: None,
                    working_tree: true,
                },
                IncrementalIndexContext {
                    previous_state: previous_state.clone(),
                    previous_snapshot,
                },
            )
            .await
    };
    let output = match output_result {
        Ok(output) => {
            remove_validation_report(&validation_report)
                .context("failed to remove stale validation report")?;
            output
        }
        Err(error) => {
            remove_validation_result(&validation_result)
                .context("failed to remove stale validation result")?;

            if let Some(report) = error.downcast_ref::<AdapterValidationReport>() {
                write_validation_report(&validation_report, report)
                    .context("failed to write validation report")?;
            }

            return Err(error).context("failed to run index pipeline");
        }
    };

    if options.validate_only {
        write_validation_result(&validation_result, &output)
            .context("failed to write validation result")?;

        return Ok(IndexReport {
            root,
            snapshot: output.snapshot.0,
            files_indexed: output.files.len(),
            output_dir,
            changed_files: output.affected_files.changed.len(),
            unchanged_files: output.affected_files.unchanged.len(),
            removed_files: output.affected_files.removed.len(),
            validation_report,
            validation_result: Some(validation_result),
            validate_only: true,
        });
    }

    remove_validation_result(&validation_result)
        .context("failed to remove stale validation result")?;

    let read_model = JsonlReadModelWriter::new(&output_dir)
        .write(&output)
        .context("failed to write JSONL read model")?;

    state_store
        .save(&IndexState::from_sources(&output.snapshot.0, &output.files))
        .context("failed to save index state")?;

    Ok(IndexReport {
        root,
        snapshot: output.snapshot.0,
        files_indexed: output.files.len(),
        output_dir: read_model.output_dir,
        changed_files: read_model.changed_files,
        unchanged_files: read_model.unchanged_files,
        removed_files: read_model.removed_files,
        validation_report,
        validation_result: None,
        validate_only: false,
    })
}

fn validation_report_path(root: &Path, configured: Option<&Path>) -> PathBuf {
    configured
        .map(Path::to_path_buf)
        .unwrap_or_else(|| root.join(".athanor/generated/current/validation-report.json"))
}

fn validation_result_path(root: &Path, configured: Option<&Path>) -> PathBuf {
    configured
        .map(Path::to_path_buf)
        .unwrap_or_else(|| root.join(".athanor/generated/current/validation-result.json"))
}

fn write_validation_report(path: &Path, report: &AdapterValidationReport) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    fs::write(path, serde_json::to_string_pretty(report)?)
        .with_context(|| format!("failed to write {}", path.display()))
}

fn write_validation_result(path: &Path, output: &crate::IndexPipelineOutput) -> Result<()> {
    let result = ValidationResultFile {
        schema: VALIDATION_RESULT_SCHEMA,
        status: "passed",
        snapshot: &output.snapshot.0,
        files_indexed: output.files.len(),
        changed_files: output.affected_files.changed.len(),
        unchanged_files: output.affected_files.unchanged.len(),
        removed_files: output.affected_files.removed.len(),
        entities: output.entities.len(),
        facts: output.facts.len(),
        relations: output.relations.len(),
        diagnostics: output.diagnostics.len(),
    };

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    fs::write(path, serde_json::to_string_pretty(&result)?)
        .with_context(|| format!("failed to write {}", path.display()))
}

fn remove_validation_report(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_file(path).with_context(|| format!("failed to remove {}", path.display()))?;
    }

    Ok(())
}

fn remove_validation_result(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_file(path).with_context(|| format!("failed to remove {}", path.display()))?;
    }

    Ok(())
}

fn repo_id_for_root(root: &Path) -> String {
    format!(
        "repo_{:016x}",
        stable_hash(root.to_string_lossy().as_bytes())
    )
}

fn stable_hash(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;

    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }

    hash
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[tokio::test]
    async fn indexes_files_to_jsonl() {
        let root = std::env::temp_dir().join(format!(
            "athanor-index-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("docs")).unwrap();
        fs::write(root.join("src/lib.rs"), "pub fn hello() {}\n").unwrap();
        fs::write(root.join("docs/auth.md"), "# Auth\n\n## Login\n").unwrap();

        let report = index_project(IndexOptions {
            root: root.clone(),
            validation_report: None,
            validation_result: None,
            validate_only: false,
        })
        .await
        .unwrap();

        assert_eq!(report.files_indexed, 2);
        assert!(report.output_dir.join("entities.jsonl").is_file());
        assert!(report.output_dir.join("facts.jsonl").is_file());
        assert!(report.output_dir.join("relations.jsonl").is_file());
        assert!(report.output_dir.join("diagnostics.jsonl").is_file());
        assert!(report.output_dir.join("manifest.json").is_file());

        let entities = fs::read_to_string(report.output_dir.join("entities.jsonl")).unwrap();
        assert!(entities.contains("file://src/lib.rs"));

        let facts = fs::read_to_string(report.output_dir.join("facts.jsonl")).unwrap();
        assert!(facts.contains("file_discovered"));

        let relations = fs::read_to_string(report.output_dir.join("relations.jsonl")).unwrap();
        assert!(relations.contains("contains"));
        assert_eq!(report.changed_files, 2);
        assert_eq!(report.unchanged_files, 0);
        assert_eq!(report.removed_files, 0);
        assert!(root.join(".athanor/state/index-state.json").is_file());

        let second_report = index_project(IndexOptions {
            root: root.clone(),
            validation_report: None,
            validation_result: None,
            validate_only: false,
        })
        .await
        .unwrap();

        assert_eq!(second_report.files_indexed, 2);
        assert_eq!(second_report.changed_files, 0);
        assert_eq!(second_report.unchanged_files, 2);
        assert_eq!(second_report.removed_files, 0);
        let second_entities =
            fs::read_to_string(second_report.output_dir.join("entities.jsonl")).unwrap();
        assert!(second_entities.contains("file://src/lib.rs"));
        assert!(second_entities.contains("doc://docs/auth.md#login"));

        fs::write(root.join("docs/auth.md"), "# Auth\n").unwrap();
        let third_report = index_project(IndexOptions {
            root: root.clone(),
            validation_report: None,
            validation_result: None,
            validate_only: false,
        })
        .await
        .unwrap();

        assert_eq!(third_report.files_indexed, 2);
        assert_eq!(third_report.changed_files, 1);
        assert_eq!(third_report.unchanged_files, 1);
        assert_eq!(third_report.removed_files, 0);
        let third_entities =
            fs::read_to_string(third_report.output_dir.join("entities.jsonl")).unwrap();
        assert!(third_entities.contains("file://src/lib.rs"));
        assert!(third_entities.contains("doc://docs/auth.md"));
        assert!(!third_entities.contains("doc://docs/auth.md#login"));

        fs::remove_file(root.join("src/lib.rs")).unwrap();
        let fourth_report = index_project(IndexOptions {
            root: root.clone(),
            validation_report: None,
            validation_result: None,
            validate_only: false,
        })
        .await
        .unwrap();

        assert_eq!(fourth_report.files_indexed, 1);
        assert_eq!(fourth_report.changed_files, 1);
        assert_eq!(fourth_report.unchanged_files, 0);
        assert_eq!(fourth_report.removed_files, 1);
        let fourth_entities =
            fs::read_to_string(fourth_report.output_dir.join("entities.jsonl")).unwrap();
        assert!(!fourth_entities.contains("file://src/lib.rs"));
        assert!(fourth_entities.contains("doc://docs/auth.md"));

        fs::write(root.join("docs/new.md"), "# New\n").unwrap();
        let fifth_report = index_project(IndexOptions {
            root: root.clone(),
            validation_report: None,
            validation_result: None,
            validate_only: false,
        })
        .await
        .unwrap();

        assert_eq!(fifth_report.files_indexed, 2);
        assert_eq!(fifth_report.changed_files, 2);
        assert_eq!(fifth_report.unchanged_files, 0);
        assert_eq!(fifth_report.removed_files, 0);

        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn refreshes_frontmatter_relations_and_diagnostics_when_only_target_changes() {
        let root = std::env::temp_dir().join(format!(
            "athanor-frontmatter-incremental-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("docs")).unwrap();
        fs::write(
            root.join("docs/auth.md"),
            "---\nid: doc://product/auth\nentities:\n  - api://POST:/login\n---\n# Auth\n",
        )
        .unwrap();
        write_openapi_path(&root, "/login");

        index_project(IndexOptions {
            root: root.clone(),
            validation_report: None,
            validation_result: None,
            validate_only: false,
        })
        .await
        .unwrap();
        let relations =
            fs::read_to_string(root.join(".athanor/generated/current/jsonl/relations.jsonl"))
                .unwrap();
        let diagnostics =
            fs::read_to_string(root.join(".athanor/generated/current/jsonl/diagnostics.jsonl"))
                .unwrap();
        assert!(relations.contains("markdown_frontmatter_reference"));
        assert!(!diagnostics.contains("documentation_reference_unresolved"));

        write_openapi_path(&root, "/signin");
        let changed = index_project(IndexOptions {
            root: root.clone(),
            validation_report: None,
            validation_result: None,
            validate_only: false,
        })
        .await
        .unwrap();
        assert_eq!(changed.changed_files, 1);
        let diagnostics =
            fs::read_to_string(root.join(".athanor/generated/current/jsonl/diagnostics.jsonl"))
                .unwrap();
        assert!(diagnostics.contains("documentation_reference_unresolved"));

        write_openapi_path(&root, "/login");
        index_project(IndexOptions {
            root: root.clone(),
            validation_report: None,
            validation_result: None,
            validate_only: false,
        })
        .await
        .unwrap();
        let diagnostics =
            fs::read_to_string(root.join(".athanor/generated/current/jsonl/diagnostics.jsonl"))
                .unwrap();
        assert!(!diagnostics.contains("documentation_reference_unresolved"));

        fs::remove_dir_all(root).unwrap();
    }

    fn write_openapi_path(root: &Path, path: &str) {
        fs::write(
            root.join("openapi.yaml"),
            format!(
                "openapi: 3.0.3\ninfo:\n  title: Test\n  version: 1.0.0\npaths:\n  {path}:\n    post:\n      operationId: login\n      responses:\n        '200':\n          description: ok\n"
            ),
        )
        .unwrap();
    }

    #[tokio::test]
    async fn validates_without_writing_outputs_or_state() {
        let root = std::env::temp_dir().join(format!(
            "athanor-validate-only-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        fs::create_dir_all(root.join("docs")).unwrap();
        fs::write(root.join("docs/auth.md"), "# Auth\n\n## Login\n").unwrap();

        let report = index_project(IndexOptions {
            root: root.clone(),
            validation_report: None,
            validation_result: None,
            validate_only: true,
        })
        .await
        .unwrap();

        assert!(report.validate_only);
        assert_eq!(report.files_indexed, 1);
        let validation_result = report.validation_result.unwrap();
        assert_eq!(
            validation_result,
            root.join(".athanor/generated/current/validation-result.json")
        );
        let validation_result_json = fs::read_to_string(validation_result).unwrap();
        assert!(validation_result_json.contains(VALIDATION_RESULT_SCHEMA));
        assert!(validation_result_json.contains("\"status\": \"passed\""));
        assert!(validation_result_json.contains("\"files_indexed\": 1"));
        assert!(!root.join(".athanor/state/index-state.json").exists());
        assert!(
            !root
                .join(".athanor/generated/current/jsonl/entities.jsonl")
                .exists()
        );
        assert!(
            !root
                .join(".athanor/store/canonical/jsonl/latest.json")
                .exists()
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn writes_configured_validation_result_for_validate_only() {
        let root = std::env::temp_dir().join(format!(
            "athanor-validation-result-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        fs::create_dir_all(root.join("docs")).unwrap();
        fs::write(root.join("docs/auth.md"), "# Auth\n\n## Login\n").unwrap();
        let validation_result = root.join("custom-validation-result.json");

        let report = index_project(IndexOptions {
            root: root.clone(),
            validation_report: None,
            validation_result: Some(validation_result.clone()),
            validate_only: true,
        })
        .await
        .unwrap();

        assert_eq!(report.validation_result, Some(validation_result.clone()));
        let content = fs::read_to_string(validation_result).unwrap();
        assert!(content.contains("\"schema\": \"athanor.validation_result.v1\""));
        assert!(content.contains("\"relations\""));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn writes_validation_report_json() {
        let root = std::env::temp_dir().join(format!(
            "athanor-validation-report-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = root.join("validation-report.json");
        let report = AdapterValidationReport {
            adapter: "test-adapter".to_string(),
            issues: vec![crate::AdapterValidationIssue {
                object_type: "relation",
                object_id: "rel_test".to_string(),
                missing: crate::MissingCanonicalMetadata::Ownership,
            }],
        };

        write_validation_report(&path, &report).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("\"adapter\": \"test-adapter\""));
        assert!(content.contains("\"missing\": \"ownership\""));

        fs::remove_dir_all(root).unwrap();
    }
}
