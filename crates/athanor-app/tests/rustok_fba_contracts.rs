use std::fs;
use std::path::Path;

use athanor_app::{
    GraphOmitted, RUSTOK_FBA_AUDIT_SCHEMA_V1, RustokFbaAudit, RustokFbaAuditModule,
    RustokFbaAuditSummary, RustokFbaDependenciesGraphReport, RustokFbaGraph,
    RustokFbaGraphEdge, RustokFbaGraphNode, RustokFbaModuleGraphReport,
    RustokFbaPortGraphReport, RustokFbaViolationsGraphReport, VersionedJsonContract,
};
use serde_json::Value;

#[test]
fn representative_fba_contracts_match_golden_fixture() {
    let audit = RustokFbaAudit {
        schema: RUSTOK_FBA_AUDIT_SCHEMA_V1.to_string(),
        snapshot: "snap-fba".to_string(),
        summary: RustokFbaAuditSummary {
            modules_total: 1,
            registered_modules: 1,
            dependency_only_modules: 0,
            in_progress_modules: 0,
            status_unknown_modules: 0,
            requirements_met: 10,
            requirements_total: 10,
            completion_percent: Some(100),
            modules_with_port_code: 1,
            modules_with_complete_operations: 1,
            modules_with_evidence: 1,
            dependency_edges_resolved: 1,
            dependency_edges_total: 1,
            provider_modules: 1,
            consumer_modules: 1,
            ports_total: 1,
            operations_total: 1,
            diagnostics_open: 0,
        },
        modules: vec![RustokFbaAuditModule {
            id: "fba_module://catalog".to_string(),
            module: "catalog".to_string(),
            role: Some("provider_consumer".to_string()),
            status: Some("active".to_string()),
            registry_present: true,
            requirements_met: 10,
            requirements_total: 10,
            completion_percent: Some(100),
            port_code_present: Some(true),
            port_traits_present: Some(true),
            operations_implemented: Some(true),
            context_present: Some(true),
            error_present: Some(true),
            policy_present: Some(true),
            evidence_present: Some(true),
            contract_tests_present: Some(true),
            write_idempotency_present: Some(true),
            dependencies_resolved: Some(true),
            contract_version: Some("1.0.0".to_string()),
            ports: vec!["catalog.read".to_string()],
            operations: vec!["get_product".to_string()],
            dependencies: vec!["inventory".to_string()],
            diagnostics: Vec::new(),
        }],
    };

    let module = RustokFbaModuleGraphReport::new(graph(
        Some("catalog"),
        vec![
            node(
                "catalog",
                "rustok_fba_module",
                "Catalog",
                "contracts/catalog.json",
            ),
            node(
                "catalog.read",
                "rustok_fba_port",
                "Catalog read port",
                "src/catalog/port.rs",
            ),
        ],
        edge(
            "catalog",
            "catalog.read",
            "rustok_fba_declares_port",
            "contracts/catalog.json:4",
        ),
    ));
    let port = RustokFbaPortGraphReport::new(graph(
        Some("catalog.read"),
        vec![
            node(
                "catalog.read",
                "rustok_fba_port",
                "Catalog read port",
                "src/catalog/port.rs",
            ),
            node(
                "get_product",
                "rustok_fba_operation",
                "Get product",
                "src/catalog/service.rs",
            ),
        ],
        edge(
            "catalog.read",
            "get_product",
            "rustok_fba_declares_operation",
            "src/catalog/port.rs:8",
        ),
    ));
    let dependencies = RustokFbaDependenciesGraphReport::new(graph(
        Some("catalog"),
        vec![
            node(
                "catalog",
                "rustok_fba_module",
                "Catalog",
                "contracts/catalog.json",
            ),
            node(
                "inventory",
                "rustok_fba_module",
                "Inventory",
                "contracts/inventory.json",
            ),
        ],
        edge(
            "catalog",
            "inventory",
            "rustok_fba_depends_on",
            "contracts/catalog.json:12",
        ),
    ));
    let violations = RustokFbaViolationsGraphReport::new(graph(
        None,
        vec![node(
            "missing-policy",
            "rustok_fba_violation",
            "Missing policy",
            "src/catalog/service.rs",
        )],
        edge(
            "missing-policy",
            "catalog",
            "rustok_fba_violation_for",
            "src/catalog/service.rs:20",
        ),
    ));

    audit.validate_contract().expect("valid FBA audit contract");
    module
        .validate_contract()
        .expect("valid FBA module graph contract");
    port.validate_contract()
        .expect("valid FBA port graph contract");
    dependencies
        .validate_contract()
        .expect("valid FBA dependencies graph contract");
    violations
        .validate_contract()
        .expect("valid FBA violations graph contract");

    let fixture = read_fixture("rustok_fba_contracts.v1.json");
    assert_eq!(serde_json::to_value(audit).unwrap(), fixture["audit"]);
    assert_eq!(
        serde_json::to_value(module).unwrap(),
        fixture["module_graph"]
    );
    assert_eq!(serde_json::to_value(port).unwrap(), fixture["port_graph"]);
    assert_eq!(
        serde_json::to_value(dependencies).unwrap(),
        fixture["dependencies_graph"]
    );
    assert_eq!(
        serde_json::to_value(violations).unwrap(),
        fixture["violations_graph"]
    );
}

fn graph(
    root: Option<&str>,
    nodes: Vec<RustokFbaGraphNode>,
    edge: RustokFbaGraphEdge,
) -> RustokFbaGraph {
    RustokFbaGraph {
        schema: "overwritten-by-wrapper".to_string(),
        snapshot: "snap-fba".to_string(),
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

fn node(id: &str, kind: &str, name: &str, source: &str) -> RustokFbaGraphNode {
    RustokFbaGraphNode {
        id: id.to_string(),
        kind: kind.to_string(),
        name: name.to_string(),
        source: Some(source.to_string()),
    }
}

fn edge(from: &str, to: &str, kind: &str, evidence: &str) -> RustokFbaGraphEdge {
    RustokFbaGraphEdge {
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
