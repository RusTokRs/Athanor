use anyhow::{Result, bail};

mod retention;

pub use retention::{
    CanonicalRepairState, GeneratedRepairState, IndexGenerationCleanupOptions,
    IndexGenerationCleanupReport, IndexGenerationCleanupRow, RepairApplyOptions, RepairApplyReport,
    RepairCleanupOptions, RepairCleanupRemoval, RepairCleanupRemovalKind, RepairCleanupReport,
    RepairCleanupRetained, RepairInspectOptions, RepairInspectReport, RepairIssue,
    RepairRecoverCanonicalOptions, RepairRecoverCanonicalReport, RepairRegenerateOptions,
    RepairRegenerateReport, RepairStatus, apply_repair, cleanup_repair, recover_canonical_repair,
    regenerate_repair,
};

const POINTER_PATH: &str = ".athanor/state/index-current.json";

/// Distinguishes an unpointed canonical-latest generation from a disposable orphan.
pub fn inspect_repair(options: RepairInspectOptions) -> Result<RepairInspectReport> {
    let mut report = retention::inspect_repair(options)?;
    if report.root.join(POINTER_PATH).exists() {
        return Ok(report);
    }
    let Some(latest) = report.canonical.latest_snapshot.as_deref() else {
        return Ok(report);
    };
    let recoverable = format!("gen_{latest}");
    for issue in &mut report.issues {
        if issue.code == "orphan_index_generation"
            && issue.path.file_name().and_then(|name| name.to_str()) == Some(recoverable.as_str())
        {
            issue.code = "recoverable_index_generation".to_string();
            issue.message = format!(
                "immutable index generation {recoverable} matches canonical latest and can restore index-current; recover the pointer before cleanup"
            );
        }
    }
    Ok(report)
}

pub fn cleanup_index_generations(
    options: IndexGenerationCleanupOptions,
) -> Result<IndexGenerationCleanupReport> {
    let inspection = inspect_repair(RepairInspectOptions {
        root: options.root.clone(),
    })?;
    if let Some(recoverable) = inspection
        .issues
        .iter()
        .find(|issue| issue.code == "recoverable_index_generation")
    {
        bail!(
            "refusing index-generation cleanup while a recoverable canonical-latest generation is unpointed: {} ({})",
            recoverable.message,
            recoverable.path.display()
        );
    }
    retention::cleanup_index_generations(options)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use serde_json::json;

    use super::*;

    #[test]
    fn canonical_latest_generation_is_recoverable_not_removable() {
        let root = test_root();
        let snapshot = "snap_current";
        let generation = format!("gen_{snapshot}");
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
        let read_model = root
            .join(".athanor/generated/index-generations")
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
            root.join(".athanor/state/index-state-gen_snap_current.json"),
            serde_json::to_vec_pretty(&json!({
                "schema": crate::index_state::INDEX_STATE_SCHEMA,
                "snapshot": snapshot,
                "generation": "gen_snap_current",
                "files": {}
            }))
            .unwrap(),
        )
        .unwrap();

        let report = inspect_repair(RepairInspectOptions { root: root.clone() }).unwrap();
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "recoverable_index_generation")
        );
        let error = cleanup_index_generations(IndexGenerationCleanupOptions {
            root: root.clone(),
            dry_run: true,
            keep: 0,
            confirmation_token: None,
        })
        .expect_err("recoverable generation must not enter cleanup planning");
        assert!(error.to_string().contains("recoverable canonical-latest"));
        assert!(read_model.is_dir());
        fs::remove_dir_all(root).unwrap();
    }

    fn test_root() -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("athanor-recoverable-index-{nonce}"));
        fs::create_dir_all(&root).unwrap();
        root
    }
}
