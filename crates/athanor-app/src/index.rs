use std::fs::{self, File};
use std::io::Write;
#[cfg(windows)]
use std::path::{Component, Prefix};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use athanor_domain::{RepoId, SnapshotBase};
use athanor_store_memory::MemoryKnowledgeStore;
use serde::Serialize;
use serde_json::json;

use crate::RuntimeBuilder;

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
    let root = normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );

    let output = RuntimeBuilder::new(&root)
        .build_index_pipeline(MemoryKnowledgeStore::new())
        .run(
            RepoId(repo_id_for_root(&root)),
            SnapshotBase {
                branch: None,
                commit: None,
                parent_snapshot: None,
                working_tree: true,
            },
        )
        .await
        .context("failed to run index pipeline")?;

    let output_dir = root.join(".athanor/generated/current/jsonl");
    write_jsonl(&output_dir.join("entities.jsonl"), &output.entities)?;
    write_jsonl(&output_dir.join("facts.jsonl"), &output.facts)?;
    write_jsonl(&output_dir.join("relations.jsonl"), &output.relations)?;
    write_jsonl(&output_dir.join("diagnostics.jsonl"), &output.diagnostics)?;
    write_manifest(
        &output_dir.join("manifest.json"),
        &output.snapshot.0,
        output.files.len(),
        output.entities.len(),
        output.facts.len(),
        output.relations.len(),
        output.diagnostics.len(),
    )?;

    Ok(IndexReport {
        root,
        snapshot: output.snapshot.0,
        files_indexed: output.files.len(),
        output_dir,
    })
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
    facts: usize,
    relations: usize,
    diagnostics: usize,
) -> Result<()> {
    let manifest = json!({
        "schema": "athanor.jsonl_manifest.v1",
        "snapshot": snapshot,
        "files_indexed": files_indexed,
        "entities": entities,
        "facts": facts,
        "relations": relations,
        "diagnostics": diagnostics,
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

fn normalize_canonical_path(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        let mut components = path.components();

        if let Some(Component::Prefix(prefix)) = components.next() {
            match prefix.kind() {
                Prefix::VerbatimDisk(disk) => {
                    let drive = char::from(disk);
                    return PathBuf::from(format!("{drive}:\\")).join(components.as_path());
                }
                Prefix::VerbatimUNC(server, share) => {
                    return PathBuf::from(format!(
                        "\\\\{}\\{}",
                        server.to_string_lossy(),
                        share.to_string_lossy()
                    ))
                    .join(components.as_path());
                }
                _ => {}
            }
        }
    }

    path
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
        fs::create_dir_all(root.join("docs")).unwrap();
        fs::write(root.join("src/lib.rs"), "pub fn hello() {}\n").unwrap();
        fs::write(root.join("docs/auth.md"), "# Auth\n\n## Login\n").unwrap();

        let report = index_project(IndexOptions { root: root.clone() })
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

        fs::remove_dir_all(root).unwrap();
    }
}
