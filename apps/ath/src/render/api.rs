use anyhow::Result;
use athanor_app::{ApiCleanupReport, ApiContractDiff};

use super::support::serialized_name;

pub(crate) fn print_contract_diff(diff: &ApiContractDiff) -> Result<()> {
    println!(
        "API contract {} -> {}: {} changes, {} breaking",
        diff.from,
        diff.to,
        diff.changes.len(),
        diff.breaking_changes
    );
    for change in &diff.changes {
        println!(
            "{} {}{}",
            serialized_name(&change.kind)?,
            change.stable_key,
            if change.breaking { " [breaking]" } else { "" }
        );
        for reason in &change.reasons {
            println!("  reason: {reason}");
        }
    }
    if let Some(cleanup) = &diff.cleanup {
        print_cleanup_summary(cleanup);
    }
    Ok(())
}

fn print_cleanup_summary(report: &ApiCleanupReport) {
    println!(
        "API cleanup: {} removed, {} retained{}",
        report.removed.len(),
        report.retained.len(),
        if report.dry_run { " (dry run)" } else { "" }
    );
}
