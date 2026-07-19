use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use athanor_core::CanonicalSnapshot;
use athanor_domain::{
    DiagnosticKind, Entity, EntityId, EntityKind, Ownership, SourceLocation, StableKey,
};
use serde_json::{Value, json};

use super::diff::{
    build_api_contract_diff, diff_api_contracts, endpoint_compatibility_reasons,
    schema_compatibility_reasons,
};
use super::model::{
    API_CONTRACT_SNAPSHOT_SCHEMA, ApiCleanupOptions, ApiContractChangeKind, ApiContractItem,
    ApiContractSnapshot, ApiDiffOptions, ApiRetentionOverrides,
};
use super::retention::cleanup_api_contracts;
use super::snapshot::build_api_contract_snapshot;

static NEXT_TEMP_ROOT: AtomicU64 = AtomicU64::new(1);

#[test]
fn classifies_removed_and_changed_contract_items_as_breaking() {
    let before = contract(
        "snap_old",
        vec![item("api://GET:/users", json!({"responses": ["200"]}))],
        vec![item("api-schema://api#User", json!({"type": "object"}))],
    );
    let after = contract(
        "snap_new",
        Vec::new(),
        vec![item("api-schema://api#User", json!({"type": "string"}))],
    );
    let diff = build_api_contract_diff(&before, &after);
    assert_eq!(diff.breaking_changes, 2);
    assert_eq!(diff.diagnostics.len(), 2);
    assert!(diff.diagnostics.iter().all(|diagnostic| {
        diagnostic.kind == DiagnosticKind::ApiBreakingChangeDetected
            && !diagnostic.evidence.is_empty()
            && !diagnostic.ownership.is_empty()
    }));
    assert!(
        diff.changes
            .iter()
            .any(|change| change.kind == ApiContractChangeKind::EndpointRemoved)
    );
    assert!(
        diff.changes
            .iter()
            .any(|change| change.kind == ApiContractChangeKind::SchemaChanged)
    );
}

#[test]
fn builds_sorted_contract_from_canonical_entities() {
    let canonical = CanonicalSnapshot {
        snapshot: Some(athanor_domain::SnapshotId("snap_test".to_string())),
        entities: vec![
            entity("api://POST:/z", EntityKind::ApiEndpoint),
            entity("api://GET:/a", EntityKind::ApiEndpoint),
        ],
        ..CanonicalSnapshot::default()
    };
    let contract = build_api_contract_snapshot(&canonical).unwrap();
    assert_eq!(contract.endpoints[0].stable_key, "api://GET:/a");
}

#[test]
fn treats_documentation_and_optional_schema_additions_as_non_breaking() {
    let before = contract(
        "snap_old",
        vec![item(
            "api://GET:/users",
            json!({"responses": ["200"], "description": "old"}),
        )],
        vec![item(
            "api-schema://api#User",
            json!({"schema": {"type": "object", "properties": {"id": {"type": "string"}}}}),
        )],
    );
    let after = contract(
        "snap_new",
        vec![item(
            "api://GET:/users",
            json!({"responses": ["200"], "description": "new"}),
        )],
        vec![item(
            "api-schema://api#User",
            json!({"schema": {"type": "object", "properties": {"id": {"type": "string"}, "name": {"type": "string"}}}}),
        )],
    );

    let diff = build_api_contract_diff(&before, &after);
    assert_eq!(diff.changes.len(), 2);
    assert_eq!(diff.breaking_changes, 0);
}

#[test]
fn identifies_status_auth_and_field_level_breaking_changes() {
    let endpoint_reasons = endpoint_compatibility_reasons(
        &json!({"responses": ["200", "404"], "security": [{"oauth": []}]}),
        &json!({"responses": ["200"], "security": []}),
    );
    assert!(endpoint_reasons.contains(&"response_status_removed".to_string()));
    assert!(endpoint_reasons.contains(&"security_changed".to_string()));

    let schema_reasons = schema_compatibility_reasons(
        &json!({"type": "object", "properties": {"id": {"type": "string"}, "name": {"type": "string"}}, "required": ["id"]}),
        &json!({"type": "object", "properties": {"id": {"type": "integer"}}, "required": ["id", "email"]}),
    );
    assert!(schema_reasons.contains(&"required_field_added".to_string()));
    assert!(schema_reasons.contains(&"schema_property_removed".to_string()));
    assert!(schema_reasons.contains(&"property_type_changed:id".to_string()));
}

#[test]
fn ignores_provenance_only_changes_between_snapshot_versions() {
    let before = contract(
        "snap_old",
        vec![item("api://GET:/users", json!({"responses": ["200"]}))],
        Vec::new(),
    );
    let mut after_item = item("api://GET:/users", json!({"responses": ["200"]}));
    after_item.entity_id = Some(EntityId("ent_users".to_string()));
    after_item.source = Some(SourceLocation {
        path: "openapi.yaml".to_string(),
        line_start: Some(1),
        line_end: Some(1),
    });
    after_item.ownership = vec![Ownership {
        source_file: "openapi.yaml".to_string(),
    }];
    let after = contract("snap_new", vec![after_item], Vec::new());

    assert!(build_api_contract_diff(&before, &after).changes.is_empty());
}

#[test]
fn cleanup_api_contracts_retains_latest_and_newest_baseline() {
    let root = temp_root();
    let api_root = root.join(".athanor/api");
    let snapshots_dir = api_root.join("snapshots");
    let diffs_dir = api_root.join("diffs");
    fs::create_dir_all(&snapshots_dir).unwrap();
    fs::create_dir_all(&diffs_dir).unwrap();
    for snapshot in [
        "snap_jsonl_00000001",
        "snap_jsonl_00000002",
        "snap_jsonl_00000003",
    ] {
        fs::write(
            snapshots_dir.join(format!("{snapshot}.json")),
            serde_json::to_string(&contract(snapshot, Vec::new(), Vec::new())).unwrap(),
        )
        .unwrap();
    }
    fs::write(
        api_root.join("latest.json"),
        r#"{"schema":"athanor.api_contract_latest.v1","snapshot":"snap_jsonl_00000003","path":"snapshots/snap_jsonl_00000003.json"}"#,
    )
    .unwrap();
    fs::write(
        diffs_dir.join("snap_jsonl_00000001--snap_jsonl_00000002.json"),
        "{}",
    )
    .unwrap();
    fs::write(
        diffs_dir.join("snap_jsonl_00000002--snap_jsonl_00000003.json"),
        "{}",
    )
    .unwrap();

    let report = cleanup_api_contracts(ApiCleanupOptions {
        root: root.clone(),
        dry_run: false,
        keep_snapshots: 2,
        keep_diffs: 1,
    })
    .unwrap();

    assert!(!snapshots_dir.join("snap_jsonl_00000001.json").exists());
    assert!(snapshots_dir.join("snap_jsonl_00000002.json").is_file());
    assert!(snapshots_dir.join("snap_jsonl_00000003.json").is_file());
    assert!(
        !diffs_dir
            .join("snap_jsonl_00000001--snap_jsonl_00000002.json")
            .exists()
    );
    assert!(
        diffs_dir
            .join("snap_jsonl_00000002--snap_jsonl_00000003.json")
            .is_file()
    );
    assert_eq!(report.removed.len(), 2);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn cleanup_api_contracts_dry_run_does_not_remove() {
    let root = temp_root();
    let snapshots_dir = root.join(".athanor/api/snapshots");
    fs::create_dir_all(&snapshots_dir).unwrap();
    for snapshot in ["snap_jsonl_00000001", "snap_jsonl_00000002"] {
        fs::write(
            snapshots_dir.join(format!("{snapshot}.json")),
            serde_json::to_string(&contract(snapshot, Vec::new(), Vec::new())).unwrap(),
        )
        .unwrap();
    }

    let report = cleanup_api_contracts(ApiCleanupOptions {
        root: root.clone(),
        dry_run: true,
        keep_snapshots: 1,
        keep_diffs: 0,
    })
    .unwrap();

    assert_eq!(report.removed.len(), 1);
    assert!(snapshots_dir.join("snap_jsonl_00000001.json").is_file());
    assert!(snapshots_dir.join("snap_jsonl_00000002.json").is_file());

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn diff_api_contracts_runs_configured_auto_cleanup() {
    let root = temp_root();
    let api_root = root.join(".athanor/api");
    let snapshots_dir = api_root.join("snapshots");
    let diffs_dir = api_root.join("diffs");
    fs::create_dir_all(&snapshots_dir).unwrap();
    fs::create_dir_all(&diffs_dir).unwrap();
    fs::write(
        root.join("athanor.toml"),
        r#"[api.retention]
auto_cleanup = true
keep_snapshots = 2
keep_diffs = 1
"#,
    )
    .unwrap();
    for snapshot in [
        "snap_jsonl_00000001",
        "snap_jsonl_00000002",
        "snap_jsonl_00000003",
    ] {
        fs::write(
            snapshots_dir.join(format!("{snapshot}.json")),
            serde_json::to_string(&contract(snapshot, Vec::new(), Vec::new())).unwrap(),
        )
        .unwrap();
    }
    fs::write(
        api_root.join("latest.json"),
        r#"{"schema":"athanor.api_contract_latest.v1","snapshot":"snap_jsonl_00000003","path":"snapshots/snap_jsonl_00000003.json"}"#,
    )
    .unwrap();
    fs::write(
        diffs_dir.join("snap_jsonl_00000001--snap_jsonl_00000002.json"),
        "{}",
    )
    .unwrap();

    let diff = diff_api_contracts(ApiDiffOptions {
        root: root.clone(),
        from: Some("snap_jsonl_00000002".to_string()),
        to: Some("snap_jsonl_00000003".to_string()),
        retention: ApiRetentionOverrides::default(),
    })
    .unwrap();

    let cleanup = diff.cleanup.expect("auto cleanup report");
    assert_eq!(cleanup.removed.len(), 2);
    assert!(!snapshots_dir.join("snap_jsonl_00000001.json").exists());
    assert!(snapshots_dir.join("snap_jsonl_00000002.json").is_file());
    assert!(snapshots_dir.join("snap_jsonl_00000003.json").is_file());
    assert!(
        !diffs_dir
            .join("snap_jsonl_00000001--snap_jsonl_00000002.json")
            .exists()
    );
    assert!(
        diffs_dir
            .join("snap_jsonl_00000002--snap_jsonl_00000003.json")
            .is_file()
    );

    fs::remove_dir_all(root).unwrap();
}

fn contract(
    snapshot: &str,
    endpoints: Vec<ApiContractItem>,
    schemas: Vec<ApiContractItem>,
) -> ApiContractSnapshot {
    ApiContractSnapshot {
        schema: API_CONTRACT_SNAPSHOT_SCHEMA.to_string(),
        snapshot: snapshot.to_string(),
        endpoints,
        schemas,
        examples: Vec::new(),
    }
}

fn item(stable_key: &str, payload: Value) -> ApiContractItem {
    ApiContractItem {
        entity_id: None,
        stable_key: stable_key.to_string(),
        name: stable_key.to_string(),
        source: None,
        ownership: Vec::new(),
        payload,
    }
}

fn entity(stable_key: &str, kind: EntityKind) -> Entity {
    Entity {
        id: EntityId(stable_key.to_string()),
        stable_key: StableKey(stable_key.to_string()),
        kind,
        name: stable_key.to_string(),
        title: None,
        source: None,
        language: None,
        aliases: Vec::new(),
        ownership: Vec::new(),
        payload: json!({}),
    }
}

fn temp_root() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "athanor-api-test-{}-{}",
        std::process::id(),
        NEXT_TEMP_ROOT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&root).unwrap();
    root
}
