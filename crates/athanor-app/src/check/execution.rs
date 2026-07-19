use std::path::PathBuf;

use anyhow::{Context, Result};
use athanor_core::{CanonicalSnapshot, CanonicalSnapshotStore};

use crate::RuntimeComposition;
use crate::config::{ProjectConfig, load_config};
use crate::json_contract::OPERATIONS_DOCS_CHECK_SCHEMA_V1;
use crate::project_path::normalize_canonical_path;

use super::affected::build_affected_check_report;
use super::diagnostics::{build_check_report, sum_counts};
use super::model::{
    AffectedCheckOptions, AffectedCheckReport, DiagnosticCheckOptions, DiagnosticCheckReport,
    DiagnosticScope, OperationsDocsCheckOptions, OperationsDocsCheckReport,
};

/// Runs diagnostic checks with explicitly supplied runtime dependencies.
pub async fn check_project_with_composition(
    options: DiagnosticCheckOptions,
    composition: &RuntimeComposition,
) -> Result<DiagnosticCheckReport> {
    let (_, config, snapshot) = load_latest_check_snapshot(options.root, composition).await?;
    Ok(build_check_report(
        snapshot_id(&snapshot),
        options.scope,
        &snapshot.diagnostics,
        &config.api,
    ))
}

/// Runs the affected-file diagnostic check with explicitly supplied runtime dependencies.
pub async fn check_affected_with_composition(
    options: AffectedCheckOptions,
    composition: &RuntimeComposition,
) -> Result<AffectedCheckReport> {
    let (root, config, snapshot) = load_latest_check_snapshot(options.root, composition).await?;
    build_affected_check_report(root, snapshot_id(&snapshot), &snapshot, &config)
}

/// Checks operations documentation with explicitly supplied runtime dependencies.
pub async fn check_operations_docs_with_composition(
    options: OperationsDocsCheckOptions,
    composition: &RuntimeComposition,
) -> Result<OperationsDocsCheckReport> {
    let (_, config, snapshot) = load_latest_check_snapshot(options.root, composition).await?;
    let snapshot_id = snapshot_id(&snapshot);

    let env = build_check_report(
        snapshot_id.clone(),
        DiagnosticScope::Env,
        &snapshot.diagnostics,
        &config.api,
    );
    let scripts = build_check_report(
        snapshot_id.clone(),
        DiagnosticScope::Scripts,
        &snapshot.diagnostics,
        &config.api,
    );
    let deployment = build_check_report(
        snapshot_id.clone(),
        DiagnosticScope::Deployment,
        &snapshot.diagnostics,
        &config.api,
    );
    let runbooks = build_check_report(
        snapshot_id.clone(),
        DiagnosticScope::Runbooks,
        &snapshot.diagnostics,
        &config.api,
    );

    Ok(OperationsDocsCheckReport {
        schema: OPERATIONS_DOCS_CHECK_SCHEMA_V1.to_string(),
        snapshot: snapshot_id,
        counts: sum_counts([&env, &scripts, &deployment, &runbooks]),
        env,
        scripts,
        deployment,
        runbooks,
    })
}

async fn load_latest_check_snapshot(
    root: PathBuf,
    composition: &RuntimeComposition,
) -> Result<(PathBuf, ProjectConfig, CanonicalSnapshot)> {
    let root = normalize_canonical_path(
        root.canonicalize()
            .with_context(|| format!("failed to canonicalize {}", root.display()))?,
    );
    let config = load_config(&root)?;
    let store = composition.init_store(&root, &config).await?;
    let snapshot = store
        .load_latest_snapshot()
        .await
        .context("failed to load latest canonical snapshot")?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "no canonical snapshot found; run `ath index {}` first",
                root.display()
            )
        })?;
    Ok((root, config, snapshot))
}

fn snapshot_id(snapshot: &CanonicalSnapshot) -> String {
    snapshot
        .snapshot
        .as_ref()
        .map_or_else(|| "unknown".to_string(), |snapshot| snapshot.0.clone())
}
