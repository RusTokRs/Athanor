use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use athanor_core::{CanonicalSnapshotStore, OperationContext};
use serde::Serialize;
use serde_json::json;

use crate::config::load_config;
use crate::project_path::normalize_canonical_path;
use crate::projection::HTML_REPORT_PROJECTION_SCHEMA;
use crate::{CancellationToken, RuntimeComposition};

pub const HTML_REPORT_SCHEMA: &str = "athanor.html_report.v1";

#[derive(Debug, Clone)]
pub struct HtmlReportOptions {
    pub root: PathBuf,
    pub output: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HtmlReport {
    pub schema: &'static str,
    pub root: PathBuf,
    pub output_dir: PathBuf,
    pub snapshot: String,
    pub entities: usize,
    pub facts: usize,
    pub relations: usize,
    pub open_diagnostics: usize,
}

/// Projects the HTML report with explicitly supplied runtime dependencies.
pub async fn project_html_report_with_composition(
    options: HtmlReportOptions,
    composition: &RuntimeComposition,
) -> Result<HtmlReport> {
    project_html_report_inner(
        options,
        None,
        composition,
        OperationContext::new("html_report"),
    )
    .await
}

/// Projects the HTML report with explicit runtime dependencies and operation metadata.
pub async fn project_html_report_with_composition_and_operation_context(
    options: HtmlReportOptions,
    composition: &RuntimeComposition,
    operation: OperationContext,
) -> Result<HtmlReport> {
    project_html_report_inner(options, None, composition, operation).await
}

/// Projects the HTML report with explicit dependencies and cooperative cancellation.
pub async fn project_html_report_cancellable_with_composition(
    options: HtmlReportOptions,
    cancellation: CancellationToken,
    composition: &RuntimeComposition,
) -> Result<HtmlReport> {
    project_html_report_inner(
        options,
        Some(cancellation),
        composition,
        OperationContext::new("html_report"),
    )
    .await
}

/// Cancellable composition-aware HTML projection with explicit operation metadata.
pub async fn project_html_report_cancellable_with_composition_and_operation_context(
    options: HtmlReportOptions,
    cancellation: CancellationToken,
    composition: &RuntimeComposition,
    operation: OperationContext,
) -> Result<HtmlReport> {
    project_html_report_inner(options, Some(cancellation), composition, operation).await
}

async fn project_html_report_inner(
    options: HtmlReportOptions,
    cancellation: Option<CancellationToken>,
    composition: &RuntimeComposition,
    operation: OperationContext,
) -> Result<HtmlReport> {
    check_cancelled(&cancellation)?;
    operation.check_deadline()?;
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
        .unwrap_or_else(|| root.join(".athanor/generated/current/html"));
    let config = load_config(&root)?;
    let store = composition.init_store(&root, &config).await?;
    let snapshot = store
        .load_latest_snapshot()
        .await
        .context("failed to load latest canonical snapshot")?;
    let Some(snapshot) = snapshot else {
        bail!("no canonical snapshot found; run `ath index` first");
    };
    check_cancelled(&cancellation)?;
    operation.check_deadline()?;
    let snapshot_id = snapshot
        .snapshot
        .clone()
        .context("latest canonical snapshot has no snapshot id")?;
    let open_diagnostics = snapshot
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.status == athanor_domain::DiagnosticStatus::Open)
        .count();
    let report = HtmlReport {
        schema: HTML_REPORT_SCHEMA,
        root,
        output_dir: output_dir.clone(),
        snapshot: snapshot_id.0.clone(),
        entities: snapshot.entities.len(),
        facts: snapshot.facts.len(),
        relations: snapshot.relations.len(),
        open_diagnostics,
    };
    let payload = json!({
        "schema": HTML_REPORT_PROJECTION_SCHEMA,
        "entities": snapshot.entities,
        "facts": snapshot.facts,
        "relations": snapshot.relations,
        "diagnostics": snapshot.diagnostics,
    });

    let is_cancelled = || {
        cancellation
            .as_ref()
            .is_some_and(CancellationToken::is_cancelled)
            || operation.check_deadline().is_err()
    };
    composition
        .project_html(&output_dir, &snapshot_id.0, payload, &is_cancelled)
        .context("failed to project HTML report")?;
    operation.check_deadline()?;

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
            "athanor-html-report-app-test-{}",
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
        let composition = crate::test_runtime::composition();

        let report = project_html_report_with_composition(
            HtmlReportOptions {
                root: root.clone(),
                output: None,
            },
            &composition,
        )
        .await
        .unwrap();

        assert_eq!(report.schema, HTML_REPORT_SCHEMA);
        assert_eq!(report.snapshot, snapshot.0);
        assert!(report.output_dir.join("index.html").is_file());
        assert!(report.output_dir.join("manifest.json").is_file());
        fs::remove_dir_all(root).unwrap();
    }
}
