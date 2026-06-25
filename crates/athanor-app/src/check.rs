use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::docs::{DriftedDocument, build_docs_drift_report};
use crate::index_state::{AffectedFileSet, IndexStateStore};
use crate::repair::{RepairInspectOptions, RepairIssue, inspect_repair};
use crate::store::init_store;
use anyhow::{Context, Result};
use athanor_core::{CanonicalSnapshotStore, SourceProvider};
use athanor_domain::{Diagnostic, DiagnosticKind, DiagnosticStatus, Entity, EntityId, Severity};
use athanor_source_fs::LocalFileSystemSource;
use serde::{Deserialize, Serialize};

use crate::project_path::normalize_canonical_path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticScope {
    Api,
    Docs,
    Env,
    Scripts,
    Deployment,
    Runbooks,
    RustokFfa,
    RustokFba,
}

#[derive(Debug, Clone)]
pub struct DiagnosticCheckOptions {
    pub root: PathBuf,
    pub scope: DiagnosticScope,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct DiagnosticCounts {
    pub total: usize,
    pub critical: usize,
    pub high: usize,
    pub medium: usize,
    pub low: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticCheckReport {
    pub schema: String,
    pub snapshot: String,
    pub scope: DiagnosticScope,
    pub counts: DiagnosticCounts,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone)]
pub struct AffectedCheckOptions {
    pub root: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct AffectedCheckReport {
    pub schema: String,
    pub snapshot: String,
    pub affected_files: AffectedFileCounts,
    pub stale_artifacts: Vec<AffectedArtifactStatus>,
    pub documentation_drift: Vec<DriftedDocument>,
    pub counts: DiagnosticCounts,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AffectedArtifactStatus {
    pub kind: AffectedArtifactKind,
    pub path: PathBuf,
    pub message: String,
    pub suggested_command: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AffectedArtifactKind {
    GeneratedCurrent,
    GeneratedGeneration,
    Wiki,
    HtmlReport,
    ApiContract,
    ApiDiff,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct AffectedFileCounts {
    pub changed: usize,
    pub unchanged: usize,
    pub removed: usize,
}

#[derive(Debug, Clone)]
pub struct OperationsDocsCheckOptions {
    pub root: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct OperationsDocsCheckReport {
    pub schema: String,
    pub snapshot: String,
    pub counts: DiagnosticCounts,
    pub env: DiagnosticCheckReport,
    pub scripts: DiagnosticCheckReport,
    pub deployment: DiagnosticCheckReport,
    pub runbooks: DiagnosticCheckReport,
}

use crate::config::{ApiConfig, ApiSourceOfTruth, load_config};

pub async fn check_project(options: DiagnosticCheckOptions) -> Result<DiagnosticCheckReport> {
    let root = normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );
    let config = load_config(&root)?;
    let store = init_store(&root, &config).await?;
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
    let snapshot_id = snapshot
        .snapshot
        .as_ref()
        .map_or_else(|| "unknown".to_string(), |snapshot| snapshot.0.clone());

    Ok(build_check_report(
        snapshot_id,
        options.scope,
        &snapshot.diagnostics,
        &config.api,
    ))
}

pub async fn check_affected(options: AffectedCheckOptions) -> Result<AffectedCheckReport> {
    let root = normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );
    let config = load_config(&root)?;
    let store = init_store(&root, &config).await?;
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
    let snapshot_id = snapshot
        .snapshot
        .as_ref()
        .map_or_else(|| "unknown".to_string(), |snapshot| snapshot.0.clone());

    let state_store = IndexStateStore::new(root.join(".athanor/state/index-state.json"));
    let previous_state = state_store.load().context("failed to load index state")?;
    let source = LocalFileSystemSource::new(&root);
    let current_files = source
        .discover()
        .await
        .context("failed to discover source files for affected check")?;
    let affected_files = previous_state.affected_files(&current_files);
    let affected_paths = affected_path_set(&affected_files);
    let affected_entity_ids = affected_entity_ids(&snapshot.entities, &affected_paths);

    let mut diagnostics = snapshot
        .diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic.status == DiagnosticStatus::Open
                && diagnostic_touches_paths_or_entities(
                    diagnostic,
                    &affected_paths,
                    &affected_entity_ids,
                )
        })
        .cloned()
        .collect::<Vec<_>>();
    diagnostics
        .sort_by_key(|diagnostic| (severity_rank(diagnostic.severity), diagnostic.id.0.clone()));

    let counts = diagnostic_counts(&diagnostics);

    Ok(AffectedCheckReport {
        schema: "athanor.affected_check.v1".to_string(),
        stale_artifacts: affected_artifact_statuses(&root, &snapshot_id)?,
        documentation_drift: affected_documentation_drift(
            snapshot_id.clone(),
            &snapshot.entities,
            &config.docs,
            &affected_paths,
        ),
        snapshot: snapshot_id,
        affected_files: AffectedFileCounts {
            changed: affected_files.changed.len(),
            unchanged: affected_files.unchanged.len(),
            removed: affected_files.removed.len(),
        },
        counts,
        diagnostics,
    })
}

pub async fn check_operations_docs(
    options: OperationsDocsCheckOptions,
) -> Result<OperationsDocsCheckReport> {
    let root = normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );
    let config = load_config(&root)?;
    let store = init_store(&root, &config).await?;
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
    let snapshot_id = snapshot
        .snapshot
        .as_ref()
        .map_or_else(|| "unknown".to_string(), |snapshot| snapshot.0.clone());

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
        schema: "athanor.operations_docs_check.v1".to_string(),
        snapshot: snapshot_id,
        counts: sum_counts([&env, &scripts, &deployment, &runbooks]),
        env,
        scripts,
        deployment,
        runbooks,
    })
}

pub fn build_check_report(
    snapshot: String,
    scope: DiagnosticScope,
    diagnostics: &[Diagnostic],
    config: &ApiConfig,
) -> DiagnosticCheckReport {
    let mut diagnostics = diagnostics
        .iter()
        .filter(|diagnostic| {
            if diagnostic.status != DiagnosticStatus::Open
                || !diagnostic_matches_scope(&diagnostic.kind, scope)
                || (scope == DiagnosticScope::Scripts
                    && diagnostic
                        .payload
                        .get("scope")
                        .and_then(serde_json::Value::as_str)
                        != Some("scripts"))
                || (scope == DiagnosticScope::Deployment
                    && diagnostic
                        .payload
                        .get("scope")
                        .and_then(serde_json::Value::as_str)
                        != Some("deployment"))
                || (scope == DiagnosticScope::Env
                    && diagnostic.kind == DiagnosticKind::MissingDocumentation
                    && diagnostic
                        .payload
                        .get("scope")
                        .and_then(serde_json::Value::as_str)
                        != Some("env"))
                || (scope == DiagnosticScope::Runbooks
                    && diagnostic
                        .payload
                        .get("scope")
                        .and_then(serde_json::Value::as_str)
                        != Some("runbooks"))
                || (scope == DiagnosticScope::Docs
                    && diagnostic
                        .payload
                        .get("scope")
                        .and_then(serde_json::Value::as_str)
                        .is_some())
            {
                return false;
            }

            match scope {
                DiagnosticScope::Api => match diagnostic.kind {
                    DiagnosticKind::ApiEndpointImplementedButNotDocumented => {
                        config.fail_on_missing_docs
                    }
                    DiagnosticKind::ApiEndpointDocumentedButNotImplemented => {
                        config.source_of_truth != ApiSourceOfTruth::CodeFirst
                    }
                    DiagnosticKind::ApiRequestSchemaMismatch
                    | DiagnosticKind::ApiResponseSchemaMismatch
                    | DiagnosticKind::ApiExampleInvalid => config.fail_on_openapi_mismatch,
                    DiagnosticKind::ApiStatusCodeUndocumented => {
                        config.fail_on_undocumented_status_code
                    }
                    _ => true,
                },
                DiagnosticScope::Docs
                | DiagnosticScope::Env
                | DiagnosticScope::Scripts
                | DiagnosticScope::Deployment
                | DiagnosticScope::Runbooks
                | DiagnosticScope::RustokFfa
                | DiagnosticScope::RustokFba => true,
            }
        })
        .cloned()
        .collect::<Vec<_>>();
    diagnostics
        .sort_by_key(|diagnostic| (severity_rank(diagnostic.severity), diagnostic.id.0.clone()));

    let mut counts = DiagnosticCounts {
        total: diagnostics.len(),
        ..DiagnosticCounts::default()
    };
    for diagnostic in &diagnostics {
        match diagnostic.severity {
            Severity::Critical => counts.critical += 1,
            Severity::High => counts.high += 1,
            Severity::Medium => counts.medium += 1,
            Severity::Low => counts.low += 1,
        }
    }

    DiagnosticCheckReport {
        schema: "athanor.diagnostic_check.v1".to_string(),
        snapshot,
        scope,
        counts,
        diagnostics,
    }
}

fn affected_path_set(affected_files: &AffectedFileSet) -> BTreeSet<String> {
    affected_files
        .changed
        .iter()
        .chain(affected_files.removed.iter())
        .cloned()
        .collect()
}

fn affected_entity_ids(
    entities: &[Entity],
    affected_paths: &BTreeSet<String>,
) -> HashSet<EntityId> {
    entities
        .iter()
        .filter(|entity| {
            entity
                .ownership
                .iter()
                .any(|ownership| affected_paths.contains(&ownership.source_file))
                || entity
                    .source
                    .as_ref()
                    .is_some_and(|source| affected_paths.contains(&source.path))
        })
        .map(|entity| entity.id.clone())
        .collect()
}

fn diagnostic_touches_paths_or_entities(
    diagnostic: &Diagnostic,
    affected_paths: &BTreeSet<String>,
    affected_entity_ids: &HashSet<EntityId>,
) -> bool {
    diagnostic
        .entities
        .iter()
        .any(|entity| affected_entity_ids.contains(entity))
        || diagnostic
            .ownership
            .iter()
            .any(|ownership| affected_paths.contains(&ownership.source_file))
        || diagnostic.evidence.iter().any(|evidence| {
            evidence
                .source_file
                .as_ref()
                .is_some_and(|path| affected_paths.contains(path))
        })
}

fn diagnostic_counts(diagnostics: &[Diagnostic]) -> DiagnosticCounts {
    let mut counts = DiagnosticCounts {
        total: diagnostics.len(),
        ..DiagnosticCounts::default()
    };
    for diagnostic in diagnostics {
        match diagnostic.severity {
            Severity::Critical => counts.critical += 1,
            Severity::High => counts.high += 1,
            Severity::Medium => counts.medium += 1,
            Severity::Low => counts.low += 1,
        }
    }
    counts
}

fn affected_artifact_statuses(
    root: &Path,
    latest_snapshot: &str,
) -> Result<Vec<AffectedArtifactStatus>> {
    let mut statuses = Vec::new();

    let repair = inspect_repair(RepairInspectOptions {
        root: root.to_path_buf(),
    })
    .context("failed to inspect generated artifact status")?;
    statuses.extend(
        repair
            .issues
            .iter()
            .filter_map(repair_issue_to_artifact_status),
    );

    inspect_snapshot_manifest(
        root,
        ".athanor/generated/current/wiki/manifest.json",
        latest_snapshot,
        AffectedArtifactKind::Wiki,
        "cargo run -p ath --quiet -- wiki .",
        &mut statuses,
    )?;
    inspect_snapshot_manifest(
        root,
        ".athanor/generated/current/html/manifest.json",
        latest_snapshot,
        AffectedArtifactKind::HtmlReport,
        "cargo run -p ath --quiet -- report html .",
        &mut statuses,
    )?;
    let api_contract_stale = inspect_snapshot_manifest(
        root,
        ".athanor/api/latest.json",
        latest_snapshot,
        AffectedArtifactKind::ApiContract,
        "cargo run -p ath --quiet -- api snapshot",
        &mut statuses,
    )?;
    if api_contract_stale && root.join(".athanor/api/diffs").is_dir() {
        statuses.push(AffectedArtifactStatus {
            kind: AffectedArtifactKind::ApiDiff,
            path: PathBuf::from(".athanor/api/diffs"),
            message: "API diff artifacts may not include the latest API contract snapshot"
                .to_string(),
            suggested_command: "cargo run -p ath --quiet -- api diff --from <snapshot> --to <snapshot>"
                .to_string(),
        });
    }

    statuses.sort_by(|left, right| {
        (
            artifact_kind_rank(left.kind),
            left.path.clone(),
            left.message.clone(),
        )
            .cmp(&(
                artifact_kind_rank(right.kind),
                right.path.clone(),
                right.message.clone(),
            ))
    });
    statuses.dedup();
    Ok(statuses)
}

fn affected_documentation_drift(
    snapshot: String,
    entities: &[Entity],
    config: &crate::config::DocsConfig,
    affected_paths: &BTreeSet<String>,
) -> Vec<DriftedDocument> {
    let mut drifted_documents = build_docs_drift_report(snapshot, entities, config)
        .drifted_documents
        .into_iter()
        .filter(|document| affected_paths.contains(&document.path))
        .collect::<Vec<_>>();
    drifted_documents.sort_by(|left, right| {
        (&left.path, &left.stable_key).cmp(&(&right.path, &right.stable_key))
    });
    drifted_documents
}

fn repair_issue_to_artifact_status(issue: &RepairIssue) -> Option<AffectedArtifactStatus> {
    match issue.code.as_str() {
        "missing_generated_current"
        | "invalid_generated_current_schema"
        | "invalid_generated_current_path"
        | "invalid_generated_current_manifest_path"
        | "invalid_generated_current_json"
        | "missing_current_generation"
        | "missing_current_generation_manifest"
        | "invalid_current_generation_manifest_schema"
        | "current_generation_manifest_id_mismatch"
        | "current_generation_manifest_snapshot_mismatch"
        | "stale_current_generation_snapshot" => Some(AffectedArtifactStatus {
            kind: AffectedArtifactKind::GeneratedCurrent,
            path: issue.path.clone(),
            message: issue.message.clone(),
            suggested_command: "cargo run -p ath --quiet -- repair regenerate . --dry-run"
                .to_string(),
        }),
        "orphan_generated_generation" | "stale_generation_snapshot" => {
            Some(AffectedArtifactStatus {
                kind: AffectedArtifactKind::GeneratedGeneration,
                path: issue.path.clone(),
                message: issue.message.clone(),
                suggested_command: "cargo run -p ath --quiet -- repair cleanup . --dry-run"
                    .to_string(),
            })
        }
        _ => None,
    }
}

fn inspect_snapshot_manifest(
    root: &Path,
    relative_path: &str,
    latest_snapshot: &str,
    kind: AffectedArtifactKind,
    suggested_command: &str,
    statuses: &mut Vec<AffectedArtifactStatus>,
) -> Result<bool> {
    let path = root.join(relative_path);
    if !path.exists() {
        return Ok(false);
    }

    let content =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let value: serde_json::Value = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    let snapshot = value.get("snapshot").and_then(serde_json::Value::as_str);
    let stale = snapshot != Some(latest_snapshot);
    if stale {
        let artifact_snapshot = snapshot.unwrap_or("unknown");
        statuses.push(AffectedArtifactStatus {
            kind,
            path: PathBuf::from(relative_path),
            message: format!(
                "artifact was generated from {artifact_snapshot}, latest canonical snapshot is {latest_snapshot}"
            ),
            suggested_command: suggested_command.to_string(),
        });
    }
    Ok(stale)
}

fn artifact_kind_rank(kind: AffectedArtifactKind) -> u8 {
    match kind {
        AffectedArtifactKind::GeneratedCurrent => 0,
        AffectedArtifactKind::GeneratedGeneration => 1,
        AffectedArtifactKind::Wiki => 2,
        AffectedArtifactKind::HtmlReport => 3,
        AffectedArtifactKind::ApiContract => 4,
        AffectedArtifactKind::ApiDiff => 5,
    }
}

pub(crate) fn diagnostic_matches_scope(kind: &DiagnosticKind, scope: DiagnosticScope) -> bool {
    match scope {
        DiagnosticScope::Api => matches!(
            kind,
            DiagnosticKind::OpenapiMismatch
                | DiagnosticKind::DeadEndpoint
                | DiagnosticKind::ApiEndpointMissingInOpenapi
                | DiagnosticKind::ApiEndpointDocumentedButNotImplemented
                | DiagnosticKind::ApiEndpointImplementedButNotDocumented
                | DiagnosticKind::ApiMethodMismatch
                | DiagnosticKind::ApiPathMismatch
                | DiagnosticKind::ApiRequestSchemaMismatch
                | DiagnosticKind::ApiResponseSchemaMismatch
                | DiagnosticKind::ApiStatusCodeUndocumented
                | DiagnosticKind::ApiAuthRequirementMismatch
                | DiagnosticKind::ApiPermissionMismatch
                | DiagnosticKind::ApiExampleInvalid
                | DiagnosticKind::ApiErrorModelMismatch
                | DiagnosticKind::ApiBreakingChangeDetected
        ),
        DiagnosticScope::Docs => matches!(
            kind,
            DiagnosticKind::EmptyDocumentationPage
                | DiagnosticKind::DocumentationPageMissingTitle
                | DiagnosticKind::MissingDocumentation
                | DiagnosticKind::StaleDocumentation
                | DiagnosticKind::DocumentationReferenceUnresolved
                | DiagnosticKind::DuplicateDocumentationId
                | DiagnosticKind::OrphanDoc
                | DiagnosticKind::TranslationOutdated
        ),
        DiagnosticScope::Env => matches!(
            kind,
            DiagnosticKind::MissingEnvVar | DiagnosticKind::MissingDocumentation
        ),
        DiagnosticScope::Scripts => matches!(kind, DiagnosticKind::MissingDocumentation),
        DiagnosticScope::Deployment => matches!(kind, DiagnosticKind::MissingDocumentation),
        DiagnosticScope::Runbooks => matches!(
            kind,
            DiagnosticKind::MissingDocumentation | DiagnosticKind::StaleDocumentation
        ),
        DiagnosticScope::RustokFfa => {
            matches!(kind, DiagnosticKind::Other(value) if value.starts_with("rustok_ffa_"))
        }
        DiagnosticScope::RustokFba => {
            matches!(kind, DiagnosticKind::Other(value) if value.starts_with("rustok_fba_"))
        }
    }
}

fn severity_rank(severity: Severity) -> u8 {
    match severity {
        Severity::Critical => 0,
        Severity::High => 1,
        Severity::Medium => 2,
        Severity::Low => 3,
    }
}

fn sum_counts<const N: usize>(reports: [&DiagnosticCheckReport; N]) -> DiagnosticCounts {
    reports
        .into_iter()
        .fold(DiagnosticCounts::default(), |mut counts, report| {
            counts.total += report.counts.total;
            counts.critical += report.counts.critical;
            counts.high += report.counts.high;
            counts.medium += report.counts.medium;
            counts.low += report.counts.low;
            counts
        })
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use athanor_domain::{DiagnosticId, EntityId, SnapshotId};
    use serde_json::json;

    use super::*;

    #[test]
    fn filters_open_api_diagnostics_and_counts_severity() {
        let diagnostics = vec![
            diagnostic(
                "diag_api_high",
                DiagnosticKind::ApiRequestSchemaMismatch,
                Severity::High,
                DiagnosticStatus::Open,
            ),
            diagnostic(
                "diag_api_medium",
                DiagnosticKind::ApiEndpointImplementedButNotDocumented,
                Severity::Medium,
                DiagnosticStatus::Open,
            ),
            diagnostic(
                "diag_docs",
                DiagnosticKind::DocumentationPageMissingTitle,
                Severity::Medium,
                DiagnosticStatus::Open,
            ),
            diagnostic(
                "diag_resolved",
                DiagnosticKind::ApiPathMismatch,
                Severity::Critical,
                DiagnosticStatus::Resolved,
            ),
        ];

        let report = build_check_report(
            "snap_test".to_string(),
            DiagnosticScope::Api,
            &diagnostics,
            &ApiConfig::default(),
        );

        assert_eq!(report.schema, "athanor.diagnostic_check.v1");
        assert_eq!(report.counts.total, 2);
        assert_eq!(report.counts.high, 1);
        assert_eq!(report.counts.medium, 1);
        assert_eq!(report.diagnostics[0].id.0, "diag_api_high");
    }

    #[test]
    fn reports_stale_snapshot_manifests_for_affected_artifacts() {
        let root = test_root("affected-artifacts");
        write_json(
            &root.join(".athanor/generated/current/wiki/manifest.json"),
            r#"{"schema":"athanor.wiki_manifest.v1","snapshot":"snap_old"}"#,
        );
        write_json(
            &root.join(".athanor/generated/current/html/manifest.json"),
            r#"{"schema":"athanor.html_report_manifest.v1","snapshot":"snap_old"}"#,
        );
        write_json(
            &root.join(".athanor/api/latest.json"),
            r#"{"schema":"athanor.api_contract_latest.v1","snapshot":"snap_old"}"#,
        );
        fs::create_dir_all(root.join(".athanor/api/diffs")).unwrap();

        let mut statuses = Vec::new();
        let wiki_stale = inspect_snapshot_manifest(
            &root,
            ".athanor/generated/current/wiki/manifest.json",
            "snap_new",
            AffectedArtifactKind::Wiki,
            "wiki",
            &mut statuses,
        )
        .unwrap();
        let html_stale = inspect_snapshot_manifest(
            &root,
            ".athanor/generated/current/html/manifest.json",
            "snap_new",
            AffectedArtifactKind::HtmlReport,
            "html",
            &mut statuses,
        )
        .unwrap();
        let api_stale = inspect_snapshot_manifest(
            &root,
            ".athanor/api/latest.json",
            "snap_new",
            AffectedArtifactKind::ApiContract,
            "api snapshot",
            &mut statuses,
        )
        .unwrap();
        if api_stale && root.join(".athanor/api/diffs").is_dir() {
            statuses.push(AffectedArtifactStatus {
                kind: AffectedArtifactKind::ApiDiff,
                path: PathBuf::from(".athanor/api/diffs"),
                message: "API diff artifacts may not include the latest API contract snapshot"
                    .to_string(),
                suggested_command: "api diff".to_string(),
            });
        }

        assert!(wiki_stale);
        assert!(html_stale);
        assert!(api_stale);
        let kinds = statuses
            .iter()
            .map(|status| status.kind)
            .collect::<Vec<_>>();
        assert_eq!(
            kinds,
            vec![
                AffectedArtifactKind::Wiki,
                AffectedArtifactKind::HtmlReport,
                AffectedArtifactKind::ApiContract,
                AffectedArtifactKind::ApiDiff,
            ]
        );
        assert!(statuses[0].message.contains("snap_old"));
        assert!(statuses[0].message.contains("snap_new"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn maps_repair_issues_to_affected_artifact_statuses() {
        let generated = repair_issue_to_artifact_status(&RepairIssue {
            code: "stale_current_generation_snapshot".to_string(),
            path: PathBuf::from(".athanor/generated/current.json"),
            message: "generated is stale".to_string(),
        })
        .unwrap();
        let orphan = repair_issue_to_artifact_status(&RepairIssue {
            code: "orphan_generated_generation".to_string(),
            path: PathBuf::from(".athanor/generated/generations/00000001"),
            message: "orphan generation".to_string(),
        })
        .unwrap();
        let unrelated = repair_issue_to_artifact_status(&RepairIssue {
            code: "missing_canonical_latest".to_string(),
            path: PathBuf::from(".athanor/store/canonical/jsonl/latest.json"),
            message: "canonical latest missing".to_string(),
        });

        assert_eq!(generated.kind, AffectedArtifactKind::GeneratedCurrent);
        assert_eq!(orphan.kind, AffectedArtifactKind::GeneratedGeneration);
        assert!(generated.suggested_command.contains("repair regenerate"));
        assert!(orphan.suggested_command.contains("repair cleanup"));
        assert!(unrelated.is_none());
    }

    fn write_json(path: &Path, content: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    fn test_root(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = env::temp_dir().join(format!("athanor-{name}-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn filters_documentation_diagnostics() {
        let mut script = diagnostic(
            "diag_script",
            DiagnosticKind::MissingDocumentation,
            Severity::Medium,
            DiagnosticStatus::Open,
        );
        script.payload = json!({"scope": "scripts"});
        let mut deployment = diagnostic(
            "diag_deployment",
            DiagnosticKind::MissingDocumentation,
            Severity::Medium,
            DiagnosticStatus::Open,
        );
        deployment.payload = json!({"scope": "deployment"});
        let diagnostics = vec![
            diagnostic(
                "diag_docs",
                DiagnosticKind::EmptyDocumentationPage,
                Severity::High,
                DiagnosticStatus::Open,
            ),
            diagnostic(
                "diag_docs_reference",
                DiagnosticKind::DocumentationReferenceUnresolved,
                Severity::Medium,
                DiagnosticStatus::Open,
            ),
            diagnostic(
                "diag_api",
                DiagnosticKind::ApiMethodMismatch,
                Severity::High,
                DiagnosticStatus::Open,
            ),
            script,
            deployment,
        ];

        let report = build_check_report(
            "snap_test".to_string(),
            DiagnosticScope::Docs,
            &diagnostics,
            &ApiConfig::default(),
        );

        assert_eq!(report.counts.total, 2);
        assert_eq!(report.diagnostics[0].id.0, "diag_docs");
    }

    #[test]
    fn filters_environment_diagnostics() {
        let mut config = diagnostic(
            "diag_config",
            DiagnosticKind::MissingDocumentation,
            Severity::Medium,
            DiagnosticStatus::Open,
        );
        config.payload = json!({"scope": "env"});
        let mut script = diagnostic(
            "diag_script",
            DiagnosticKind::MissingDocumentation,
            Severity::Medium,
            DiagnosticStatus::Open,
        );
        script.payload = json!({"scope": "scripts"});
        let diagnostics = vec![
            diagnostic(
                "diag_env",
                DiagnosticKind::MissingEnvVar,
                Severity::Medium,
                DiagnosticStatus::Open,
            ),
            config,
            script,
            diagnostic(
                "diag_docs",
                DiagnosticKind::DocumentationPageMissingTitle,
                Severity::Medium,
                DiagnosticStatus::Open,
            ),
        ];

        let report = build_check_report(
            "snap_test".to_string(),
            DiagnosticScope::Env,
            &diagnostics,
            &ApiConfig::default(),
        );

        assert_eq!(report.counts.total, 2);
        assert_eq!(
            report
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.id.0.as_str())
                .collect::<Vec<_>>(),
            vec!["diag_config", "diag_env"]
        );
    }

    #[test]
    fn filters_affected_diagnostics_by_changed_files_and_entities() {
        let changed = entity("ent_changed", "docs/changed.md");
        let unrelated = entity("ent_unrelated", "docs/unrelated.md");
        let affected_paths = BTreeSet::from(["docs/changed.md".to_string()]);
        let affected_entity_ids =
            affected_entity_ids(&[changed.clone(), unrelated.clone()], &affected_paths);

        let mut attached = diagnostic(
            "diag_attached",
            DiagnosticKind::MissingDocumentation,
            Severity::Medium,
            DiagnosticStatus::Open,
        );
        attached.entities = vec![changed.id.clone()];
        let mut evidence = diagnostic(
            "diag_evidence",
            DiagnosticKind::MissingDocumentation,
            Severity::Medium,
            DiagnosticStatus::Open,
        );
        evidence.evidence = vec![athanor_domain::Evidence {
            source_file: Some("docs/changed.md".to_string()),
            line_start: Some(1),
            line_end: Some(1),
            extractor: Some("test".to_string()),
            commit_hash: None,
            confidence: 1.0,
            status: athanor_domain::EvidenceStatus::Missing,
        }];
        let mut other = diagnostic(
            "diag_other",
            DiagnosticKind::MissingDocumentation,
            Severity::Medium,
            DiagnosticStatus::Open,
        );
        other.entities = vec![unrelated.id.clone()];

        assert!(diagnostic_touches_paths_or_entities(
            &attached,
            &affected_paths,
            &affected_entity_ids
        ));
        assert!(diagnostic_touches_paths_or_entities(
            &evidence,
            &affected_paths,
            &affected_entity_ids
        ));
        assert!(!diagnostic_touches_paths_or_entities(
            &other,
            &affected_paths,
            &affected_entity_ids
        ));
    }

    #[test]
    fn reports_documentation_drift_only_for_affected_documents() {
        let changed = editable_doc("ent_changed_doc", "docs/changed.md", Some("snap_old"));
        let unchanged = editable_doc("ent_unchanged_doc", "docs/unchanged.md", Some("snap_old"));
        let current = editable_doc("ent_current_doc", "docs/current.md", Some("snap_current"));
        let affected_paths =
            BTreeSet::from(["docs/changed.md".to_string(), "docs/current.md".to_string()]);

        let drift = affected_documentation_drift(
            "snap_current".to_string(),
            &[changed, unchanged, current],
            &crate::config::DocsConfig::default(),
            &affected_paths,
        );

        assert_eq!(drift.len(), 1);
        assert_eq!(drift[0].path, "docs/changed.md");
        assert_eq!(drift[0].verified_snapshot.as_deref(), Some("snap_old"));
    }

    #[test]
    fn filters_script_diagnostics_by_payload_scope() {
        let mut script = diagnostic(
            "diag_script",
            DiagnosticKind::MissingDocumentation,
            Severity::Medium,
            DiagnosticStatus::Open,
        );
        script.payload = json!({"scope": "scripts"});
        let docs = diagnostic(
            "diag_docs",
            DiagnosticKind::MissingDocumentation,
            Severity::Medium,
            DiagnosticStatus::Open,
        );

        let report = build_check_report(
            "snap_test".to_string(),
            DiagnosticScope::Scripts,
            &[docs, script],
            &ApiConfig::default(),
        );

        assert_eq!(report.counts.total, 1);
        assert_eq!(report.diagnostics[0].id.0, "diag_script");
    }

    #[test]
    fn filters_deployment_diagnostics_by_payload_scope() {
        let mut deployment = diagnostic(
            "diag_deployment",
            DiagnosticKind::MissingDocumentation,
            Severity::Medium,
            DiagnosticStatus::Open,
        );
        deployment.payload = json!({"scope": "deployment"});
        let docs = diagnostic(
            "diag_docs",
            DiagnosticKind::MissingDocumentation,
            Severity::Medium,
            DiagnosticStatus::Open,
        );

        let report = build_check_report(
            "snap_test".to_string(),
            DiagnosticScope::Deployment,
            &[docs, deployment],
            &ApiConfig::default(),
        );

        assert_eq!(report.counts.total, 1);
        assert_eq!(report.diagnostics[0].id.0, "diag_deployment");
    }

    #[test]
    fn filters_runbook_diagnostics_by_payload_scope() {
        let mut runbook = diagnostic(
            "diag_runbook",
            DiagnosticKind::StaleDocumentation,
            Severity::Medium,
            DiagnosticStatus::Open,
        );
        runbook.payload = json!({"scope": "runbooks"});
        let docs = diagnostic(
            "diag_docs",
            DiagnosticKind::StaleDocumentation,
            Severity::Medium,
            DiagnosticStatus::Open,
        );

        let report = build_check_report(
            "snap_test".to_string(),
            DiagnosticScope::Runbooks,
            &[docs, runbook],
            &ApiConfig::default(),
        );

        assert_eq!(report.counts.total, 1);
        assert_eq!(report.diagnostics[0].id.0, "diag_runbook");
    }

    #[test]
    fn openapi_first_policy_filters_out_missing_docs_unless_fail_on_missing_docs() {
        let diagnostics = vec![
            diagnostic(
                "diag_missing_docs",
                DiagnosticKind::ApiEndpointImplementedButNotDocumented,
                Severity::Medium,
                DiagnosticStatus::Open,
            ),
            diagnostic(
                "diag_missing_impl",
                DiagnosticKind::ApiEndpointDocumentedButNotImplemented,
                Severity::High,
                DiagnosticStatus::Open,
            ),
        ];

        // Case 1: openapi-first and fail_on_missing_docs is false
        let mut config = ApiConfig {
            source_of_truth: ApiSourceOfTruth::OpenapiFirst,
            fail_on_missing_docs: false,
            ..Default::default()
        };
        let report = build_check_report(
            "snap_test".to_string(),
            DiagnosticScope::Api,
            &diagnostics,
            &config,
        );
        assert_eq!(report.counts.total, 1);
        assert_eq!(report.diagnostics[0].id.0, "diag_missing_impl");

        // Case 2: openapi-first and fail_on_missing_docs is true
        config.fail_on_missing_docs = true;
        let report = build_check_report(
            "snap_test".to_string(),
            DiagnosticScope::Api,
            &diagnostics,
            &config,
        );
        assert_eq!(report.counts.total, 2);
    }

    #[test]
    fn code_first_policy_filters_out_missing_implementations() {
        let diagnostics = vec![diagnostic(
            "diag_missing_impl",
            DiagnosticKind::ApiEndpointDocumentedButNotImplemented,
            Severity::High,
            DiagnosticStatus::Open,
        )];

        let config = ApiConfig {
            source_of_truth: ApiSourceOfTruth::CodeFirst,
            ..Default::default()
        };
        let report = build_check_report(
            "snap_test".to_string(),
            DiagnosticScope::Api,
            &diagnostics,
            &config,
        );
        assert_eq!(report.counts.total, 0);
    }

    #[test]
    fn fail_on_openapi_mismatch_filters_out_mismatches() {
        let diagnostics = vec![diagnostic(
            "diag_mismatch",
            DiagnosticKind::ApiRequestSchemaMismatch,
            Severity::High,
            DiagnosticStatus::Open,
        )];

        let config = ApiConfig {
            fail_on_openapi_mismatch: false,
            ..Default::default()
        };
        let report = build_check_report(
            "snap_test".to_string(),
            DiagnosticScope::Api,
            &diagnostics,
            &config,
        );
        assert_eq!(report.counts.total, 0);
    }

    #[test]
    fn fail_on_undocumented_status_code_filters_out_undocumented_codes() {
        let diagnostics = vec![diagnostic(
            "diag_code",
            DiagnosticKind::ApiStatusCodeUndocumented,
            Severity::Medium,
            DiagnosticStatus::Open,
        )];

        let config = ApiConfig {
            fail_on_undocumented_status_code: false,
            ..Default::default()
        };
        let report = build_check_report(
            "snap_test".to_string(),
            DiagnosticScope::Api,
            &diagnostics,
            &config,
        );
        assert_eq!(report.counts.total, 0);
    }

    fn diagnostic(
        id: &str,
        kind: DiagnosticKind,
        severity: Severity,
        status: DiagnosticStatus,
    ) -> Diagnostic {
        Diagnostic {
            id: DiagnosticId(id.to_string()),
            kind,
            severity,
            status,
            title: id.to_string(),
            message: id.to_string(),
            entities: vec![EntityId("ent_test".to_string())],
            evidence: Vec::new(),
            ownership: Vec::new(),
            snapshot: SnapshotId("snap_test".to_string()),
            suggested_fix: None,
            payload: json!({}),
        }
    }

    fn entity(id: &str, path: &str) -> Entity {
        Entity {
            id: EntityId(id.to_string()),
            stable_key: athanor_domain::StableKey(format!("doc://{path}")),
            kind: athanor_domain::EntityKind::DocumentationPage,
            name: path.to_string(),
            title: None,
            source: Some(athanor_domain::SourceLocation {
                path: path.to_string(),
                line_start: Some(1),
                line_end: Some(1),
            }),
            language: None,
            aliases: Vec::new(),
            ownership: vec![athanor_domain::Ownership {
                source_file: path.to_string(),
            }],
            payload: json!({}),
        }
    }

    fn editable_doc(id: &str, path: &str, verified_snapshot: Option<&str>) -> Entity {
        let mut entity = entity(id, path);
        entity.payload = json!({
            "documentation_layer": "editable",
            "last_verified_snapshot": verified_snapshot,
        });
        entity
    }
}
