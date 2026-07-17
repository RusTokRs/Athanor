use anyhow::Result;
use athanor_app::{
    ChangeMapReport, EntityExplanation, ImpactAnalysis, NamedCount, RepositoryOverview,
};

pub(super) fn print_explanation(explanation: &EntityExplanation) -> Result<()> {
    println!("{}", explanation.entity.stable_key.0);
    println!("snapshot: {}", explanation.snapshot);
    println!("kind: {}", serialized_name(&explanation.entity.kind)?);
    println!("name: {}", explanation.entity.name);
    if let Some(source) = &explanation.entity.source {
        let line = source
            .line_start
            .map_or_else(String::new, |line| format!(":{line}"));
        println!("source: {}{line}", source.path);
    }
    for fact in &explanation.facts {
        println!(
            "fact: {} (confidence {:.2})",
            serialized_name(&fact.kind)?,
            fact.confidence
        );
    }
    for explained in &explanation.outgoing_relations {
        let target = explained
            .related_entity
            .as_ref()
            .map_or(explained.relation.to.0.as_str(), |entity| {
                entity.stable_key.0.as_str()
            });
        println!(
            "relation: --{}--> {} [{}]",
            serialized_name(&explained.relation.kind)?,
            target,
            serialized_name(&explained.relation.status)?
        );
    }
    for explained in &explanation.incoming_relations {
        let source = explained
            .related_entity
            .as_ref()
            .map_or(explained.relation.from.0.as_str(), |entity| {
                entity.stable_key.0.as_str()
            });
        println!(
            "relation: {} --{}--> [this] [{}]",
            source,
            serialized_name(&explained.relation.kind)?,
            serialized_name(&explained.relation.status)?
        );
    }
    for diagnostic in &explanation.diagnostics {
        println!(
            "diagnostic: {} {} — {}",
            serialized_name(&diagnostic.severity)?,
            serialized_name(&diagnostic.kind)?,
            diagnostic.title
        );
    }
    Ok(())
}

pub(super) fn print_overview(overview: &RepositoryOverview) -> Result<()> {
    println!("Athanor overview (snapshot: {})", overview.snapshot);
    println!(
        "canonical objects: {} entities, {} facts, {} relations, {} diagnostics ({} open)",
        overview.totals.entities,
        overview.totals.facts,
        overview.totals.relations,
        overview.totals.diagnostics,
        overview.totals.open_diagnostics
    );
    println!("source files: {}", overview.totals.source_files);
    println!();
    println!(
        "API: {} endpoints, {} schemas, {} examples, {} documented, {} implemented",
        overview.api.endpoints,
        overview.api.schemas,
        overview.api.examples,
        overview.api.documented_endpoints,
        overview.api.implemented_endpoints
    );
    println!(
        "Docs: {} pages, {} sections, {} runbooks, {} operation steps, {} operations pages",
        overview.docs.pages,
        overview.docs.sections,
        overview.docs.runbooks,
        overview.docs.operation_steps,
        overview.docs.operations_pages
    );
    println!(
        "Operations: {} env vars, {} script commands, {} deployment resources, {} migrations, {} packages, {} dependencies",
        overview.operations.environment_variables,
        overview.operations.script_commands,
        overview.operations.deployment_resources,
        overview.operations.database_migrations,
        overview.operations.packages,
        overview.operations.dependencies
    );
    println!();

    print_named_counts("Top entity kinds", &overview.entity_kinds);
    print_named_counts("Top relation kinds", &overview.relation_kinds);
    print_named_counts("Top source roots", &overview.source_roots);

    println!("Module structure:");
    if overview.module_structure.is_empty() {
        println!("  (none)");
    } else {
        for module in &overview.module_structure {
            let source = module.source.as_deref().unwrap_or("unknown source");
            println!(
                "  - {} ({}) members={} source={}",
                module.name, module.stable_key, module.direct_members, source
            );
        }
    }

    println!("Integration boundaries:");
    if overview.integration_boundaries.is_empty() {
        println!("  (none)");
    } else {
        for boundary in &overview.integration_boundaries {
            println!(
                "  - {} -> {}: {} relations",
                boundary.from_root, boundary.to_root, boundary.relations
            );
        }
    }

    println!("Graph hubs:");
    if overview.graph_hubs.is_empty() {
        println!("  (none)");
    } else {
        for hub in &overview.graph_hubs {
            let source = hub.source.as_deref().unwrap_or("unknown source");
            println!(
                "  - [{}] {} ({}) degree={} source={}",
                hub.kind, hub.name, hub.stable_key, hub.degree, source
            );
        }
    }

    println!("Open diagnostics:");
    if overview.open_diagnostics.is_empty() {
        println!("  (none)");
    } else {
        for diagnostic in &overview.open_diagnostics {
            let source = diagnostic.source.as_deref().unwrap_or("unknown source");
            println!(
                "  - [{}] {} at {} - {}",
                diagnostic.severity, diagnostic.kind, source, diagnostic.title
            );
        }
    }
    Ok(())
}

pub(super) fn print_impact_analysis(analysis: &ImpactAnalysis) -> Result<()> {
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

pub(super) fn print_change_map(report: &ChangeMapReport) -> Result<()> {
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

fn print_named_counts(title: &str, counts: &[NamedCount]) {
    println!("{title}:");
    if counts.is_empty() {
        println!("  (none)");
    } else {
        for item in counts {
            println!("  - {}: {}", item.name, item.count);
        }
    }
}

fn serialized_name(value: &impl serde::Serialize) -> Result<String> {
    Ok(serde_json::to_value(value)?
        .as_str()
        .map_or_else(|| "unknown".to_string(), str::to_string))
}
