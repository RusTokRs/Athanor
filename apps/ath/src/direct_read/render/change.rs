use anyhow::Result;
use athanor_app::{ChangeMapReport, ImpactAnalysis};

use super::support::serialized_name;

pub(crate) fn print_impact_analysis(analysis: &ImpactAnalysis) -> Result<()> {
    println!("Code Impact Analysis (snapshot: {})", analysis.snapshot);
    println!("Starting Entities:");
    if analysis.starting_entities.is_empty() {
        println!("  (none)");
    } else {
        for entity in &analysis.starting_entities {
            println!(
                "  - [{}] {} ({})",
                serialized_name(&entity.kind)?,
                entity.name,
                entity.stable_key.0
            );
        }
    }
    println!();

    println!("Impacted Files ({}):", analysis.impacted_files.len());
    for file in &analysis.impacted_files {
        println!("  - {file}");
    }
    println!();

    println!("Impacted Entities ({}):", analysis.impacted_entities.len());
    if analysis.impacted_entities.is_empty() {
        println!("  (none)");
    } else {
        let max_depth = analysis
            .impacted_entities
            .iter()
            .map(|item| item.depth)
            .max()
            .unwrap_or_default();
        for depth in 1..=max_depth {
            let items_at_depth = analysis
                .impacted_entities
                .iter()
                .filter(|item| item.depth == depth)
                .collect::<Vec<_>>();
            if items_at_depth.is_empty() {
                continue;
            }
            println!("  Depth {depth}:");
            for item in items_at_depth {
                let relation_info = if item.path_steps.is_empty() {
                    "direct".to_string()
                } else {
                    item.path_steps
                        .iter()
                        .map(|step| {
                            format!(
                                "{} --{}:{}--> {}",
                                step.from.name, step.relation_kind, step.relation_id, step.to.name
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(" | ")
                };
                println!(
                    "    - [{}] {} ({})",
                    serialized_name(&item.entity.kind)?,
                    item.entity.name,
                    item.entity.stable_key.0
                );
                println!("      path: {relation_info}");
            }
        }
    }
    println!();

    println!(
        "Impacted Diagnostics ({}):",
        analysis.impacted_diagnostics.len()
    );
    if analysis.impacted_diagnostics.is_empty() {
        println!("  (none)");
    } else {
        for diagnostic in &analysis.impacted_diagnostics {
            println!(
                "  - [{}] {} — {}",
                serialized_name(&diagnostic.severity)?,
                serialized_name(&diagnostic.kind)?,
                diagnostic.title
            );
        }
    }
    Ok(())
}

pub(crate) fn print_change_map(report: &ChangeMapReport) -> Result<()> {
    println!("Change Map (snapshot: {})", report.snapshot);
    println!(
        "Returned: {} entities, {} files, {} diagnostics; omitted: {}/{}/{}",
        report.returned.entities,
        report.returned.files,
        report.returned.diagnostics,
        report.omitted.entities,
        report.omitted.files,
        report.omitted.diagnostics
    );
    println!();
    println!("Files:");
    for file in &report.files {
        println!("  {}. {} [score={}]", file.rank, file.path, file.score);
        println!("     kinds: {}", file.entity_kinds.join(", "));
    }
    if report.files.is_empty() {
        println!("  (none)");
    }
    println!();
    println!("Entities:");
    for item in &report.items {
        println!(
            "  {}. [{}] {} ({}) score={} depth={}",
            item.rank,
            serialized_name(&item.entity.kind)?,
            item.entity.name,
            item.entity.stable_key.0,
            item.score,
            item.depth
        );
        println!("     why: {}", item.reasons.join("; "));
        for step in &item.path {
            println!(
                "     path: {} --{}:{}--> {} (confidence {:.2})",
                step.from.stable_key,
                step.relation_kind,
                step.relation_id,
                step.to.stable_key,
                step.confidence
            );
        }
        println!(
            "     tests: {}{}",
            serialized_name(&item.test_coverage.status)?,
            if item.test_coverage.tests.is_empty() {
                String::new()
            } else {
                format!(" ({})", item.test_coverage.tests.join(", "))
            }
        );
        for annotation in &item.annotations {
            println!(
                "     annotation: {} [{}] {}",
                annotation.source, annotation.schema, annotation.message
            );
        }
    }
    if report.items.is_empty() {
        println!("  (none)");
    }
    println!();
    println!("Completeness: {}", report.completeness.note);
    if report.completeness.candidate_limit_reached {
        println!(
            "Traversal stopped at the internal candidate limit of {}.",
            report.completeness.candidate_limit
        );
    }
    println!("Inspect coverage: {}", report.completeness.suggested_command);
    Ok(())
}
