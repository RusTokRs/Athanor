use std::fs;
use std::path::Path;

use athanor_app::{
    GraphOmitted, RUSTOK_PAGE_BUILDER_AUDIT_SCHEMA_V1, RustokPageBuilderAudit,
    RustokPageBuilderAuditConsumer, RustokPageBuilderAuditSummary,
    RustokPageBuilderConsumerGraphReport, RustokPageBuilderGraph, RustokPageBuilderGraphEdge,
    RustokPageBuilderGraphNode, RustokPageBuilderProviderGraphReport,
    RustokPageBuilderViolationsGraphReport, VersionedJsonContract,
};
use serde_json::Value;

#[test]
fn representative_page_builder_contracts_match_golden_fixture() {
    let audit = RustokPageBuilderAudit {
        schema: RUSTOK_PAGE_BUILDER_AUDIT_SCHEMA_V1.to_string(),
        snapshot: "snap-page-builder".to_string(),
        summary: RustokPageBuilderAuditSummary {
            providers_total: 1,
            consumers_total: 1,
            contracts_total: 1,
            capabilities_total: 1,
            fallback_profiles_total: 1,
            wave_evidence_total: 1,
            diagnostics_open: 0,
        },
        providers: vec!["catalog_provider".to_string()],
        consumers: vec![RustokPageBuilderAuditConsumer {
            id: "page_builder_consumer://catalog".to_string(),
            module: "catalog".to_string(),
            diagnostics: Vec::new(),
        }],
        contracts: vec!["page_builder.contract.catalog.v1".to_string()],
        capabilities: vec!["catalog.products".to_string()],
        fallback_profiles: vec!["catalog-safe".to_string()],
        wave_evidence: vec!["docs/waves/catalog.md".to_string()],
        diagnostics: Vec::new(),
    };

    let provider = RustokPageBuilderProviderGraphReport::new(graph(
        Some("catalog_provider"),
        vec![
            node(
                "catalog_provider",
                "rustok_page_builder_provider",
                "Catalog provider",
                "src/catalog/provider.rs",
            ),
            node(
                "catalog.products",
                "rustok_page_builder_capability",
                "Catalog products",
                "contracts/catalog.json",
            ),
        ],
        edge(
            "catalog_provider",
            "catalog.products",
            "rustok_page_builder_provides",
            "src/catalog/provider.rs:10",
        ),
    ));
    let consumer = RustokPageBuilderConsumerGraphReport::new(graph(
        Some("catalog"),
        vec![
            node(
                "catalog",
                "rustok_page_builder_consumer",
                "Catalog page",
                "src/catalog/page.rs",
            ),
            node(
                "catalog.products",
                "rustok_page_builder_capability",
                "Catalog products",
                "contracts/catalog.json",
            ),
        ],
        edge(
            "catalog",
            "catalog.products",
            "rustok_page_builder_consumes",
            "src/catalog/page.rs:14",
        ),
    ));
    let violations = RustokPageBuilderViolationsGraphReport::new(graph(
        None,
        vec![node(
            "missing-fallback",
            "rustok_page_builder_violation",
            "Missing fallback profile",
            "src/catalog/page.rs",
        )],
        edge(
            "missing-fallback",
            "catalog",
            "rustok_page_builder_violation_for",
            "src/catalog/page.rs:28",
        ),
    ));

    audit
        .validate_contract()
        .expect("valid Page Builder audit contract");
    provider
        .validate_contract()
        .expect("valid Page Builder provider graph contract");
    consumer
        .validate_contract()
        .expect("valid Page Builder consumer graph contract");
    violations
        .validate_contract()
        .expect("valid Page Builder violations graph contract");

    let fixture = read_fixture("rustok_page_builder_contracts.v1.json");
    assert_eq!(serde_json::to_value(audit).unwrap(), fixture["audit"]);
    assert_eq!(
        serde_json::to_value(provider).unwrap(),
        fixture["provider_graph"]
    );
    assert_eq!(
        serde_json::to_value(consumer).unwrap(),
        fixture["consumer_graph"]
    );
    assert_eq!(
        serde_json::to_value(violations).unwrap(),
        fixture["violations_graph"]
    );
}

fn graph(
    root: Option<&str>,
    nodes: Vec<RustokPageBuilderGraphNode>,
    edge: RustokPageBuilderGraphEdge,
) -> RustokPageBuilderGraph {
    RustokPageBuilderGraph {
        schema: "overwritten-by-wrapper".to_string(),
        snapshot: "snap-page-builder".to_string(),
        root: root.map(str::to_string),
        nodes,
        edges: vec![edge],
        diagnostics: Vec::new(),
        omitted: GraphOmitted {
            nodes: 0,
            edges: 0,
            reason: "limits".to_string(),
        },
    }
}

fn node(id: &str, kind: &str, name: &str, source: &str) -> RustokPageBuilderGraphNode {
    RustokPageBuilderGraphNode {
        id: id.to_string(),
        kind: kind.to_string(),
        name: name.to_string(),
        source: Some(source.to_string()),
    }
}

fn edge(from: &str, to: &str, kind: &str, evidence: &str) -> RustokPageBuilderGraphEdge {
    RustokPageBuilderGraphEdge {
        from: from.to_string(),
        to: to.to_string(),
        kind: kind.to_string(),
        evidence: vec![evidence.to_string()],
    }
}

fn read_fixture(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name);
    serde_json::from_str(
        &fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display())),
    )
    .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}
