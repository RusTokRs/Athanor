use athanor_app::{
    DOCUMENTATION_GENERATION_LIMIT_MAX, DOCUMENTATION_GENERATION_MANIFEST_SCHEMA_V1,
    DOCUMENTATION_GENERATION_REQUEST_SCHEMA_V1, DocumentationGenerationManifest,
    DocumentationGenerationRequest, VERSIONED_JSON_CONTRACTS,
};
use serde_json::Value;

const FIXTURE: &str = include_str!("fixtures/documentation_generation_contracts.v1.json");

#[test]
fn request_and_manifest_fixtures_are_strict_aligned_and_round_trip() {
    let fixture: Value = serde_json::from_str(FIXTURE).expect("valid documentation fixture");
    let request: DocumentationGenerationRequest =
        serde_json::from_value(fixture["request"].clone()).expect("typed request fixture");
    let manifest: DocumentationGenerationManifest =
        serde_json::from_value(fixture["manifest"].clone()).expect("typed manifest fixture");

    request.validate().expect("valid request contract");
    manifest
        .validate_for_request(&request)
        .expect("manifest must align with request");
    assert_eq!(serde_json::to_value(&request).unwrap(), fixture["request"]);
    assert_eq!(
        serde_json::to_value(&manifest).unwrap(),
        fixture["manifest"]
    );
}

#[test]
fn request_and_manifest_are_owned_by_the_public_contract_registry() {
    for (schema, rust_type) in [
        (
            DOCUMENTATION_GENERATION_REQUEST_SCHEMA_V1,
            "DocumentationGenerationRequest",
        ),
        (
            DOCUMENTATION_GENERATION_MANIFEST_SCHEMA_V1,
            "DocumentationGenerationManifest",
        ),
    ] {
        let descriptor = VERSIONED_JSON_CONTRACTS
            .iter()
            .find(|descriptor| descriptor.schema == schema)
            .unwrap_or_else(|| panic!("missing documentation contract {schema}"));
        assert_eq!(descriptor.rust_type, rust_type);
    }
}

#[test]
fn schema_drift_unknown_fields_and_request_mismatches_fail_closed() {
    let fixture: Value = serde_json::from_str(FIXTURE).expect("valid documentation fixture");

    let mut wrong_schema = fixture["request"].clone();
    wrong_schema["schema"] = Value::String("athanor.documentation_generation_request.v2".into());
    let wrong_schema: DocumentationGenerationRequest =
        serde_json::from_value(wrong_schema).expect("schema validation is explicit");
    assert!(wrong_schema.validate().is_err());

    let mut unknown_field = fixture["request"].clone();
    unknown_field["raw_source_access"] = Value::Bool(true);
    assert!(serde_json::from_value::<DocumentationGenerationRequest>(unknown_field).is_err());

    let mut unbounded_request: DocumentationGenerationRequest =
        serde_json::from_value(fixture["request"].clone()).unwrap();
    unbounded_request.limits.max_entities = DOCUMENTATION_GENERATION_LIMIT_MAX + 1;
    assert!(unbounded_request.validate().is_err());

    let request: DocumentationGenerationRequest =
        serde_json::from_value(fixture["request"].clone()).unwrap();
    let mut mismatched_snapshot: DocumentationGenerationManifest =
        serde_json::from_value(fixture["manifest"].clone()).unwrap();
    mismatched_snapshot.snapshot = "snap-other".to_string();
    assert!(mismatched_snapshot.validate_for_request(&request).is_err());
}

#[test]
fn unsafe_non_portable_and_colliding_outputs_fail_closed() {
    let fixture: Value = serde_json::from_str(FIXTURE).expect("valid documentation fixture");
    let request: DocumentationGenerationRequest =
        serde_json::from_value(fixture["request"].clone()).unwrap();

    for path in [
        "../editable.md",
        "C:/editable.md",
        "architecture/NUL.md",
        "architecture/trailing. ",
    ] {
        let mut manifest: DocumentationGenerationManifest =
            serde_json::from_value(fixture["manifest"].clone()).unwrap();
        manifest.documents[0].path = path.to_string();
        assert!(
            manifest.validate_for_request(&request).is_err(),
            "accepted unsafe output path {path}"
        );
    }

    let mut colliding_paths: DocumentationGenerationManifest =
        serde_json::from_value(fixture["manifest"].clone()).unwrap();
    let mut duplicate = colliding_paths.documents[0].clone();
    duplicate.id = "architecture-overview-copy".to_string();
    duplicate.path = "Architecture/INDEX.md".to_string();
    colliding_paths.documents.push(duplicate);
    assert!(colliding_paths.validate_for_request(&request).is_err());

    let mut invalid_hash: DocumentationGenerationManifest =
        serde_json::from_value(fixture["manifest"].clone()).unwrap();
    invalid_hash.documents[0].sha256 = "SHA256:fixture".to_string();
    assert!(invalid_hash.validate_for_request(&request).is_err());
}
