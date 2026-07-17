use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use athanor_app::{
    DAEMON_JOBS_CONTRACT_SCHEMA_V1, DAEMON_REQUEST_CONTRACT_SCHEMA_V3,
    DAEMON_REQUEST_SCHEMA_V1, DAEMON_REQUEST_SCHEMA_V2, DAEMON_RESPONSE_CONTRACT_SCHEMA_V3,
    DAEMON_RESPONSE_SCHEMA_V2, DaemonCommand, DaemonError, DaemonErrorCode, DaemonJob,
    DaemonJobKind, DaemonJobStatus, DaemonJobsReport, DaemonRequest, DaemonResponse,
    VERSIONED_JSON_CONTRACTS, VersionedJsonContract,
};
use serde_json::{Value, json};

#[test]
fn daemon_transport_contracts_match_golden_fixture() {
    let request = DaemonRequest {
        schema: DAEMON_REQUEST_CONTRACT_SCHEMA_V3.to_string(),
        request_id: "request-transport-1".to_string(),
        project_id: "project-alpha".to_string(),
        auth_token: Some("redacted-test-token".to_string()),
        command: DaemonCommand::Search {
            query: "authentication".to_string(),
            limit: 5,
            deadline_unix_ms: None,
        },
    };
    let success = DaemonResponse {
        schema: DAEMON_RESPONSE_CONTRACT_SCHEMA_V3.to_string(),
        request_id: request.request_id.clone(),
        project_id: request.project_id.clone(),
        ok: true,
        result: Some(json!({
            "schema": "athanor.search.v1",
            "items": []
        })),
        error: None,
        error_details: None,
    };
    let failure = DaemonResponse {
        schema: DAEMON_RESPONSE_CONTRACT_SCHEMA_V3.to_string(),
        request_id: request.request_id.clone(),
        project_id: request.project_id.clone(),
        ok: false,
        result: None,
        error: Some("entity not found".to_string()),
        error_details: Some(DaemonError {
            code: DaemonErrorCode::NotFound,
            message: "entity not found".to_string(),
            retryable: false,
            details: serde_json::Map::new(),
        }),
    };
    let jobs = DaemonJobsReport {
        schema: DAEMON_JOBS_CONTRACT_SCHEMA_V1.to_string(),
        total: 1,
        returned: 1,
        retention_limit: 100,
        jobs: vec![DaemonJob {
            id: "job_00000001".to_string(),
            kind: DaemonJobKind::Search,
            status: DaemonJobStatus::Succeeded,
            description: "search authentication".to_string(),
            created_at_unix_ms: 1_000,
            started_at_unix_ms: Some(1_001),
            finished_at_unix_ms: Some(1_005),
            result: Some(json!({
                "schema": "athanor.search.v1",
                "items": []
            })),
            error: None,
        }],
    };

    request.validate_contract().expect("valid daemon request contract");
    success.validate_contract().expect("valid daemon success response contract");
    failure.validate_contract().expect("valid daemon error response contract");
    jobs.validate_contract().expect("valid daemon jobs contract");

    let fixture = read_fixture("daemon_transport_contracts.v1.json");
    assert_eq!(serde_json::to_value(request).unwrap(), fixture["request"]);
    assert_eq!(serde_json::to_value(success).unwrap(), fixture["success"]);
    assert_eq!(serde_json::to_value(failure).unwrap(), fixture["failure"]);
    assert_eq!(serde_json::to_value(jobs).unwrap(), fixture["jobs"]);
}

#[test]
fn registry_contains_only_current_top_level_daemon_owners() {
    let schemas = VERSIONED_JSON_CONTRACTS
        .iter()
        .map(|contract| contract.schema)
        .collect::<BTreeSet<_>>();
    let owners = VERSIONED_JSON_CONTRACTS
        .iter()
        .map(|contract| contract.rust_type)
        .collect::<BTreeSet<_>>();

    for schema in [
        DAEMON_REQUEST_CONTRACT_SCHEMA_V3,
        DAEMON_RESPONSE_CONTRACT_SCHEMA_V3,
        DAEMON_JOBS_CONTRACT_SCHEMA_V1,
    ] {
        assert!(schemas.contains(schema), "missing daemon contract {schema}");
    }
    for legacy in [
        DAEMON_REQUEST_SCHEMA_V1,
        DAEMON_REQUEST_SCHEMA_V2,
        DAEMON_RESPONSE_SCHEMA_V2,
    ] {
        assert!(!schemas.contains(legacy), "legacy daemon schema became current: {legacy}");
    }
    for embedded_or_persisted in ["DaemonError", "DaemonCommand", "DaemonJob", "DaemonEndpoint"] {
        assert!(
            !owners.contains(embedded_or_persisted),
            "embedded or persisted daemon type became a public owner: {embedded_or_persisted}"
        );
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
