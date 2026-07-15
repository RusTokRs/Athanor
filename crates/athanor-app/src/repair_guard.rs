use std::path::Path;

use anyhow::{Result, bail};

mod current {
    include!("repair_pointer.rs");
}

pub use current::{
    CanonicalRepairState, GeneratedRepairState, RepairApplyOptions, RepairApplyReport,
    RepairCleanupOptions, RepairCleanupRemoval, RepairCleanupRemovalKind, RepairCleanupReport,
    RepairCleanupRetained, RepairInspectOptions, RepairInspectReport, RepairIssue,
    RepairRecoverCanonicalOptions, RepairRecoverCanonicalReport, RepairRegenerateOptions,
    RepairRegenerateReport, RepairStatus, recover_canonical_repair, regenerate_repair,
};

const POINTER_PATH: &str = ".athanor/state/index-current.json";

pub fn inspect_repair(options: RepairInspectOptions) -> Result<RepairInspectReport> {
    let mut report = current::inspect_repair(options)?;
    let pointer_exists = report.root.join(POINTER_PATH).is_file();
    let canonical_pointer_invalid = report.issues.iter().any(|issue| {
        matches!(
            issue.code.as_str(),
            "missing_latest_snapshot" | "missing_latest_pointer"
        ) || (issue.code == "invalid_json"
            && issue
                .path
                .ends_with(Path::new(".athanor/store/canonical/jsonl/latest.json")))
    });
    if pointer_exists && (report.canonical.latest_snapshot.is_none() || canonical_pointer_invalid) {
        report.issues.push(RepairIssue {
            code: "missing_index_current_canonical_snapshot".to_string(),
            path: report
                .root
                .join(".athanor/store/canonical/jsonl/latest.json"),
            message: "index-current exists but its authoritative canonical latest snapshot cannot be validated"
                .to_string(),
        });
        report.status = RepairStatus::NeedsRepair;
    }
    Ok(report)
}

pub fn cleanup_repair(options: RepairCleanupOptions) -> Result<RepairCleanupReport> {
    let preflight = inspect_repair(RepairInspectOptions {
        root: options.root.clone(),
    })?;
    if let Some(blocker) = preflight
        .issues
        .iter()
        .find(|issue| issue.code == "missing_index_current_canonical_snapshot")
    {
        bail!(
            "refusing repair cleanup while index publication state is unsafe: {} ({})",
            blocker.message,
            blocker.path.display()
        );
    }
    let mut report = current::cleanup_repair(options)?;
    let inspection = inspect_repair(RepairInspectOptions {
        root: report.root.clone(),
    })?;
    report.remaining_issues = inspection.issues.clone();
    report.inspection = inspection;
    Ok(report)
}

pub async fn apply_repair(options: RepairApplyOptions) -> Result<RepairApplyReport> {
    let canonical = recover_canonical_repair(RepairRecoverCanonicalOptions {
        root: options.root.clone(),
        dry_run: options.dry_run,
    })?;
    let root = canonical.root.clone();
    let generated = regenerate_repair(RepairRegenerateOptions {
        root: root.clone(),
        dry_run: options.dry_run,
    })
    .await?;
    let cleanup = cleanup_repair(RepairCleanupOptions {
        root: root.clone(),
        dry_run: options.dry_run,
        keep_canonical: options.keep_canonical,
        keep_generated: options.keep_generated,
        generated_only: options.generated_only,
    })?;
    Ok(RepairApplyReport {
        schema: "athanor.repair_apply.v2".to_string(),
        root,
        dry_run: options.dry_run,
        canonical,
        generated,
        remaining_issues: cleanup.remaining_issues.clone(),
        cleanup,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn pointer_without_canonical_latest_blocks_cleanup() {
        let root = test_root();
        fs::create_dir_all(root.join(".athanor/state")).unwrap();
        fs::write(root.join(POINTER_PATH), "{}").unwrap();

        let report = inspect_repair(RepairInspectOptions { root: root.clone() }).unwrap();
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "missing_index_current_canonical_snapshot")
        );
        let error = cleanup_repair(RepairCleanupOptions {
            root: root.clone(),
            dry_run: false,
            keep_canonical: 0,
            keep_generated: 0,
            generated_only: false,
        })
        .expect_err("invalid canonical latest must block cleanup");
        assert!(error.to_string().contains("refusing repair cleanup"));
        fs::remove_dir_all(root).unwrap();
    }

    fn test_root() -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("athanor-repair-guard-{nonce}"));
        fs::create_dir_all(&root).unwrap();
        root
    }
}
