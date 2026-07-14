use std::fs;
use std::path::{Path, PathBuf};

use athanor_core::{CanonicalSnapshot, CanonicalSnapshotStore};
use athanor_store_jsonl::JsonlKnowledgeStore;
use serde::Serialize;
use serde_json::Value;

use crate::{
    CancellationToken, INDEX_REPORT_METRICS_SCHEMA, IndexOptions, VALIDATION_RESULT_SCHEMA,
    index_project_cancellable_with_composition, index_project_with_composition,
};

#[tokio::test]
async fn production_index_runtime_publishes_one_typed_generation() {
    let root = test_root("typed-runtime");
    fs::create_dir_all(root.join("src")).expect("create source directory");
    fs::write(root.join("src/lib.rs"), "pub fn typed_runtime() {}\n")
        .expect("write source file");

    let composition = crate::test_runtime::composition();
    let report = run_index(&root, &composition).await;

    let state: Value = serde_json::from_slice(
        &fs::read(root.join(".athanor/state/index-state.json")).expect("read index state"),
    )
    .expect("parse index state");
    let manifest: Value = serde_json::from_slice(
        &fs::read(root.join(".athanor/generated/current/jsonl/manifest.json"))
            .expect("read read-model manifest"),
    )
    .expect("parse read-model manifest");
    assert_eq!(state["snapshot"], report.snapshot);
    assert_eq!(manifest["snapshot"], report.snapshot);
    assert!(!publication_journal(&root).exists());

    let store = JsonlKnowledgeStore::new(root.join(".athanor/store/canonical/jsonl"));
    let latest = store
        .load_latest_snapshot()
        .await
        .expect("load latest canonical snapshot")
        .expect("canonical snapshot exists");
    assert_eq!(
        latest.snapshot.as_ref().map(|snapshot| snapshot.0.as_str()),
        Some(report.snapshot.as_str())
    );

    let second = run_index(&root, &composition).await;
    assert_eq!(second.snapshot, report.snapshot);
    assert_eq!(second.changed_files, 0);
    assert_eq!(second.removed_files, 0);
    assert!(!publication_journal(&root).exists());

    fs::remove_dir_all(root).expect("remove typed runtime test root");
}

#[tokio::test]
async fn cancelled_index_does_not_publish_snapshot_state_or_read_model() {
    let root = test_root("cancelled");
    fs::create_dir_all(root.join("src")).expect("create source directory");
    fs::write(root.join("src/lib.rs"), "pub fn cancelled() {}\n")
        .expect("write source file");
    let composition = crate::test_runtime::composition();
    let cancellation = CancellationToken::new();
    cancellation.cancel();

    let error = index_project_cancellable_with_composition(
        IndexOptions {
            root: root.clone(),
            validation_report: None,
            validation_result: None,
            validate_only: false,
        },
        cancellation,
        &composition,
    )
    .await
    .expect_err("cancelled index must fail");

    assert!(error.to_string().contains("failed to run index pipeline"));
    assert!(
        error
            .chain()
            .any(|cause| cause.to_string() == "operation cancelled")
    );
    assert!(!root.join(".athanor/state/index-state.json").exists());
    assert!(!root.join(".athanor/generated/current/jsonl").exists());
    assert!(!publication_journal(&root).exists());

    fs::remove_dir_all(root).expect("remove cancelled runtime test root");
}

#[tokio::test]
async fn incremental_runtime_updates_and_removes_only_changed_sources() {
    let root = test_root("incremental");
    fs::create_dir_all(root.join("src")).expect("create source directory");
    fs::create_dir_all(root.join("docs")).expect("create docs directory");
    fs::write(root.join("src/lib.rs"), "pub fn hello() {}\n").expect("write Rust source");
    fs::write(root.join("docs/auth.md"), "# Auth\n\n## Login\n")
        .expect("write Markdown source");
    let composition = crate::test_runtime::composition();

    let first = run_index(&root, &composition).await;
    assert_eq!(first.files_indexed, 2);
    assert_eq!(first.changed_files, 2);
    assert_eq!(first.unchanged_files, 0);
    assert_eq!(first.removed_files, 0);
    assert_eq!(first.metrics.schema, INDEX_REPORT_METRICS_SCHEMA);
    assert_eq!(first.metrics.pipeline.schema, crate::INDEX_METRICS_SCHEMA);
    assert_eq!(first.metrics.pipeline.files_discovered, 2);
    assert_eq!(first.metrics.pipeline.files_to_extract, 2);
    assert!(first.output_dir.join("entities.jsonl").is_file());
    assert!(first.output_dir.join("facts.jsonl").is_file());
    assert!(first.output_dir.join("relations.jsonl").is_file());
    assert!(first.output_dir.join("diagnostics.jsonl").is_file());
    assert!(first.output_dir.join("manifest.json").is_file());
    assert!(
        fs::read_to_string(first.output_dir.join("entities.jsonl"))
            .expect("read first entities")
            .contains("file://src/lib.rs")
    );
    assert!(
        fs::read_to_string(first.output_dir.join("facts.jsonl"))
            .expect("read first facts")
            .contains("file_discovered")
    );
    assert!(
        fs::read_to_string(first.output_dir.join("relations.jsonl"))
            .expect("read first relations")
            .contains("contains")
    );

    let second = run_index(&root, &composition).await;
    assert_eq!(second.snapshot, first.snapshot);
    assert_eq!(second.changed_files, 0);
    assert_eq!(second.unchanged_files, 2);
    assert_eq!(second.removed_files, 0);
    assert_eq!(second.metrics.pipeline.files_to_extract, 0);

    fs::write(root.join("docs/auth.md"), "# Auth\n").expect("update Markdown source");
    let third = run_index(&root, &composition).await;
    assert_eq!(third.changed_files, 1);
    assert_eq!(third.unchanged_files, 1);
    assert_eq!(third.removed_files, 0);
    let third_entities = fs::read_to_string(third.output_dir.join("entities.jsonl"))
        .expect("read updated entities");
    assert!(third_entities.contains("doc://docs/auth.md"));
    assert!(!third_entities.contains("doc://docs/auth.md#login"));

    fs::remove_file(root.join("src/lib.rs")).expect("remove Rust source");
    let fourth = run_index(&root, &composition).await;
    assert_eq!(fourth.files_indexed, 1);
    assert_eq!(fourth.changed_files, 0);
    assert_eq!(fourth.unchanged_files, 1);
    assert_eq!(fourth.removed_files, 1);
    assert_eq!(fourth.metrics.pipeline.files_to_extract, 0);
    let fourth_entities = fs::read_to_string(fourth.output_dir.join("entities.jsonl"))
        .expect("read removal entities");
    assert!(!fourth_entities.contains("file://src/lib.rs"));

    fs::write(root.join("docs/new.md"), "# New\n").expect("write new Markdown source");
    let fifth = run_index(&root, &composition).await;
    assert_eq!(fifth.files_indexed, 2);
    assert_eq!(fifth.changed_files, 1);
    assert_eq!(fifth.unchanged_files, 1);
    assert_eq!(fifth.removed_files, 0);
    assert_eq!(fifth.metrics.pipeline.files_to_extract, 1);
    assert!(!publication_journal(&root).exists());

    fs::remove_dir_all(root).expect("remove incremental runtime test root");
}

#[tokio::test]
async fn runtime_refreshes_frontmatter_links_when_api_target_changes() {
    let root = test_root("frontmatter");
    fs::create_dir_all(root.join("docs")).expect("create docs directory");
    fs::write(
        root.join("docs/auth.md"),
        "---\nid: doc://product/auth\nentities:\n  - api://POST:/login\n---\n# Auth\n",
    )
    .expect("write frontmatter document");
    write_openapi_path(&root, "/login");
    let composition = crate::test_runtime::composition();

    run_index(&root, &composition).await;
    let relations = read_generated(&root, "relations.jsonl");
    let diagnostics = read_generated(&root, "diagnostics.jsonl");
    assert!(relations.contains("markdown_frontmatter_reference"));
    assert!(!diagnostics.contains("documentation_reference_unresolved"));

    write_openapi_path(&root, "/signin");
    let changed = run_index(&root, &composition).await;
    assert_eq!(changed.changed_files, 1);
    assert!(
        read_generated(&root, "diagnostics.jsonl")
            .contains("documentation_reference_unresolved")
    );

    write_openapi_path(&root, "/login");
    run_index(&root, &composition).await;
    assert!(
        !read_generated(&root, "diagnostics.jsonl")
            .contains("documentation_reference_unresolved")
    );

    fs::remove_dir_all(root).expect("remove frontmatter runtime test root");
}

#[tokio::test]
async fn incremental_runtime_matches_fresh_index_for_same_sources() {
    let incremental_root = test_root("incremental-equivalence");
    write_equivalence_fixture(&incremental_root, "# Auth\n\n## Login\n");
    let composition = crate::test_runtime::composition();

    run_index(&incremental_root, &composition).await;
    fs::write(
        incremental_root.join("docs/auth.md"),
        "# Auth\n\n## Login\n\n## Logout\n",
    )
    .expect("update incremental document");
    let incremental_report = run_index(&incremental_root, &composition).await;
    assert_eq!(incremental_report.changed_files, 1);
    assert_eq!(incremental_report.unchanged_files, 2);

    let fresh_root = test_root("fresh-equivalence");
    write_equivalence_fixture(&fresh_root, "# Auth\n\n## Login\n\n## Logout\n");
    let fresh_report = run_index(&fresh_root, &composition).await;
    assert_eq!(fresh_report.changed_files, 3);

    let incremental = load_latest_canonical_snapshot(&incremental_root).await;
    let fresh = load_latest_canonical_snapshot(&fresh_root).await;
    assert_eq!(
        normalized_snapshot_objects(&incremental.entities),
        normalized_snapshot_objects(&fresh.entities)
    );
    assert_eq!(
        normalized_snapshot_objects(&incremental.facts),
        normalized_snapshot_objects(&fresh.facts)
    );
    assert_eq!(
        normalized_snapshot_objects(&incremental.relations),
        normalized_snapshot_objects(&fresh.relations)
    );
    assert_eq!(
        normalized_snapshot_objects(&incremental.diagnostics),
        normalized_snapshot_objects(&fresh.diagnostics)
    );

    fs::remove_dir_all(incremental_root).expect("remove incremental fixture");
    fs::remove_dir_all(fresh_root).expect("remove fresh fixture");
}

#[tokio::test]
async fn validate_only_writes_result_without_publication_artifacts() {
    let root = test_root("validate-only");
    fs::create_dir_all(root.join("docs")).expect("create docs directory");
    fs::write(root.join("docs/auth.md"), "# Auth\n\n## Login\n")
        .expect("write validation source");
    let composition = crate::test_runtime::composition();

    let report = index_project_with_composition(
        IndexOptions {
            root: root.clone(),
            validation_report: None,
            validation_result: None,
            validate_only: true,
        },
        &composition,
    )
    .await
    .expect("run validate-only index");

    assert!(report.validate_only);
    assert_eq!(report.files_indexed, 1);
    assert_eq!(report.metrics.schema, INDEX_REPORT_METRICS_SCHEMA);
    assert_eq!(report.metrics.pipeline.schema, crate::INDEX_METRICS_SCHEMA);
    assert_eq!(report.metrics.pipeline.files_discovered, 1);
    assert_eq!(report.metrics.read_model_write_ms, 0);
    let validation_result = report.validation_result.expect("validation result path");
    assert_eq!(
        validation_result.canonicalize().expect("canonical result"),
        root.join(".athanor/generated/current/validation-result.json")
            .canonicalize()
            .expect("canonical default result")
    );
    let validation_json = fs::read_to_string(validation_result).expect("read validation result");
    assert!(validation_json.contains(VALIDATION_RESULT_SCHEMA));
    assert!(validation_json.contains("\"status\": \"passed\""));
    assert!(validation_json.contains("\"files_indexed\": 1"));
    assert!(!root.join(".athanor/state/index-state.json").exists());
    assert!(!root.join(".athanor/generated/current/jsonl").exists());
    assert!(!root.join(".athanor/store/canonical/jsonl/latest.json").exists());
    assert!(!publication_journal(&root).exists());

    fs::remove_dir_all(root).expect("remove validate-only runtime test root");
}

#[tokio::test]
async fn validate_only_honors_configured_result_path() {
    let root = test_root("configured-validation");
    fs::create_dir_all(root.join("docs")).expect("create docs directory");
    fs::write(root.join("docs/auth.md"), "# Auth\n\n## Login\n")
        .expect("write validation source");
    let validation_result = root.join("custom-validation-result.json");
    let composition = crate::test_runtime::composition();

    let report = index_project_with_composition(
        IndexOptions {
            root: root.clone(),
            validation_report: None,
            validation_result: Some(validation_result.clone()),
            validate_only: true,
        },
        &composition,
    )
    .await
    .expect("run configured validate-only index");

    assert_eq!(report.validation_result, Some(validation_result.clone()));
    let content = fs::read_to_string(validation_result).expect("read configured validation result");
    assert!(content.contains("\"schema\": \"athanor.validation_result.v1\""));
    assert!(content.contains("\"relations\""));
    assert!(!publication_journal(&root).exists());

    fs::remove_dir_all(root).expect("remove configured validation test root");
}

async fn run_index(root: &Path, composition: &crate::RuntimeComposition) -> crate::IndexReport {
    index_project_with_composition(
        IndexOptions {
            root: root.to_path_buf(),
            validation_report: None,
            validation_result: None,
            validate_only: false,
        },
        composition,
    )
    .await
    .expect("run production index runtime")
}

fn publication_journal(root: &Path) -> PathBuf {
    root.join(".athanor/state/index-publication.json")
}

fn read_generated(root: &Path, name: &str) -> String {
    fs::read_to_string(root.join(".athanor/generated/current/jsonl").join(name))
        .expect("read generated artifact")
}

fn write_openapi_path(root: &Path, path: &str) {
    fs::write(
        root.join("openapi.yaml"),
        format!(
            "openapi: 3.0.3\ninfo:\n  title: Test\n  version: 1.0.0\npaths:\n  {path}:\n    post:\n      operationId: login\n      responses:\n        '200':\n          description: ok\n"
        ),
    )
    .expect("write OpenAPI source");
}

fn write_equivalence_fixture(root: &Path, auth_markdown: &str) {
    fs::create_dir_all(root.join("docs")).expect("create docs directory");
    fs::create_dir_all(root.join("src")).expect("create source directory");
    fs::write(root.join("docs/auth.md"), auth_markdown).expect("write auth document");
    fs::write(root.join("src/lib.rs"), "pub fn login() {}\n").expect("write Rust source");
    write_openapi_path(root, "/login");
}

async fn load_latest_canonical_snapshot(root: &Path) -> CanonicalSnapshot {
    JsonlKnowledgeStore::new(root.join(".athanor/store/canonical/jsonl"))
        .load_latest_snapshot()
        .await
        .expect("load latest canonical snapshot")
        .expect("latest canonical snapshot exists")
}

fn normalized_snapshot_objects<T: Serialize>(items: &[T]) -> Vec<Value> {
    let mut values = items
        .iter()
        .map(|item| {
            let mut value = serde_json::to_value(item).expect("serialize snapshot object");
            normalize_snapshot_fields(&mut value);
            value
        })
        .collect::<Vec<_>>();
    values.sort_by(|left, right| {
        serde_json::to_string(left)
            .expect("serialize left object")
            .cmp(&serde_json::to_string(right).expect("serialize right object"))
    });
    values
}

fn normalize_snapshot_fields(value: &mut Value) {
    match value {
        Value::Object(object) => {
            if object.contains_key("snapshot") {
                object.insert(
                    "snapshot".to_string(),
                    Value::String("<snapshot>".to_string()),
                );
            }
            for child in object.values_mut() {
                normalize_snapshot_fields(child);
            }
        }
        Value::Array(items) => {
            for item in items {
                normalize_snapshot_fields(item);
            }
        }
        _ => {}
    }
}

fn test_root(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("current time")
        .as_nanos();
    std::env::temp_dir().join(format!("athanor-index-{label}-{nonce}"))
}
