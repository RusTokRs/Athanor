use std::fs;
use std::path::Path;

use athanor_app::{
    RUSTOK_ARCHITECTURE_CONTEXT_SCHEMA_V1, RustokArchitectureContext,
    RustokArchitectureContract, RustokArchitectureDiagnostic, RustokArchitectureEvidence,
    RustokArchitectureInteraction, RustokArchitectureModule, RustokArchitectureOmitted,
    RustokArchitectureResolution, RustokArchitectureTest, VersionedJsonContract,
};
use serde_json::Value;

#[test]
fn rustok_architecture_context_matches_golden_fixture() {
    let report = RustokArchitectureContext {
        schema: RUSTOK_ARCHITECTURE_CONTEXT_SCHEMA_V1.to_string(),
        snapshot: "snap-architecture".to_string(),
        intent: "change catalog product rendering".to_string(),
        resolution: RustokArchitectureResolution {
            status: "resolved".to_string(),
            primary_module: Some("catalog".to_string()),
            candidates: vec!["catalog".to_string()],
            summary: "Resolved catalog as the primary module".to_string(),
        },
        modules: vec![RustokArchitectureModule {
            slug: "catalog".to_string(),
            score: 104,
            reasons: vec![
                "explicit module selector".to_string(),
                "context entity ownership".to_string(),
            ],
        }],
        contracts: vec![RustokArchitectureContract {
            stable_key: "rustok-fba://catalog/read".to_string(),
            kind: "rustok_fba_port".to_string(),
            module: "catalog".to_string(),
            source: Some("contracts/catalog.json".to_string()),
        }],
        interactions: vec![RustokArchitectureInteraction {
            consumer: "catalog-page".to_string(),
            provider: "catalog-provider".to_string(),
            profile: "catalog-products".to_string(),
            stable_key: "rustok-page-builder://catalog/products".to_string(),
            source: Some("src/catalog/page.rs".to_string()),
        }],
        tests: vec![RustokArchitectureTest {
            stable_key: "rust-test://catalog/products".to_string(),
            name: "catalog products contract".to_string(),
            source: Some("tests/catalog_products.rs".to_string()),
        }],
        diagnostics: vec![RustokArchitectureDiagnostic {
            kind: "missing_documentation".to_string(),
            severity: "medium".to_string(),
            message: "Document the catalog fallback profile".to_string(),
            source: Some("src/catalog/page.rs".to_string()),
        }],
        evidence: vec![RustokArchitectureEvidence {
            stable_key: "rustok-fba://catalog/read".to_string(),
            source: "contracts/catalog.json".to_string(),
        }],
        guidance: vec!["Update the catalog port contract before its consumers.".to_string()],
        omitted: RustokArchitectureOmitted {
            modules: 0,
            contracts: 0,
            interactions: 0,
            evidence: 0,
        },
    };

    report
        .validate_contract()
        .expect("valid Rustok architecture context contract");
    assert_eq!(
        serde_json::to_value(report).unwrap(),
        read_fixture("rustok_architecture_context.v1.json")
    );
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
