use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Serialize;
use sha2::{Digest, Sha256};

mod guard;

pub use guard::*;

const INDEX_RETENTION_SCHEMA: &str = "athanor.index_generation_cleanup.v1";
const INDEX_GENERATIONS_PATH: &str = ".athanor/generated/index-generations";
const INDEX_STATE_PREFIX: &str = "index-state-";
const INDEX_STATE_SUFFIX: &str = ".json";

/// Explicit two-step cleanup policy for immutable transactional index generations.
#[derive(Debug, Clone)]
pub struct IndexGenerationCleanupOptions {
    pub root: PathBuf,
    pub dry_run: bool,
    pub keep: usize,
    /// Token returned by a dry-run over the exact same root, retention count, and orphan set.
    pub confirmation_token: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IndexGenerationCleanupRow {
    pub generation: String,
    pub read_model: PathBuf,
    pub index_state: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexGenerationCleanupReport {
    pub schema: String,
    pub root: PathBuf,
    pub dry_run: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmation_token: Option<String>,
    pub removed: Vec<IndexGenerationCleanupRow>,
    pub retained: Vec<IndexGenerationCleanupRow>,
    pub remaining_issues: Vec<RepairIssue>,
    pub inspection: RepairInspectReport,
}

/// Plans or applies immutable index-generation retention.
///
/// A destructive call must provide the confirmation token emitted by a dry-run over the same
/// canonical root, `keep` value, and exact orphan generation set. Pointed, pending, incomplete,
/// corrupt, or canonically unresolved publication state is never eligible for removal.
pub fn cleanup_index_generations(
    options: IndexGenerationCleanupOptions,
) -> Result<IndexGenerationCleanupReport> {
    let inspection = inspect_repair(RepairInspectOptions {
        root: options.root.clone(),
    })?;
    reject_unsafe_index_cleanup(&inspection)?;

    let root = inspection.root.clone();
    let mut candidates = orphan_rows(&inspection);
    candidates.sort_by(|left, right| left.generation.cmp(&right.generation));
    let remove_count = candidates.len().saturating_sub(options.keep);
    let retained = candidates.split_off(remove_count);
    let removed = candidates;
    let expected_token =
        (!removed.is_empty()).then(|| confirmation_token(&root, options.keep, &removed));

    if !options.dry_run && !removed.is_empty() {
        if options.confirmation_token.as_deref() != expected_token.as_deref() {
            bail!(
                "refusing index-generation cleanup without the matching dry-run confirmation token"
            );
        }
        for row in &removed {
            remove_generation_pair(&root, row)?;
        }
    }

    let remaining_inspection = if options.dry_run || removed.is_empty() {
        inspection.clone()
    } else {
        inspect_repair(RepairInspectOptions { root: root.clone() })?
    };

    Ok(IndexGenerationCleanupReport {
        schema: INDEX_RETENTION_SCHEMA.to_string(),
        root,
        dry_run: options.dry_run,
        confirmation_token: options.dry_run.then_some(expected_token).flatten(),
        removed,
        retained,
        remaining_issues: remaining_inspection.issues.clone(),
        inspection: remaining_inspection,
    })
}

fn reject_unsafe_index_cleanup(inspection: &RepairInspectReport) -> Result<()> {
    if let Some(blocker) = inspection.issues.iter().find(|issue| {
        issue.code == "pending_index_current_publication"
            || issue.code == "incomplete_index_generation"
            || issue.code == "stale_index_current_snapshot"
            || issue.code == "missing_index_current_canonical_snapshot"
            || issue.code.starts_with("invalid_index_current")
            || issue.code.starts_with("missing_index_current")
            || issue.code.starts_with("index_current_")
    }) {
        bail!(
            "refusing index-generation cleanup while publication state is unsafe: {} ({})",
            blocker.message,
            blocker.path.display()
        );
    }
    Ok(())
}

fn orphan_rows(inspection: &RepairInspectReport) -> Vec<IndexGenerationCleanupRow> {
    let mut rows = BTreeMap::new();
    for issue in &inspection.issues {
        if issue.code != "orphan_index_generation" {
            continue;
        }
        let Some(generation) = issue.path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !generation.starts_with("gen_") {
            continue;
        }
        rows.entry(generation.to_string())
            .or_insert_with(|| generation_row(&inspection.root, generation));
    }
    rows.into_values().collect()
}

fn generation_row(root: &Path, generation: &str) -> IndexGenerationCleanupRow {
    IndexGenerationCleanupRow {
        generation: generation.to_string(),
        read_model: root.join(INDEX_GENERATIONS_PATH).join(generation),
        index_state: root.join(".athanor/state").join(format!(
            "{INDEX_STATE_PREFIX}{generation}{INDEX_STATE_SUFFIX}"
        )),
    }
}

fn confirmation_token(root: &Path, keep: usize, rows: &[IndexGenerationCleanupRow]) -> String {
    let mut digest = Sha256::new();
    digest.update(INDEX_RETENTION_SCHEMA.as_bytes());
    digest.update([0]);
    digest.update(root.as_os_str().to_string_lossy().as_bytes());
    digest.update([0]);
    digest.update(keep.to_string().as_bytes());
    for row in rows {
        digest.update([0]);
        digest.update(row.generation.as_bytes());
    }
    format!("sha256:{:x}", digest.finalize())
}

fn remove_generation_pair(root: &Path, row: &IndexGenerationCleanupRow) -> Result<()> {
    let read_root = root.join(INDEX_GENERATIONS_PATH);
    let state_root = root.join(".athanor/state");
    ensure_direct_child(&read_root, &row.read_model, true)?;
    ensure_direct_child(&state_root, &row.index_state, false)?;

    let nonce = cleanup_nonce();
    let read_tombstone = read_root.join(format!(".cleanup-{}-{nonce}", row.generation));
    let state_tombstone = state_root.join(format!(".cleanup-{}-{nonce}.json", row.generation));
    fs::rename(&row.read_model, &read_tombstone).with_context(|| {
        format!(
            "failed to stage index read-model generation {} for cleanup",
            row.read_model.display()
        )
    })?;
    if let Err(error) = fs::rename(&row.index_state, &state_tombstone) {
        let _ = fs::rename(&read_tombstone, &row.read_model);
        return Err(error).with_context(|| {
            format!(
                "failed to stage index state generation {} for cleanup",
                row.index_state.display()
            )
        });
    }

    fs::remove_dir_all(&read_tombstone).with_context(|| {
        format!(
            "failed to remove staged index read-model generation {}",
            read_tombstone.display()
        )
    })?;
    fs::remove_file(&state_tombstone).with_context(|| {
        format!(
            "failed to remove staged index state generation {}",
            state_tombstone.display()
        )
    })
}

fn ensure_direct_child(root: &Path, path: &Path, directory: bool) -> Result<()> {
    let root = root
        .canonicalize()
        .with_context(|| format!("failed to canonicalize cleanup root {}", root.display()))?;
    let path = path
        .canonicalize()
        .with_context(|| format!("failed to canonicalize cleanup target {}", path.display()))?;
    if path.parent() != Some(root.as_path()) {
        bail!(
            "refusing to remove index generation target {} outside direct cleanup root {}",
            path.display(),
            root.display()
        );
    }
    if directory && !path.is_dir() {
        bail!(
            "index generation target is not a directory: {}",
            path.display()
        );
    }
    if !directory && !path.is_file() {
        bail!("index generation target is not a file: {}", path.display());
    }
    Ok(())
}

fn cleanup_nonce() -> String {
    format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    )
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::json;

    use super::*;

    #[test]
    fn dry_run_token_is_required_for_the_exact_plan() {
        let root = test_root("token");
        write_canonical(&root, "snap_current");
        write_generation(&root, "snap_orphan", false);

        let plan = cleanup_index_generations(IndexGenerationCleanupOptions {
            root: root.clone(),
            dry_run: true,
            keep: 0,
            confirmation_token: None,
        })
        .unwrap();
        let token = plan.confirmation_token.clone().unwrap();
        assert_eq!(plan.removed.len(), 1);
        assert!(plan.removed[0].read_model.is_dir());

        let error = cleanup_index_generations(IndexGenerationCleanupOptions {
            root: root.clone(),
            dry_run: false,
            keep: 0,
            confirmation_token: Some("sha256:wrong".to_string()),
        })
        .expect_err("changed or absent confirmation must fail closed");
        assert!(error.to_string().contains("matching dry-run confirmation"));

        let applied = cleanup_index_generations(IndexGenerationCleanupOptions {
            root: root.clone(),
            dry_run: false,
            keep: 0,
            confirmation_token: Some(token),
        })
        .unwrap();
        assert_eq!(applied.removed.len(), 1);
        assert!(!applied.removed[0].read_model.exists());
        assert!(!applied.removed[0].index_state.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn corruption_matrix_fails_closed() {
        for (case, expected) in [
            ("schema", "invalid_index_current_schema"),
            ("path", "invalid_index_current_read_model_path"),
            ("stale", "stale_index_current_snapshot"),
            ("generation", "index_current_generation_mismatch"),
            ("manifest", "missing_index_current_manifest"),
            ("state", "missing_index_current_state"),
            ("journal", "invalid_index_current_publication_json"),
            ("half", "incomplete_index_generation"),
        ] {
            let root = test_root(case);
            write_canonical(&root, "snap_current");
            match case {
                "schema" => write_generation_with_pointer(
                    &root,
                    "snap_current",
                    json!({
                        "schema": "athanor.index_current.v999",
                        "generation": "gen_snap_current",
                        "snapshot": "snap_current",
                        "read_model": ".athanor/generated/index-generations/gen_snap_current/jsonl",
                        "index_state": ".athanor/state/index-state-gen_snap_current.json"
                    }),
                ),
                "path" => write_generation_with_pointer(
                    &root,
                    "snap_current",
                    json!({
                        "schema": crate::index_current::INDEX_CURRENT_SCHEMA,
                        "generation": "gen_snap_current",
                        "snapshot": "snap_current",
                        "read_model": "../foreign",
                        "index_state": ".athanor/state/index-state-gen_snap_current.json"
                    }),
                ),
                "stale" => {
                    write_generation(&root, "snap_old", false);
                    write_pointer(&root, "snap_old", "gen_snap_old");
                }
                "generation" => {
                    write_generation(&root, "snap_current", false);
                    write_pointer(&root, "snap_current", "gen_foreign");
                }
                "manifest" => {
                    write_generation(&root, "snap_current", true);
                    fs::remove_file(root.join(
                        ".athanor/generated/index-generations/gen_snap_current/jsonl/manifest.json",
                    ))
                    .unwrap();
                }
                "state" => {
                    write_generation(&root, "snap_current", true);
                    fs::remove_file(root.join(".athanor/state/index-state-gen_snap_current.json"))
                        .unwrap();
                }
                "journal" => {
                    fs::create_dir_all(root.join(".athanor/state")).unwrap();
                    fs::write(
                        root.join(".athanor/state/index-current-publication.json"),
                        "{",
                    )
                    .unwrap();
                }
                "half" => {
                    let read_model =
                        root.join(".athanor/generated/index-generations/gen_snap_half/jsonl");
                    fs::create_dir_all(&read_model).unwrap();
                    fs::write(
                        read_model.join("manifest.json"),
                        serde_json::to_vec_pretty(&json!({
                            "schema": crate::read_model::JSONL_MANIFEST_SCHEMA,
                            "snapshot": "snap_half",
                            "generation": "gen_snap_half"
                        }))
                        .unwrap(),
                    )
                    .unwrap();
                }
                _ => unreachable!(),
            }

            let report = inspect_repair(RepairInspectOptions { root: root.clone() }).unwrap();
            assert!(
                report.issues.iter().any(|issue| issue.code == expected),
                "case {case} did not report {expected}: {:?}",
                report
                    .issues
                    .iter()
                    .map(|issue| issue.code.as_str())
                    .collect::<Vec<_>>()
            );
            fs::remove_dir_all(root).unwrap();
        }
    }

    fn write_generation_with_pointer(root: &Path, snapshot: &str, pointer: serde_json::Value) {
        write_generation(root, snapshot, false);
        fs::write(
            root.join(".athanor/state/index-current.json"),
            serde_json::to_vec_pretty(&pointer).unwrap(),
        )
        .unwrap();
    }

    fn write_generation(root: &Path, snapshot: &str, pointer: bool) {
        let generation = format!("gen_{snapshot}");
        let read_model = root
            .join(INDEX_GENERATIONS_PATH)
            .join(&generation)
            .join("jsonl");
        fs::create_dir_all(&read_model).unwrap();
        fs::create_dir_all(root.join(".athanor/state")).unwrap();
        fs::write(
            read_model.join("manifest.json"),
            serde_json::to_vec_pretty(&json!({
                "schema": crate::read_model::JSONL_MANIFEST_SCHEMA,
                "snapshot": snapshot,
                "generation": generation
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            root.join(".athanor/state")
                .join(format!("index-state-gen_{snapshot}.json")),
            serde_json::to_vec_pretty(&json!({
                "schema": crate::index_state::INDEX_STATE_SCHEMA,
                "snapshot": snapshot,
                "generation": format!("gen_{snapshot}"),
                "files": {}
            }))
            .unwrap(),
        )
        .unwrap();
        if pointer {
            write_pointer(root, snapshot, &format!("gen_{snapshot}"));
        }
    }

    fn write_pointer(root: &Path, snapshot: &str, generation: &str) {
        fs::write(
            root.join(".athanor/state/index-current.json"),
            serde_json::to_vec_pretty(&json!({
                "schema": crate::index_current::INDEX_CURRENT_SCHEMA,
                "generation": generation,
                "snapshot": snapshot,
                "read_model": format!(
                    ".athanor/generated/index-generations/{generation}/jsonl"
                ),
                "index_state": format!(".athanor/state/index-state-{generation}.json")
            }))
            .unwrap(),
        )
        .unwrap();
    }

    fn write_canonical(root: &Path, snapshot: &str) {
        let canonical = root.join(".athanor/store/canonical/jsonl");
        fs::create_dir_all(canonical.join("snapshots").join(snapshot)).unwrap();
        fs::write(
            canonical.join("latest.json"),
            serde_json::to_vec_pretty(&json!({ "snapshot": snapshot })).unwrap(),
        )
        .unwrap();
        fs::write(
            canonical
                .join("snapshots")
                .join(snapshot)
                .join("manifest.json"),
            serde_json::to_vec_pretty(&json!({
                "schema": "athanor.canonical_snapshot.v1",
                "snapshot": snapshot
            }))
            .unwrap(),
        )
        .unwrap();
    }

    fn test_root(label: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("athanor-index-retention-{label}-{nonce}"));
        fs::create_dir_all(&root).unwrap();
        root
    }
}
