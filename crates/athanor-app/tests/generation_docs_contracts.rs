use std::fs;
use std::path::{Path, PathBuf};

use athanor_app::{
    DOCS_APPLY_PATCH_SCHEMA_V1, DOCS_CHECK_SCHEMA_V1, DOCS_DRIFT_SCHEMA_V1, DOCS_PATCH_SCHEMA,
    DocsApplyPatchReport, DocsCheckReport, DocsDriftReport, DocsFrontmatterChange,
    DocsPatchOperation, DocsPatchProposal, DocsPolicyViolation, DriftedDocument,
    GENERATION_SCHEMA_V1, GenerationMetrics, GenerationReport, GenerationStatus,
    VersionedJsonContract, validate_contract_value,
};
use serde_json::{Value, json};

#[test]
fn generation_and_docs_contracts_match_golden_fixture() {
    let generation = GenerationReport {
        schema: GENERATION_SCHEMA_V1,
        status: GenerationStatus::Published,
        root: PathBuf::from("project"),
        generation: "00000007".to_string(),
        generation_dir: PathBuf::from(".athanor/generated/generations/00000007"),
        current_pointer: PathBuf::from(".athanor/generated/current.json"),
        snapshot: "snap-generation".to_string(),
        entities: 12,
        facts: 18,
        relations: 7,
        diagnostics: 2,
        metrics: GenerationMetrics {
            schema: "athanor.generation_metrics.v1",
            total_ms: 50,
            snapshot_load_ms: 5,
            jsonl_ms: 10,
            wiki_ms: 11,
            html_ms: 12,
            publish_ms: 12,
        },
    };
    let docs_check = DocsCheckReport {
        schema: DOCS_CHECK_SCHEMA_V1.to_string(),
        snapshot: "snap-docs".to_string(),
        passed: false,
        editable_documents: 3,
        policy_violations: vec![DocsPolicyViolation {
            path: "docs/catalog.md".to_string(),
            stable_key: "doc://docs/catalog.md".to_string(),
            field: "last_verified_snapshot".to_string(),
            message: "verification snapshot is stale".to_string(),
        }],
        diagnostics: Vec::new(),
    };
    let docs_drift = DocsDriftReport {
        schema: DOCS_DRIFT_SCHEMA_V1.to_string(),
        snapshot: "snap-docs".to_string(),
        editable_documents: 3,
        current_documents: 2,
        drifted_documents: vec![DriftedDocument {
            path: "docs/catalog.md".to_string(),
            stable_key: "doc://docs/catalog.md".to_string(),
            verified_snapshot: Some("snap-old".to_string()),
            reason: "verified_against_older_snapshot".to_string(),
        }],
    };
    let docs_apply_patch = DocsApplyPatchReport {
        schema: DOCS_APPLY_PATCH_SCHEMA_V1.to_string(),
        id: "docs-patch-snap-docs".to_string(),
        snapshot: "snap-docs".to_string(),
        files_changed: 1,
        changes_applied: 1,
    };
    let docs_patch_proposal = DocsPatchProposal {
        schema: DOCS_PATCH_SCHEMA.to_string(),
        id: "docs-patch-snap-docs".to_string(),
        snapshot: "snap-docs".to_string(),
        operations: vec![DocsPatchOperation {
            path: "docs/catalog.md".to_string(),
            stable_key: "doc://docs/catalog.md".to_string(),
            create: false,
            content: None,
            changes: vec![DocsFrontmatterChange {
                field: "last_verified_snapshot".to_string(),
                old_value: Some(json!("snap-old")),
                new_value: json!("snap-docs"),
                reason: "verified_against_older_snapshot".to_string(),
            }],
        }],
    };

    generation
        .validate_contract()
        .expect("valid generation contract");
    docs_check
        .validate_contract()
        .expect("valid docs check contract");
    docs_drift
        .validate_contract()
        .expect("valid docs drift contract");
    docs_apply_patch
        .validate_contract()
        .expect("valid docs apply patch contract");

    let proposal_value = serde_json::to_value(&docs_patch_proposal).unwrap();
    validate_contract_value(DOCS_PATCH_SCHEMA, &proposal_value)
        .expect("valid docs patch interchange artifact");

    let fixture = read_fixture("generation_docs_contracts.v1.json");
    assert_eq!(
        serde_json::to_value(generation).unwrap(),
        fixture["generation"]
    );
    assert_eq!(
        serde_json::to_value(docs_check).unwrap(),
        fixture["docs_check"]
    );
    assert_eq!(
        serde_json::to_value(docs_drift).unwrap(),
        fixture["docs_drift"]
    );
    assert_eq!(
        serde_json::to_value(docs_apply_patch).unwrap(),
        fixture["docs_apply_patch"]
    );
    assert_eq!(proposal_value, fixture["docs_patch_proposal"]);
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
