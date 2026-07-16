use std::fs;
use std::path::Path;

use athanor_app::{
    GraphOmitted, RUSTOK_FFA_AUDIT_SCHEMA_V1, RustokFfaAudit, RustokFfaAuditSummary,
    RustokFfaAuditSurface, RustokFfaGraph, RustokFfaGraphEdge, RustokFfaGraphNode,
    RustokFfaSurfaceGraphReport, RustokFfaViolationsGraphReport, VersionedJsonContract,
};
use serde_json::Value;

#[test]
fn representative_ffa_contracts_match_golden_fixture() {
    let audit = RustokFfaAudit {
        schema: RUSTOK_FFA_AUDIT_SCHEMA_V1.to_string(),
        snapshot: "snap-ffa".to_string(),
        summary: RustokFfaAuditSummary {
            observed_surfaces: 1,
            surfaces_total: 1,
            core_transport_ui: 1,
            incomplete: 0,
            requirements_met: 4,
            requirements_total: 4,
            completion_percent: Some(100),
            missing_core: 0,
            missing_transport: 0,
            missing_ui_adapter: 0,
            scaffold_surfaces: 0,
            host_wiring_surfaces: 1,
            diagnostics_open: 0,
        },
        surfaces: vec![RustokFfaAuditSurface {
            id: "ffa_surface://catalog/products".to_string(),
            module: "catalog".to_string(),
            surface: "products".to_string(),
            shape: "core_transport_ui".to_string(),
            actionable: true,
            requirements_met: 4,
            requirements_total: 4,
            completion_percent: Some(100),
            core_present: true,
            transport_present: true,
            ui_adapter_present: true,
            host_wiring_present: true,
            diagnostics_open: 0,
            layers: vec![
                "core".to_string(),
                "transport".to_string(),
                "ui_adapter".to_string(),
            ],
            files: vec![
                "src/catalog/core.rs".to_string(),
                "src/catalog/transport.rs".to_string(),
                "src/catalog/ui.rs".to_string(),
            ],
            diagnostics: Vec::new(),
        }],
    };

    let surface = RustokFfaSurfaceGraphReport::new(RustokFfaGraph {
        schema: "overwritten-by-wrapper".to_string(),
        snapshot: "snap-ffa".to_string(),
        surface: Some("products".to_string()),
        nodes: vec![
            node("core", "Catalog core", "src/catalog/core.rs"),
            node(
                "transport",
                "Catalog transport",
                "src/catalog/transport.rs",
            ),
        ],
        edges: vec![RustokFfaGraphEdge {
            from: "core".to_string(),
            to: "transport".to_string(),
            kind: "rustok_ffa_layer_link".to_string(),
            evidence: vec!["src/catalog/transport.rs:12".to_string()],
        }],
        diagnostics: Vec::new(),
        omitted: omitted(),
    });

    let violations = RustokFfaViolationsGraphReport::new(RustokFfaGraph {
        schema: "overwritten-by-wrapper".to_string(),
        snapshot: "snap-ffa".to_string(),
        surface: None,
        nodes: vec![RustokFfaGraphNode {
            id: "missing-ui".to_string(),
            kind: "rustok_ffa_violation".to_string(),
            name: "Missing UI adapter".to_string(),
            source: Some("src/catalog/core.rs".to_string()),
        }],
        edges: vec![RustokFfaGraphEdge {
            from: "missing-ui".to_string(),
            to: "core".to_string(),
            kind: "rustok_ffa_violation_for".to_string(),
            evidence: vec!["src/catalog/core.rs:4".to_string()],
        }],
        diagnostics: Vec::new(),
        omitted: omitted(),
    });

    audit.validate_contract().expect("valid FFA audit contract");
    surface
        .validate_contract()
        .expect("valid FFA surface graph contract");
    violations
        .validate_contract()
        .expect("valid FFA violations graph contract");

    let fixture = read_fixture("rustok_ffa_contracts.v1.json");
    assert_eq!(serde_json::to_value(audit).unwrap(), fixture["audit"]);
    assert_eq!(
        serde_json::to_value(surface).unwrap(),
        fixture["surface_graph"]
    );
    assert_eq!(
        serde_json::to_value(violations).unwrap(),
        fixture["violations_graph"]
    );
}

fn node(id: &str, name: &str, source: &str) -> RustokFfaGraphNode {
    RustokFfaGraphNode {
        id: id.to_string(),
        kind: "rustok_ffa_layer".to_string(),
        name: name.to_string(),
        source: Some(source.to_string()),
    }
}

fn omitted() -> GraphOmitted {
    GraphOmitted {
        nodes: 0,
        edges: 0,
        reason: "limits".to_string(),
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
