use athanor_app::{GraphCycles, GraphHubs, GraphPageRank, GraphPath, GraphRelated};

pub(crate) fn print_related(report: &GraphRelated) {
    println!(
        "Related graph for {} (snapshot: {})",
        report.root.entity.stable_key, report.snapshot
    );
    println!(
        "{} entities, {} relations{}",
        report.nodes.len(),
        report.edges.len(),
        if report.truncated { " (truncated)" } else { "" }
    );
    for node in &report.nodes {
        let source = node.entity.source.as_deref().unwrap_or("unknown source");
        println!(
            "  - distance={} [{}] {} ({}) source={}",
            node.distance, node.entity.kind, node.entity.name, node.entity.stable_key, source
        );
    }
    if !report.edges.is_empty() {
        println!("Relations:");
        for edge in &report.edges {
            println!(
                "  - {} [{}] {} -> {}",
                edge.id, edge.kind, edge.from, edge.to
            );
        }
    }
}

pub(crate) fn print_path(report: &GraphPath) {
    println!(
        "Graph path from {} to {} (snapshot: {})",
        report.from.stable_key, report.to.stable_key, report.snapshot
    );
    if !report.found {
        println!(
            "No path found after visiting {} entities{}",
            report.visited,
            if report.truncated { " (search truncated)" } else { "" }
        );
        return;
    }

    println!(
        "{} hops after visiting {} entities{}",
        report.hops.unwrap_or_default(),
        report.visited,
        if report.truncated { " (search truncated)" } else { "" }
    );
    for (index, node) in report.nodes.iter().enumerate() {
        let source = node.source.as_deref().unwrap_or("unknown source");
        println!(
            "  {index}. [{}] {} ({}) source={}",
            node.kind, node.name, node.stable_key, source
        );
        if let Some(edge) = report.edges.get(index) {
            println!(
                "     via {} [{}] {} -> {}",
                edge.id, edge.kind, edge.from, edge.to
            );
        }
    }
}

pub(crate) fn print_hubs(report: &GraphHubs) {
    let kind = report.kind.as_deref().unwrap_or("all kinds");
    println!(
        "Graph hubs for {kind} (snapshot: {}, omitted: {})",
        report.snapshot, report.omitted
    );
    if report.hubs.is_empty() {
        println!("  (none)");
        return;
    }
    for (index, hub) in report.hubs.iter().enumerate() {
        let source = hub.entity.source.as_deref().unwrap_or("unknown source");
        println!(
            "  {}. [{}] {} ({}) degree={} incoming={} outgoing={} source={}",
            index + 1,
            hub.entity.kind,
            hub.entity.name,
            hub.entity.stable_key,
            hub.entity.degree,
            hub.incoming_degree,
            hub.outgoing_degree,
            source
        );
    }
}

pub(crate) fn print_pagerank(report: &GraphPageRank) {
    let kind = report.kind.as_deref().unwrap_or("all kinds");
    println!(
        "Graph PageRank for {kind} (snapshot: {}, entities: {}, relations: {}, iterations: {}, converged: {}, omitted: {})",
        report.snapshot,
        report.entity_count,
        report.relation_count,
        report.iterations,
        report.converged,
        report.omitted
    );
    if report.ranks.is_empty() {
        println!("  (none)");
        return;
    }
    for entry in &report.ranks {
        let source = entry.entity.source.as_deref().unwrap_or("unknown source");
        println!(
            "  {}. [{:.8}] [{}] {} ({}) source={}",
            entry.rank,
            entry.score,
            entry.entity.kind,
            entry.entity.name,
            entry.entity.stable_key,
            source
        );
    }
}

pub(crate) fn print_cycles(report: &GraphCycles) {
    println!(
        "Directed graph cycles (snapshot: {}, starts: {}, omitted starts: {})",
        report.snapshot, report.start_entities, report.omitted_start_entities
    );
    if report.cycles.is_empty() {
        println!(
            "  (none{})",
            if report.truncated { "; search truncated" } else { "" }
        );
        return;
    }
    for (index, cycle) in report.cycles.iter().enumerate() {
        let stable_keys = cycle
            .nodes
            .iter()
            .map(|node| node.stable_key.as_str())
            .collect::<Vec<_>>()
            .join(" -> ");
        println!(
            "  {}. length={} {} -> {}",
            index + 1,
            cycle.length,
            stable_keys,
            cycle.nodes[0].stable_key
        );
        println!(
            "     relations: {}",
            cycle
                .edges
                .iter()
                .map(|edge| edge.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    if report.truncated {
        println!("  search truncated by configured limits");
    }
}
