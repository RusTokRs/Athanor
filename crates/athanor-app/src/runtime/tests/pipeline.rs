use std::fs;

use athanor_domain::{EntityKind, RelationKind, RepoId, SnapshotBase};

use super::fixtures::{empty_array_command, empty_output_command, source_output_command};
use super::super::*;
use crate::transient_store::TransientKnowledgeStore;

#[tokio::test]
async fn builds_builtin_index_pipeline() {
    let root = temp_root("runtime-builtins");
    fs::create_dir_all(root.join("docs")).unwrap();
    fs::write(root.join("docs/runtime.md"), "# Runtime\n\n## Registry\n").unwrap();

    let output = RuntimeBuilder::new(&root)
        .build_index_pipeline(TransientKnowledgeStore::new())
        .run(
            RepoId("repo_runtime_test".to_string()),
            SnapshotBase {
                branch: None,
                commit: None,
                parent_snapshot: None,
                working_tree: true,
            },
        )
        .await
        .unwrap();

    assert_eq!(output.files.len(), 1);
    assert!(output.entities.iter().any(|entity| entity.kind == EntityKind::File));
    assert!(
        output
            .relations
            .iter()
            .any(|relation| relation.kind == RelationKind::Contains)
    );
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn plugin_manifest_deduplicates_builtin_ids() {
    let root = temp_root("runtime-plugin-dedupe");
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/lib.rs"), "pub fn hello() {}\n").unwrap();
    let manifest = AdapterPluginManifest {
        schema: ADAPTER_MANIFEST_SCHEMA.to_string(),
        name: "duplicate-builtins".to_string(),
        version: None,
        adapters: vec![AdapterPluginEntry {
            id: "builtin.extractor.file".to_string(),
            kind: AdapterPluginKind::Extractor,
            enabled: true,
            command: None,
            supports_extensions: Vec::new(),
        }],
    };

    let output = RuntimeBuilder::new(&root)
        .with_registry(AdapterRegistry::built_in().with_plugin_manifest(&manifest).unwrap())
        .build_index_pipeline(TransientKnowledgeStore::new())
        .run(
            RepoId("repo_runtime_plugin_dedupe_test".to_string()),
            SnapshotBase {
                branch: None,
                commit: None,
                parent_snapshot: None,
                working_tree: true,
            },
        )
        .await
        .unwrap();

    assert_eq!(
        output
            .facts
            .iter()
            .filter(|fact| fact.extractor == "file")
            .count(),
        1
    );
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn external_process_adapters_cover_all_four_ports() {
    let root = temp_root("runtime-process-ports");
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/lib.rs"), "pub fn hello() {}\n").unwrap();
    let manifest = AdapterPluginManifest {
        schema: ADAPTER_MANIFEST_SCHEMA.to_string(),
        name: "process-ports".to_string(),
        version: None,
        adapters: vec![
            AdapterPluginEntry {
                id: "external.source.virtual".to_string(),
                kind: AdapterPluginKind::Source,
                enabled: true,
                command: Some(source_output_command()),
                supports_extensions: Vec::new(),
            },
            AdapterPluginEntry {
                id: "external.extractor.empty".to_string(),
                kind: AdapterPluginKind::Extractor,
                enabled: true,
                command: Some(empty_output_command()),
                supports_extensions: vec!["md".to_string()],
            },
            AdapterPluginEntry {
                id: "external.linker.empty".to_string(),
                kind: AdapterPluginKind::Linker,
                enabled: true,
                command: Some(empty_array_command()),
                supports_extensions: Vec::new(),
            },
            AdapterPluginEntry {
                id: "external.checker.empty".to_string(),
                kind: AdapterPluginKind::Checker,
                enabled: true,
                command: Some(empty_array_command()),
                supports_extensions: Vec::new(),
            },
        ],
    };

    let output = AdapterRegistry::empty()
        .with_plugin_manifest(&manifest)
        .unwrap()
        .build_index_pipeline(&root, TransientKnowledgeStore::new())
        .run(
            RepoId("repo_runtime_process_ports".to_string()),
            SnapshotBase {
                branch: None,
                commit: None,
                parent_snapshot: None,
                working_tree: true,
            },
        )
        .await
        .unwrap();

    assert_eq!(output.files.len(), 1);
    assert_eq!(output.files[0].path, "virtual/readme.md");
    assert!(output.entities.is_empty());
    assert!(output.relations.is_empty());
    assert!(output.diagnostics.is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn discovers_adapter_plugin_manifests() {
    let root = temp_root("runtime-plugin-discovery");
    let manifest_path = root.join(".athanor/adapters/files.json");
    fs::create_dir_all(manifest_path.parent().unwrap()).unwrap();
    fs::write(
        &manifest_path,
        serde_json::json!({
            "schema": ADAPTER_MANIFEST_SCHEMA,
            "name": "files",
            "adapters": [{ "id": "builtin.extractor.file", "kind": "extractor" }]
        })
        .to_string(),
    )
    .unwrap();

    let plugins = discover_adapter_plugins(&root).unwrap();
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].manifest_path, manifest_path);
    assert!(plugins[0].manifest.adapters[0].enabled);
    fs::remove_dir_all(root).unwrap();
}

fn temp_root(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "athanor-{label}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}
