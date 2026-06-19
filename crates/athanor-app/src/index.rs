use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use athanor_core::{EntityQuery, KnowledgeStore, SourceFile, SourceProvider};
use athanor_domain::{
    Entity, EntityId, EntityKind, RepoId, SnapshotBase, SourceLocation, StableKey,
};
use athanor_source_fs::LocalFileSystemSource;
use athanor_store_memory::MemoryKnowledgeStore;
use serde::Serialize;
use serde_json::{Value, json};

#[derive(Debug, Clone)]
pub struct IndexOptions {
    pub root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct IndexReport {
    pub root: PathBuf,
    pub snapshot: String,
    pub files_indexed: usize,
    pub output_dir: PathBuf,
}

pub async fn index_project(options: IndexOptions) -> Result<IndexReport> {
    let root = options
        .root
        .canonicalize()
        .with_context(|| format!("failed to canonicalize {}", options.root.display()))?;

    let source = LocalFileSystemSource::new(&root);
    let files = source
        .discover()
        .await
        .context("failed to discover project files")?;

    let store = MemoryKnowledgeStore::new();
    let snapshot = store
        .begin_snapshot(
            RepoId(repo_id_for_root(&root)),
            SnapshotBase {
                branch: None,
                commit: None,
                parent_snapshot: None,
                working_tree: true,
            },
        )
        .await
        .context("failed to begin snapshot")?;

    let entities = files
        .iter()
        .map(|file| file_entity(file, &snapshot.0))
        .collect::<Vec<_>>();

    store
        .put_entities(snapshot.clone(), entities)
        .await
        .context("failed to store file entities")?;
    store
        .commit_snapshot(snapshot.clone())
        .await
        .context("failed to commit snapshot")?;

    let entities = store
        .query_entities(EntityQuery::default())
        .await
        .context("failed to query indexed entities")?;

    let output_dir = root.join(".athanor/generated/current/jsonl");
    write_jsonl(&output_dir.join("entities.jsonl"), &entities)?;
    write_jsonl::<Value>(&output_dir.join("facts.jsonl"), &[])?;
    write_jsonl::<Value>(&output_dir.join("relations.jsonl"), &[])?;
    write_jsonl::<Value>(&output_dir.join("diagnostics.jsonl"), &[])?;
    write_manifest(
        &output_dir.join("manifest.json"),
        &snapshot.0,
        files.len(),
        entities.len(),
    )?;

    Ok(IndexReport {
        root,
        snapshot: snapshot.0,
        files_indexed: files.len(),
        output_dir,
    })
}

fn file_entity(file: &SourceFile, snapshot: &str) -> Entity {
    let stable_key = StableKey(format!("file://{}", file.path));

    Entity {
        id: EntityId(format!(
            "ent_file_{:016x}",
            stable_hash(stable_key.0.as_bytes())
        )),
        stable_key,
        kind: EntityKind::File,
        name: file.path.clone(),
        title: None,
        source: Some(SourceLocation {
            path: file.path.clone(),
            line_start: None,
            line_end: None,
        }),
        language: file
            .language_hint
            .as_ref()
            .map(|language| athanor_domain::LanguageCode(language.clone())),
        aliases: Vec::new(),
        payload: json!({
            "snapshot": snapshot,
            "content_hash": file.content_hash,
            "has_text_content": file.content.is_some(),
        }),
    }
}

fn write_jsonl<T: Serialize>(path: &Path, items: &[T]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let mut file =
        File::create(path).with_context(|| format!("failed to create {}", path.display()))?;

    for item in items {
        serde_json::to_writer(&mut file, item)
            .with_context(|| format!("failed to write JSON to {}", path.display()))?;
        file.write_all(b"\n")
            .with_context(|| format!("failed to write newline to {}", path.display()))?;
    }

    Ok(())
}

fn write_manifest(
    path: &Path,
    snapshot: &str,
    files_indexed: usize,
    entities: usize,
) -> Result<()> {
    let manifest = json!({
        "schema": "athanor.jsonl_manifest.v1",
        "snapshot": snapshot,
        "files_indexed": files_indexed,
        "entities": entities,
        "facts": 0,
        "relations": 0,
        "diagnostics": 0,
    });

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    fs::write(path, serde_json::to_string_pretty(&manifest)?)
        .with_context(|| format!("failed to write {}", path.display()))
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
        fs::write(root.join("src/lib.rs"), "pub fn hello() {}\n").unwrap();

        let report = index_project(IndexOptions { root: root.clone() })
            .await
            .unwrap();

        assert_eq!(report.files_indexed, 1);
        assert!(report.output_dir.join("entities.jsonl").is_file());
        assert!(report.output_dir.join("manifest.json").is_file());

        let entities = fs::read_to_string(report.output_dir.join("entities.jsonl")).unwrap();
        assert!(entities.contains("file://src/lib.rs"));

        fs::remove_dir_all(root).unwrap();
    }
}
