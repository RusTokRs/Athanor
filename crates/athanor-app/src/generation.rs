use std::fs;
use std::path::{Path, PathBuf};

use crate::config::load_config;
use crate::store::init_store;
use anyhow::{Context, Result, bail};
use athanor_core::{CanonicalSnapshotStore, ProjectInput, Projector};
use athanor_projector_html::{
    HTML_REPORT_PROJECTION_SCHEMA, HtmlReportProjectionPayload, HtmlReportProjector,
};
use athanor_projector_support::{NewDirectoryPublication, replace_output_file, write_output_file};
use athanor_projector_wiki::{
    MarkdownWikiProjector, WIKI_PROJECTION_SCHEMA, WikiProjectionPayload,
};
use serde::{Deserialize, Serialize};

use crate::JsonlReadModelWriter;
use crate::project_path::normalize_canonical_path;

pub const GENERATED_GENERATION_SCHEMA: &str = "athanor.generated_generation.v1";
pub const GENERATED_CURRENT_SCHEMA: &str = "athanor.generated_current.v1";

#[derive(Debug, Clone)]
pub struct GenerationOptions {
    pub root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct GenerationReport {
    pub root: PathBuf,
    pub generation: String,
    pub generation_dir: PathBuf,
    pub current_pointer: PathBuf,
    pub snapshot: String,
    pub entities: usize,
    pub facts: usize,
    pub relations: usize,
    pub diagnostics: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentGeneration {
    pub schema: String,
    pub generation: String,
    pub snapshot: String,
    pub path: String,
    pub manifest: String,
}

#[derive(Debug, Serialize)]
struct GenerationManifest<'a> {
    schema: &'static str,
    status: &'static str,
    generation: &'a str,
    snapshot: &'a str,
    athanor_version: &'static str,
    entities: usize,
    facts: usize,
    relations: usize,
    diagnostics: usize,
    outputs: GenerationOutputs,
}

#[derive(Debug, Serialize)]
struct GenerationOutputs {
    jsonl: &'static str,
    wiki: &'static str,
    html: &'static str,
}

pub async fn generate_project(options: GenerationOptions) -> Result<GenerationReport> {
    let root = normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );
    let generated_root = root.join(".athanor/generated");
    let generations_dir = generated_root.join("generations");
    let generation = next_generation_id(&generations_dir)?;
    let generation_dir = generations_dir.join(&generation);
    let current_pointer = generated_root.join("current.json");
    let config = load_config(&root)?;
    let store = init_store(&root, &config).await?;
    let snapshot = store
        .load_latest_snapshot()
        .await
        .context("failed to load latest canonical snapshot")?;
    let Some(snapshot) = snapshot else {
        bail!("no canonical snapshot found; run `ath index` first");
    };
    let snapshot_id = snapshot
        .snapshot
        .clone()
        .context("latest canonical snapshot has no snapshot id")?;

    let publication = NewDirectoryPublication::new(&generation_dir, "generated generation")
        .context("failed to prepare generated generation")?;
    let staging = publication.staging_path().to_path_buf();
    JsonlReadModelWriter::new(staging.join("jsonl"))
        .write_canonical_snapshot(&snapshot)
        .context("failed to project generation JSONL")?;

    let wiki_payload = WikiProjectionPayload {
        schema: WIKI_PROJECTION_SCHEMA.to_string(),
        entities: snapshot.entities.clone(),
        facts: snapshot.facts.clone(),
        relations: snapshot.relations.clone(),
        diagnostics: snapshot.diagnostics.clone(),
    };
    MarkdownWikiProjector
        .project(ProjectInput {
            snapshot: snapshot_id.clone(),
            target: staging.join("wiki").to_string_lossy().into_owned(),
            payload: serde_json::to_value(wiki_payload)
                .context("failed to build generation wiki input")?,
        })
        .await
        .context("failed to project generation wiki")?;

    let html_payload = HtmlReportProjectionPayload {
        schema: HTML_REPORT_PROJECTION_SCHEMA.to_string(),
        entities: snapshot.entities.clone(),
        facts: snapshot.facts.clone(),
        relations: snapshot.relations.clone(),
        diagnostics: snapshot.diagnostics.clone(),
    };
    HtmlReportProjector
        .project(ProjectInput {
            snapshot: snapshot_id.clone(),
            target: staging.join("html").to_string_lossy().into_owned(),
            payload: serde_json::to_value(html_payload)
                .context("failed to build generation HTML input")?,
        })
        .await
        .context("failed to project generation HTML report")?;

    let manifest = GenerationManifest {
        schema: GENERATED_GENERATION_SCHEMA,
        status: "complete",
        generation: &generation,
        snapshot: &snapshot_id.0,
        athanor_version: env!("CARGO_PKG_VERSION"),
        entities: snapshot.entities.len(),
        facts: snapshot.facts.len(),
        relations: snapshot.relations.len(),
        diagnostics: snapshot.diagnostics.len(),
        outputs: GenerationOutputs {
            jsonl: "jsonl",
            wiki: "wiki",
            html: "html",
        },
    };
    write_output_file(
        &staging.join("manifest.json"),
        &serde_json::to_string_pretty(&manifest)
            .context("failed to serialize generation manifest")?,
    )
    .context("failed to write generation manifest")?;
    publication
        .publish()
        .context("failed to publish immutable generation")?;

    let pointer = CurrentGeneration {
        schema: GENERATED_CURRENT_SCHEMA.to_string(),
        generation: generation.clone(),
        snapshot: snapshot_id.0.clone(),
        path: format!("generations/{generation}"),
        manifest: format!("generations/{generation}/manifest.json"),
    };
    replace_output_file(
        &current_pointer,
        &serde_json::to_string_pretty(&pointer)
            .context("failed to serialize current generation pointer")?,
        "current generation pointer",
    )
    .context("failed to update current generation pointer")?;

    Ok(GenerationReport {
        root,
        generation,
        generation_dir,
        current_pointer,
        snapshot: snapshot_id.0,
        entities: snapshot.entities.len(),
        facts: snapshot.facts.len(),
        relations: snapshot.relations.len(),
        diagnostics: snapshot.diagnostics.len(),
    })
}

fn next_generation_id(generations_dir: &Path) -> Result<String> {
    let max = match fs::read_dir(generations_dir) {
        Ok(entries) => entries
            .filter_map(std::result::Result::ok)
            .filter_map(|entry| entry.file_name().to_str().map(str::to_string))
            .filter_map(|name| name.parse::<u64>().ok())
            .max()
            .unwrap_or(0),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "failed to inspect generations at {}",
                    generations_dir.display()
                )
            });
        }
    };
    let next = max
        .checked_add(1)
        .context("generated generation number overflow")?;
    Ok(format!("{next:08}"))
}

#[cfg(test)]
mod tests {
    use athanor_core::KnowledgeStore;
    use athanor_domain::{RepoId, SnapshotBase};
    use athanor_store_jsonl::JsonlKnowledgeStore;

    use super::*;

    #[tokio::test]
    async fn publishes_complete_immutable_generations_and_moves_pointer() {
        let root = std::env::temp_dir().join(format!(
            "athanor-generation-app-test-{}",
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

        let first = generate_project(GenerationOptions { root: root.clone() })
            .await
            .unwrap();
        let second = generate_project(GenerationOptions { root: root.clone() })
            .await
            .unwrap();

        assert_eq!(first.generation, "00000001");
        assert_eq!(second.generation, "00000002");
        assert!(first.generation_dir.join("jsonl/manifest.json").is_file());
        assert!(first.generation_dir.join("wiki/index.md").is_file());
        assert!(first.generation_dir.join("html/index.html").is_file());
        assert!(first.generation_dir.join("manifest.json").is_file());
        let pointer: CurrentGeneration =
            serde_json::from_str(&fs::read_to_string(&second.current_pointer).unwrap()).unwrap();
        assert_eq!(pointer.generation, "00000002");
        assert_eq!(pointer.snapshot, snapshot.0);
        assert!(first.generation_dir.is_dir());

        fs::remove_dir_all(root).unwrap();
    }
}
