use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::config::load_config;
use crate::store::init_store;
use anyhow::{Context, Result, bail};
use athanor_core::CanonicalSnapshotStore;
use athanor_projector_html::{
    HTML_REPORT_PROJECTION_SCHEMA, HtmlReportProjectionPayload,
    project_html_report_payload_cancellable,
};
use athanor_projector_support::{NewDirectoryPublication, replace_output_file, write_output_file};
use athanor_projector_wiki::{
    WIKI_PROJECTION_SCHEMA, WikiProjectionPayload, project_wiki_payload_cancellable,
};
use serde::{Deserialize, Serialize};

use crate::CancellationToken;
use crate::JsonlReadModelWriter;
use crate::project_path::normalize_canonical_path;

pub const GENERATED_GENERATION_SCHEMA: &str = "athanor.generated_generation.v1";
pub const GENERATED_CURRENT_SCHEMA: &str = "athanor.generated_current.v1";

#[derive(Debug, Clone)]
pub struct GenerationOptions {
    pub root: PathBuf,
    pub force: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct GenerationReport {
    pub schema: &'static str,
    pub status: GenerationStatus,
    pub root: PathBuf,
    pub generation: String,
    pub generation_dir: PathBuf,
    pub current_pointer: PathBuf,
    pub snapshot: String,
    pub entities: usize,
    pub facts: usize,
    pub relations: usize,
    pub diagnostics: usize,
    pub metrics: GenerationMetrics,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GenerationStatus {
    Published,
    UpToDate,
}

#[derive(Debug, Clone, Serialize)]
pub struct GenerationMetrics {
    pub schema: &'static str,
    pub total_ms: u64,
    pub snapshot_load_ms: u64,
    pub jsonl_ms: u64,
    pub wiki_ms: u64,
    pub html_ms: u64,
    pub publish_ms: u64,
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
    generate_project_inner(options, None).await
}

pub async fn generate_project_cancellable(
    options: GenerationOptions,
    cancellation: CancellationToken,
) -> Result<GenerationReport> {
    generate_project_inner(options, Some(cancellation)).await
}

async fn generate_project_inner(
    options: GenerationOptions,
    cancellation: Option<CancellationToken>,
) -> Result<GenerationReport> {
    let total_started = Instant::now();
    check_cancelled(&cancellation)?;
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
    let snapshot_load_started = Instant::now();
    let config = load_config(&root)?;
    let store = init_store(&root, &config).await?;
    let snapshot = store
        .load_latest_snapshot()
        .await
        .context("failed to load latest canonical snapshot")?;
    let Some(snapshot) = snapshot else {
        bail!("no canonical snapshot found; run `ath index` first");
    };
    let snapshot_load_ms = elapsed_ms(snapshot_load_started);
    check_cancelled(&cancellation)?;
    let snapshot_id = snapshot
        .snapshot
        .clone()
        .context("latest canonical snapshot has no snapshot id")?;
    if !options.force
        && let Some(current) = load_current_generation_if_current(&generated_root, &snapshot_id.0)?
    {
        let generation_dir = generated_root.join(&current.path);
        return Ok(GenerationReport {
            schema: "athanor.generation.v1",
            status: GenerationStatus::UpToDate,
            root,
            generation: current.generation.clone(),
            generation_dir,
            current_pointer,
            snapshot: snapshot_id.0,
            entities: snapshot.entities.len(),
            facts: snapshot.facts.len(),
            relations: snapshot.relations.len(),
            diagnostics: snapshot.diagnostics.len(),
            metrics: GenerationMetrics {
                schema: "athanor.generation_metrics.v1",
                total_ms: elapsed_ms(total_started),
                snapshot_load_ms,
                jsonl_ms: 0,
                wiki_ms: 0,
                html_ms: 0,
                publish_ms: 0,
            },
        });
    }

    let publication = NewDirectoryPublication::new(&generation_dir, "generated generation")
        .context("failed to prepare generated generation")?;
    let staging = publication.staging_path().to_path_buf();
    let jsonl_started = Instant::now();
    JsonlReadModelWriter::new(staging.join("jsonl"))
        .write_canonical_snapshot(&snapshot)
        .context("failed to project generation JSONL")?;
    let jsonl_ms = elapsed_ms(jsonl_started);
    check_cancelled(&cancellation)?;

    let wiki_started = Instant::now();
    let wiki_payload = WikiProjectionPayload {
        schema: WIKI_PROJECTION_SCHEMA.to_string(),
        entities: snapshot.entities.clone(),
        facts: snapshot.facts.clone(),
        relations: snapshot.relations.clone(),
        diagnostics: snapshot.diagnostics.clone(),
    };
    let wiki_target = staging.join("wiki");
    let wiki_snapshot = snapshot_id.0.clone();
    let wiki_cancellation = cancellation.clone();
    let wiki = tokio::task::spawn_blocking(move || {
        project_wiki_payload_cancellable(&wiki_target, &wiki_snapshot, wiki_payload, &|| {
            wiki_cancellation
                .as_ref()
                .is_some_and(CancellationToken::is_cancelled)
        })
        .map(|_| elapsed_ms(wiki_started))
    });

    let html_started = Instant::now();
    let html_payload = HtmlReportProjectionPayload {
        schema: HTML_REPORT_PROJECTION_SCHEMA.to_string(),
        entities: snapshot.entities.clone(),
        facts: snapshot.facts.clone(),
        relations: snapshot.relations.clone(),
        diagnostics: snapshot.diagnostics.clone(),
    };
    let html_target = staging.join("html");
    let html_snapshot = snapshot_id.0.clone();
    let html_cancellation = cancellation.clone();
    let html = tokio::task::spawn_blocking(move || {
        project_html_report_payload_cancellable(html_target, &html_snapshot, html_payload, &|| {
            html_cancellation
                .as_ref()
                .is_some_and(CancellationToken::is_cancelled)
        })
        .map(|_| elapsed_ms(html_started))
    });

    let (wiki_result, html_result) = tokio::join!(wiki, html);
    let wiki_ms = wiki_result
        .context("generation wiki worker panicked")?
        .context("failed to project generation wiki")?;
    let html_ms = html_result
        .context("generation HTML report worker panicked")?
        .context("failed to project generation HTML report")?;
    check_cancelled(&cancellation)?;

    let publish_started = Instant::now();
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
    check_cancelled(&cancellation)?;
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
    let publish_ms = elapsed_ms(publish_started);

    Ok(GenerationReport {
        schema: "athanor.generation.v1",
        status: GenerationStatus::Published,
        root,
        generation,
        generation_dir,
        current_pointer,
        snapshot: snapshot_id.0,
        entities: snapshot.entities.len(),
        facts: snapshot.facts.len(),
        relations: snapshot.relations.len(),
        diagnostics: snapshot.diagnostics.len(),
        metrics: GenerationMetrics {
            schema: "athanor.generation_metrics.v1",
            total_ms: elapsed_ms(total_started),
            snapshot_load_ms,
            jsonl_ms,
            wiki_ms,
            html_ms,
            publish_ms,
        },
    })
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn load_current_generation_if_current(
    generated_root: &Path,
    latest_snapshot: &str,
) -> Result<Option<CurrentGeneration>> {
    let current_pointer = generated_root.join("current.json");
    let Ok(content) = fs::read_to_string(&current_pointer) else {
        return Ok(None);
    };
    let current: CurrentGeneration =
        serde_json::from_str(&content).context("failed to parse current generation pointer")?;
    if current.schema != GENERATED_CURRENT_SCHEMA || current.snapshot != latest_snapshot {
        return Ok(None);
    }
    let generation_dir = generated_root.join(&current.path);
    let manifest_path = generated_root.join(&current.manifest);
    if !generation_dir.is_dir() || !manifest_path.is_file() {
        return Ok(None);
    }
    let manifest: GenerationManifestOwned = serde_json::from_str(
        &fs::read_to_string(&manifest_path)
            .context("failed to read current generation manifest")?,
    )
    .context("failed to parse current generation manifest")?;
    if manifest.schema == GENERATED_GENERATION_SCHEMA
        && manifest.generation == current.generation
        && manifest.snapshot == latest_snapshot
    {
        Ok(Some(current))
    } else {
        Ok(None)
    }
}

#[derive(Debug, Deserialize)]
struct GenerationManifestOwned {
    schema: String,
    generation: String,
    snapshot: String,
}

fn check_cancelled(cancellation: &Option<CancellationToken>) -> Result<()> {
    if let Some(cancellation) = cancellation {
        cancellation.check()?;
    }
    Ok(())
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

        let first = generate_project(GenerationOptions {
            root: root.clone(),
            force: false,
        })
        .await
        .unwrap();
        let second = generate_project(GenerationOptions {
            root: root.clone(),
            force: false,
        })
        .await
        .unwrap();
        let forced = generate_project(GenerationOptions {
            root: root.clone(),
            force: true,
        })
        .await
        .unwrap();

        assert_eq!(first.generation, "00000001");
        assert_eq!(first.status, GenerationStatus::Published);
        assert_eq!(second.generation, "00000001");
        assert_eq!(second.status, GenerationStatus::UpToDate);
        assert_eq!(second.metrics.jsonl_ms, 0);
        assert_eq!(second.metrics.wiki_ms, 0);
        assert_eq!(second.metrics.html_ms, 0);
        assert_eq!(forced.generation, "00000002");
        assert_eq!(forced.status, GenerationStatus::Published);
        assert!(first.generation_dir.join("jsonl/manifest.json").is_file());
        assert!(first.generation_dir.join("wiki/index.md").is_file());
        assert!(first.generation_dir.join("html/index.html").is_file());
        assert!(first.generation_dir.join("manifest.json").is_file());
        let pointer: CurrentGeneration =
            serde_json::from_str(&fs::read_to_string(&forced.current_pointer).unwrap()).unwrap();
        assert_eq!(pointer.generation, "00000002");
        assert_eq!(pointer.snapshot, snapshot.0);
        assert!(first.generation_dir.is_dir());

        fs::remove_dir_all(root).unwrap();
    }
}
