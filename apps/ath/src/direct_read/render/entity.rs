use anyhow::Result;
use athanor_app::{EntityExplanation, RepositoryOverview};

use super::support::{print_named_counts, serialized_name};

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
