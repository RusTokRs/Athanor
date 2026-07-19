use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use athanor_core::CanonicalSnapshot;
use athanor_domain::{Diagnostic, DiagnosticStatus, Entity, EntityId};

use crate::config::ProjectConfig;
use crate::docs::{DriftedDocument, build_docs_drift_report};
use crate::index_state::{AffectedFileSet, IndexStateStore};
use crate::json_contract::AFFECTED_CHECK_SCHEMA_V1;
use crate::local_source::discover_source_files;
use crate::repair::{RepairInspectOptions, RepairIssue, inspect_repair};

use super::diagnostics::{diagnostic_counts, severity_rank};
use super::model::{
    AffectedArtifactKind, AffectedArtifactStatus, AffectedCheckReport, AffectedFileCounts,
};

pub(super) fn build_affected_check_report(
    root: PathBuf,
    snapshot_id: String,
    snapshot: &CanonicalSnapshot,
    config: &ProjectConfig,
) -> Result<AffectedCheckReport> {
    let state_store = IndexStateStore::new(root.join(".athanor/state/index-state.json"));
    let previous_state = state_store.load().context("failed to load index state")?;
    let current_files = discover_source_files(&root)
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
    diagnostics.sort_by_key(|diagnostic| {
        (severity_rank(diagnostic.severity), diagnostic.id.0.clone())
    });

    Ok(AffectedCheckReport {
        schema: AFFECTED_CHECK_SCHEMA_V1.to_string(),
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
        counts: diagnostic_counts(&diagnostics),
        diagnostics,
    })
}

fn affected_path_set(affected_files: &AffectedFileSet) -> BTreeSet<String> {
    affected_files
        .changed
        .iter()
        .chain(affected_files.removed.iter())
        .cloned()
        .collect()
}

pub(super) fn affected_entity_ids(
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

pub(super) fn diagnostic_touches_paths_or_entities(
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

pub(super) fn affected_documentation_drift(
    snapshot: String,
    entities: &[Entity],
    config: &crate::config::DocsConfig,
    affected_paths: &BTreeSet<String>,
) -> Vec<DriftedDocument> {
    let mut drifted_documents = build_docs_drift_report(snapshot, None, entities, config)
        .drifted_documents
        .into_iter()
        .filter(|document| affected_paths.contains(&document.path))
        .collect::<Vec<_>>();
    drifted_documents.sort_by(|left, right| {
        (&left.path, &left.stable_key).cmp(&(&right.path, &right.stable_key))
    });
    drifted_documents
}

pub(super) fn repair_issue_to_artifact_status(
    issue: &RepairIssue,
) -> Option<AffectedArtifactStatus> {
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
                suggested_command:
                    "cargo run -p ath --quiet -- repair cleanup . --generated-only --dry-run"
                        .to_string(),
            })
        }
        _ => None,
    }
}

pub(super) fn inspect_snapshot_manifest(
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
