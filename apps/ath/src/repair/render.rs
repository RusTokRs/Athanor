use athanor_app::{
    IndexGenerationCleanupReport, RepairCanonicalLatestReport, RepairInspectReport,
    RepairRecoverIndexCleanupReport, RepairRecoverIndexReport,
};

use super::model::HelpTopic;

pub(super) fn print_help(topic: HelpTopic) {
    match topic {
        HelpTopic::Repair => {
            println!("Repair commands:");
            println!("  inspect               Inspect local repair state");
            println!("  index-retention       Plan or apply immutable index-generation retention");
            println!("  recover-index         Recover a pending transactional index publication");
            println!("  recover-index-cleanup Finish an interrupted confirmed index cleanup");
            println!(
                "  repair-latest         Repair canonical latest to the authoritative generation"
            );
        }
        HelpTopic::Inspect => {
            println!("Inspect local canonical and generated repair state");
            println!();
            println!("Usage: ath repair inspect [PATH] [--json]");
        }
        HelpTopic::IndexRetention => {
            println!("Plan or apply immutable index-generation retention");
            println!();
            println!("Usage:");
            println!("  ath repair index-retention [PATH] --dry-run [--keep <N>] [--json]");
            println!(
                "  ath repair index-retention [PATH] --confirmation-token <TOKEN> [--keep <N>] [--json]"
            );
            println!();
            println!(
                "A destructive invocation requires the token emitted by an exact dry-run plan."
            );
        }
        HelpTopic::RecoverIndex => {
            println!(
                "Recover a pending transactional index publication without running the indexing pipeline"
            );
            println!();
            println!("Usage: ath repair recover-index [PATH] [--dry-run] [--json]");
        }
        HelpTopic::RecoverIndexCleanup => {
            println!("Finish an interrupted confirmed index-generation cleanup");
            println!();
            println!("Usage: ath repair recover-index-cleanup [PATH] [--dry-run] [--json]");
        }
        HelpTopic::RepairLatest => {
            println!("Repair canonical latest to the authoritative newest committed generation");
            println!();
            println!(
                "Usage: ath repair repair-latest [PATH] [--snapshot <ID>] [--dry-run] [--json]"
            );
            println!();
            println!(
                "An explicit snapshot is accepted only when backend discovery confirms it is authoritative."
            );
        }
    }
}

pub(super) fn print_inspect(report: &RepairInspectReport) {
    println!("repair inspection at {}", report.root.display());
    println!("  issues: {}", report.issues.len());
    for issue in &report.issues {
        println!(
            "  {}: {} ({})",
            issue.code,
            issue.message,
            issue.path.display()
        );
    }
}

pub(super) fn print_index_retention(report: &IndexGenerationCleanupReport) {
    let action = if report.dry_run { "planned" } else { "removed" };
    println!("index-generation cleanup at {}", report.root.display());
    println!("  {action}: {}", report.removed.len());
    println!("  retained: {}", report.retained.len());
    if let Some(token) = &report.confirmation_token {
        println!("  confirmation token: {token}");
    }
    for row in &report.removed {
        println!("  {action} {}", row.generation);
    }
    println!("  remaining issues: {}", report.remaining_issues.len());
}

pub(super) fn print_recover_index(report: &RepairRecoverIndexReport) {
    println!("index publication recovery at {}", report.root.display());
    println!("  needed: {}", report.needed);
    println!("  recovered: {}", report.recovered);
    if let Some(snapshot) = &report.snapshot {
        println!("  snapshot: {snapshot}");
    }
    if let Some(generation) = &report.generation {
        println!("  generation: {generation}");
    }
    println!("  remaining issues: {}", report.remaining_issues.len());
}

pub(super) fn print_recover_index_cleanup(report: &RepairRecoverIndexCleanupReport) {
    println!("index cleanup recovery at {}", report.root.display());
    println!("  needed: {}", report.needed);
    println!("  recovered: {}", report.recovered);
    for tombstone in &report.tombstones {
        println!("  staged: {} ({})", tombstone.generation, tombstone.token);
    }
    println!("  remaining issues: {}", report.remaining_issues.len());
}

pub(super) fn print_repair_latest(report: &RepairCanonicalLatestReport) {
    println!("canonical latest repair at {}", report.root.display());
    println!("  needed: {}", report.needed);
    println!("  repaired: {}", report.repaired);
    println!(
        "  target: {} ({})",
        report.target.snapshot.0, report.target.generation
    );
    if let Some(previous) = &report.previous {
        println!(
            "  previous: {} ({})",
            previous.snapshot.0, previous.generation
        );
    }
    if let Some(error) = &report.previous_error {
        println!("  previous pointer error: {error}");
    }
    println!("  remaining issues: {}", report.remaining_issues.len());
}
