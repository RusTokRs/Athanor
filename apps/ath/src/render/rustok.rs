use athanor_app::{
    RustokArchitectureContext, RustokFbaAudit, RustokFbaGraph, RustokFfaAudit, RustokFfaGraph,
    RustokPageBuilderAudit, RustokPageBuilderGraph,
};

pub(crate) fn print_architecture_context(report: &RustokArchitectureContext) {
    println!(
        "RusTok architecture context (snapshot: {}, resolution: {}, primary module: {})",
        report.snapshot,
        report.resolution.status,
        report
            .resolution
            .primary_module
            .as_deref()
            .unwrap_or("unresolved")
    );
    println!("  {}", report.resolution.summary);
    if !report.modules.is_empty() {
        println!("  modules:");
        for module in &report.modules {
            println!(
                "    - {} score={} reasons={}",
                module.slug,
                module.score,
                module.reasons.join(", ")
            );
        }
    }
    if !report.contracts.is_empty() {
        println!("  public contracts:");
        for contract in &report.contracts {
            println!("    - {}", contract.stable_key);
        }
    }
    if !report.interactions.is_empty() {
        println!("  interactions:");
        for interaction in &report.interactions {
            println!(
                "    - {} -> {} profile={}",
                interaction.consumer, interaction.provider, interaction.profile
            );
        }
    }
    println!("  guidance:");
    for guidance in &report.guidance {
        println!("    - {guidance}");
    }
}

pub(crate) fn print_ffa_audit(report: &RustokFfaAudit) {
    let completion = report
        .summary
        .completion_percent
        .map_or_else(|| "n/a".to_string(), |percent| format!("{percent}%"));
    println!(
        "RusTok FFA audit (snapshot: {}, observed: {}, actionable: {}, complete: {}, incomplete: {}, structural completion: {}, missing core/transport/ui: {}/{}/{}, scaffolds: {}, host wiring: {}, diagnostics: {})",
        report.snapshot,
        report.summary.observed_surfaces,
        report.summary.surfaces_total,
        report.summary.core_transport_ui,
        report.summary.incomplete,
        completion,
        report.summary.missing_core,
        report.summary.missing_transport,
        report.summary.missing_ui_adapter,
        report.summary.scaffold_surfaces,
        report.summary.host_wiring_surfaces,
        report.summary.diagnostics_open
    );
    if report.surfaces.is_empty() {
        println!("  (none)");
        return;
    }
    for surface in &report.surfaces {
        let diagnostics = if surface.diagnostics.is_empty() {
            "none".to_string()
        } else {
            surface.diagnostics.join(", ")
        };
        let completion = surface
            .completion_percent
            .map_or_else(|| "n/a".to_string(), |percent| format!("{percent}%"));
        println!(
            "  - {} shape={} structural_completion={} requirements={}/{} layers={} files={} diagnostics={}",
            surface.id,
            surface.shape,
            completion,
            surface.requirements_met,
            surface.requirements_total,
            surface.layers.join(","),
            surface.files.len(),
            diagnostics
        );
    }
}

pub(crate) fn print_ffa_graph(report: &RustokFfaGraph) {
    let surface = report.surface.as_deref().unwrap_or("violations");
    println!(
        "RusTok FFA graph for {} (snapshot: {}, nodes: {}, edges: {}, diagnostics: {}, omitted nodes: {}, omitted edges: {})",
        surface,
        report.snapshot,
        report.nodes.len(),
        report.edges.len(),
        report.diagnostics.len(),
        report.omitted.nodes,
        report.omitted.edges
    );
    for diagnostic in &report.diagnostics {
        println!("  ! {:?} {}", diagnostic.severity, diagnostic.title);
    }
    for node in &report.nodes {
        let source = node.source.as_deref().unwrap_or("unknown source");
        println!("  - [{}] {} source={}", node.kind, node.id, source);
    }
    for edge in &report.edges {
        println!("    {} -> {} [{}]", edge.from, edge.to, edge.kind);
    }
}

pub(crate) fn print_fba_audit(report: &RustokFbaAudit) {
    let completion = report
        .summary
        .completion_percent
        .map_or_else(|| "n/a".to_string(), |percent| format!("{percent}%"));
    println!(
        "RusTok FBA audit (snapshot: {}, modules: {}, registered: {}, dependency-only: {}, in progress: {}, status unknown: {}, contract completion: {} ({}/{}), providers: {}, consumers: {}, ports: {}, operations: {}, dependency edges: {}/{}, diagnostics: {})",
        report.snapshot,
        report.summary.modules_total,
        report.summary.registered_modules,
        report.summary.dependency_only_modules,
        report.summary.in_progress_modules,
        report.summary.status_unknown_modules,
        completion,
        report.summary.requirements_met,
        report.summary.requirements_total,
        report.summary.provider_modules,
        report.summary.consumer_modules,
        report.summary.ports_total,
        report.summary.operations_total,
        report.summary.dependency_edges_resolved,
        report.summary.dependency_edges_total,
        report.summary.diagnostics_open
    );
    if report.modules.is_empty() {
        println!("  (none)");
        return;
    }
    for module in &report.modules {
        let diagnostics = if module.diagnostics.is_empty() {
            "none".to_string()
        } else {
            module.diagnostics.join(", ")
        };
        let completion = module
            .completion_percent
            .map_or_else(|| "n/a".to_string(), |percent| format!("{percent}%"));
        println!(
            "  - {} role={} status={} contract={} contract_completion={} requirements={}/{} ports={} operations={} dependencies={} diagnostics={}",
            module.id,
            module.role.as_deref().unwrap_or("unknown"),
            module.status.as_deref().unwrap_or("unknown"),
            module.contract_version.as_deref().unwrap_or("none"),
            completion,
            module.requirements_met,
            module.requirements_total,
            module.ports.len(),
            module.operations.len(),
            module.dependencies.len(),
            diagnostics
        );
    }
}

pub(crate) fn print_fba_graph(report: &RustokFbaGraph) {
    let root = report.root.as_deref().unwrap_or("violations");
    println!(
        "RusTok FBA graph for {} (snapshot: {}, nodes: {}, edges: {}, diagnostics: {}, omitted nodes: {}, omitted edges: {})",
        root,
        report.snapshot,
        report.nodes.len(),
        report.edges.len(),
        report.diagnostics.len(),
        report.omitted.nodes,
        report.omitted.edges
    );
    for diagnostic in &report.diagnostics {
        println!("  ! {:?} {}", diagnostic.severity, diagnostic.title);
    }
    for node in &report.nodes {
        let source = node.source.as_deref().unwrap_or("unknown source");
        println!("  - [{}] {} source={}", node.kind, node.id, source);
    }
    for edge in &report.edges {
        println!("    {} -> {} [{}]", edge.from, edge.to, edge.kind);
    }
}

pub(crate) fn print_page_builder_audit(report: &RustokPageBuilderAudit) {
    println!(
        "RusTok Page Builder audit (snapshot: {}, providers: {}, consumers: {}, contracts: {}, capabilities: {}, fallback profiles: {}, wave evidence: {}, diagnostics: {})",
        report.snapshot,
        report.summary.providers_total,
        report.summary.consumers_total,
        report.summary.contracts_total,
        report.summary.capabilities_total,
        report.summary.fallback_profiles_total,
        report.summary.wave_evidence_total,
        report.summary.diagnostics_open
    );
    if report.consumers.is_empty() {
        println!("  consumers: (none)");
    } else {
        println!("  consumers:");
        for consumer in &report.consumers {
            let diagnostics = if consumer.diagnostics.is_empty() {
                "none".to_string()
            } else {
                consumer.diagnostics.join(", ")
            };
            println!(
                "  - {} module={} diagnostics={}",
                consumer.id, consumer.module, diagnostics
            );
        }
    }
    if !report.diagnostics.is_empty() {
        println!("  diagnostics: {}", report.diagnostics.join(", "));
    }
}

pub(crate) fn print_page_builder_graph(report: &RustokPageBuilderGraph) {
    let root = report.root.as_deref().unwrap_or("violations");
    println!(
        "RusTok Page Builder graph for {} (snapshot: {}, nodes: {}, edges: {}, diagnostics: {}, omitted nodes: {}, omitted edges: {})",
        root,
        report.snapshot,
        report.nodes.len(),
        report.edges.len(),
        report.diagnostics.len(),
        report.omitted.nodes,
        report.omitted.edges
    );
    for diagnostic in &report.diagnostics {
        println!("  ! {:?} {}", diagnostic.severity, diagnostic.title);
    }
    for node in &report.nodes {
        let source = node.source.as_deref().unwrap_or("unknown source");
        println!("  - [{}] {} source={}", node.kind, node.id, source);
    }
    for edge in &report.edges {
        println!("    {} -> {} [{}]", edge.from, edge.to, edge.kind);
    }
}
