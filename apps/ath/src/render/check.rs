use anyhow::Result;
use athanor_app::{AffectedCheckReport, DiagnosticCheckReport};

use super::support::serialized_name;

pub(crate) fn print_diagnostics(report: &DiagnosticCheckReport) -> Result<()> {
    println!(
        "{} diagnostics in {}: {} open ({} critical, {} high, {} medium, {} low)",
        serialized_name(&report.scope)?,
        report.snapshot,
        report.counts.total,
        report.counts.critical,
        report.counts.high,
        report.counts.medium,
        report.counts.low
    );
    for diagnostic in &report.diagnostics {
        let location = diagnostic
            .evidence
            .iter()
            .find_map(|evidence| {
                evidence.source_file.as_ref().map(|path| {
                    evidence
                        .line_start
                        .map_or_else(|| path.clone(), |line| format!("{path}:{line}"))
                })
            })
            .or_else(|| {
                diagnostic
                    .ownership
                    .first()
                    .map(|ownership| ownership.source_file.clone())
            })
            .unwrap_or_else(|| "unknown source".to_string());
        println!(
            "{} {} at {} — {}",
            serialized_name(&diagnostic.severity)?,
            serialized_name(&diagnostic.kind)?,
            location,
            diagnostic.title
        );
    }
    Ok(())
}

pub(crate) fn print_affected(report: &AffectedCheckReport) -> Result<()> {
    println!(
        "affected diagnostics in {}: {} open ({} critical, {} high, {} medium, {} low)",
        report.snapshot,
        report.counts.total,
        report.counts.critical,
        report.counts.high,
        report.counts.medium,
        report.counts.low
    );
    println!(
        "affected files: {} changed, {} unchanged, {} removed",
        report.affected_files.changed,
        report.affected_files.unchanged,
        report.affected_files.removed
    );
    if !report.stale_artifacts.is_empty() {
        println!("stale artifacts: {}", report.stale_artifacts.len());
        for artifact in &report.stale_artifacts {
            println!(
                "{} at {}: {} (run `{}`)",
                serialized_name(&artifact.kind)?,
                artifact.path.display(),
                artifact.message,
                artifact.suggested_command
            );
        }
    }
    if !report.documentation_drift.is_empty() {
        println!(
            "affected documentation drift: {}",
            report.documentation_drift.len()
        );
        for document in &report.documentation_drift {
            println!(
                "{}: {} (verified: {})",
                document.path,
                document.reason,
                document.verified_snapshot.as_deref().unwrap_or("missing")
            );
        }
    }
    for diagnostic in &report.diagnostics {
        let location = diagnostic
            .evidence
            .iter()
            .find_map(|evidence| {
                evidence.source_file.as_ref().map(|path| {
                    evidence
                        .line_start
                        .map_or_else(|| path.clone(), |line| format!("{path}:{line}"))
                })
            })
            .or_else(|| {
                diagnostic
                    .ownership
                    .first()
                    .map(|ownership| ownership.source_file.clone())
            })
            .unwrap_or_else(|| "unknown source".to_string());
        println!(
            "{} {} at {} — {}",
            serialized_name(&diagnostic.severity)?,
            serialized_name(&diagnostic.kind)?,
            location,
            diagnostic.title
        );
    }
    Ok(())
}
