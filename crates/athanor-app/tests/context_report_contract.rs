use athanor_app::{CONTEXT_PACK_SCHEMA_V1, ContextReport, VersionedJsonContract};
use athanor_domain::{
    ContextLevel, ContextPack, ContextPackId, DiagnosticId, EntityId,
};
use serde_json::{Value, json};

#[test]
fn context_report_v1_matches_golden_fixture() {
    let report = ContextReport::from(ContextPack {
        id: ContextPackId("ctx_fixture".to_string()),
        task: "contract migration".to_string(),
        scope: vec!["module://contract".to_string()],
        level: ContextLevel::Normal,
        language: None,
        summary: "Fixture context pack".to_string(),
        entities: vec![EntityId("ent_contract".to_string())],
        files: vec!["src/lib.rs".to_string()],
        diagnostics: vec![DiagnosticId("diag_contract".to_string())],
        suggested_checks: vec!["cargo test".to_string()],
        confidence: 1.0,
        payload: json!({
            "schema": CONTEXT_PACK_SCHEMA_V1,
            "snapshot": "snap_fixture",
            "estimated_tokens": 42,
        }),
    });

    report
        .validate_contract()
        .expect("context report must satisfy the registered top-level contract");

    let actual = serde_json::to_value(&report).expect("context report must serialize");
    let expected: Value = serde_json::from_str(include_str!("fixtures/context_pack.v1.json"))
        .expect("context report fixture must be valid JSON");

    assert_eq!(actual, expected);
    assert_eq!(actual["schema"], CONTEXT_PACK_SCHEMA_V1);
    assert_eq!(actual["payload"]["schema"], CONTEXT_PACK_SCHEMA_V1);
    assert!(actual.get("pack").is_none(), "flattening must preserve the existing top-level context fields");
}
