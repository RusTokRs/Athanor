use std::path::PathBuf;

use crate::config::load_config;
use crate::store::init_store;
use anyhow::{Context, Result, bail};
use athanor_core::CanonicalSnapshotStore;
use serde_json::json;

use crate::CancellationToken;
use crate::project_path::normalize_canonical_path;
use crate::projection::{WIKI_PROJECTION_SCHEMA, project_wiki_payload};

#[derive(Debug, Clone)]
pub struct WikiOptions {
    pub root: PathBuf,
    pub output: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct WikiReport {
    pub root: PathBuf,
    pub output_dir: PathBuf,
    pub snapshot: String,
    pub entities: usize,
    pub facts: usize,
    pub relations: usize,
    pub open_diagnostics: usize,
}

pub async fn project_wiki(options: WikiOptions) -> Result<WikiReport> {
    project_wiki_inner(options, None).await
}

pub async fn project_wiki_cancellable(
    options: WikiOptions,
    cancellation: CancellationToken,
) -> Result<WikiReport> {
    project_wiki_inner(options, Some(cancellation)).await
}

async fn project_wiki_inner(
    options: WikiOptions,
    cancellation: Option<CancellationToken>,
) -> Result<WikiReport> {
    check_cancelled(&cancellation)?;
    let root = normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );
    let output_dir = options
        .output
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                root.join(path)
            }
        })
        .unwrap_or_else(|| root.join(".athanor/generated/current/wiki"));
    let config = load_config(&root)?;
    let store = init_store(&root, &config).await?;
    let snapshot = store
        .load_latest_snapshot()
        .await
        .context("failed to load latest canonical snapshot")?;
    let Some(snapshot) = snapshot else {
        bail!("no canonical snapshot found; run `ath index` first");
    };
    check_cancelled(&cancellation)?;
    let snapshot_id = snapshot
        .snapshot
        .clone()
        .context("latest canonical snapshot has no snapshot id")?;
    let open_diagnostics = snapshot
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.status == athanor_domain::DiagnosticStatus::Open)
        .count();
    let report = WikiReport {
        root,
        output_dir: output_dir.clone(),
        snapshot: snapshot_id.0.clone(),
        entities: snapshot.entities.len(),
        facts: snapshot.facts.len(),
        relations: snapshot.relations.len(),
        open_diagnostics,
    };
    let payload = json!({
        "schema": WIKI_PROJECTION_SCHEMA,
        "entities": snapshot.entities,
        "facts": snapshot.facts,
        "relations": snapshot.relations,
        "diagnostics": snapshot.diagnostics,
    });

    project_wiki_payload(&output_dir, &snapshot_id.0, payload, &|| {
        cancellation
            .as_ref()
            .is_some_and(CancellationToken::is_cancelled)
    })
    .context("failed to project Markdown wiki")?;

    Ok(report)
}

fn check_cancelled(cancellation: &Option<CancellationToken>) -> Result<()> {
    if let Some(cancellation) = cancellation {
        cancellation.check()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use athanor_core::KnowledgeStore;
    use athanor_domain::{RepoId, SnapshotBase};
    use athanor_store_jsonl::JsonlKnowledgeStore;

    use super::*;

    #[tokio::test]
    async fn projects_latest_canonical_snapshot() {
        let root = std::env::temp_dir().join(format!(
            "athanor-wiki-app-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let store = JsonlKnowledgeStore::new(root.join(".athanor/store/canonical/jsonl"));
        let snapshot = store
            .begin_snapshot(
                RepoId("repo_test".to_string()),
                SnapshotBase {
                    branch: None,
                    commit: None,
                    parent_snapshot: None,
                    working_tree: true,
                },
            )
            .await
            .unwrap();
        store.commit_snapshot(snapshot.clone()).await.unwrap();

        let report = project_wiki(WikiOptions {
            root: root.clone(),
            output: None,
        })
        .await
        .unwrap();

        assert_eq!(report.snapshot, snapshot.0);
        assert!(report.output_dir.join("index.md").is_file());
        assert!(report.output_dir.join("manifest.json").is_file());

        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn requires_an_existing_snapshot() {
        let root = std::env::temp_dir().join(format!(
            "athanor-wiki-no-snapshot-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();

        let error = project_wiki(WikiOptions {
            root: root.clone(),
            output: None,
        })
        .await
        .unwrap_err();

        assert!(error.to_string().contains("run `ath index` first"));
        fs::remove_dir_all(root).unwrap();
    }
}
