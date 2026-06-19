#[cfg(windows)]
use std::path::{Component, Prefix};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use athanor_domain::{RepoId, SnapshotBase};
use athanor_store_memory::MemoryKnowledgeStore;

use crate::{JsonlReadModelWriter, RuntimeBuilder};

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
    let read_model = JsonlReadModelWriter::new(&output_dir)
        .write(&output)
        .context("failed to write JSONL read model")?;

    Ok(IndexReport {
        root,
        snapshot: output.snapshot.0,
        files_indexed: output.files.len(),
        output_dir: read_model.output_dir,
    })
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
