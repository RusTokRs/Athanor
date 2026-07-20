use std::path::PathBuf;

use athanor_core::CanonicalSnapshot;
use athanor_domain::{
    Diagnostic, DiagnosticId, DiagnosticKind, DiagnosticStatus, Entity, EntityId, EntityKind,
    Evidence, EvidenceStatus, Ownership, Relation, RelationId, RelationKind, RelationStatus,
    Severity, SnapshotId, StableKey,
};
use serde_json::json;

use super::evidence::entity_evidence;
use super::execution::validate_options;
use super::model::{ChangeMapLimits, ChangeMapOptions, ChangeMapQuery, ChangeMapTestStatus, Seed};
use super::ranking::build_change_map;
use crate::json_contract::CHANGE_MAP_SCHEMA_V1;

fn entity(id: &str, key: &str, kind: EntityKind, path: &str, schema: Option<&str>) -> Entity {
    Entity {
        id: EntityId(id.to_string()),
        stable_key: StableKey(key.to_string()),
        kind,
        name: key.to_string(),
        title: None,
        source: Some(athanor_domain::SourceLocation {
            path: path.to_string(),
            line_start: Some(1),
            line_end: Some(2),
        }),
        language: None,
        aliases: Vec::new(),
        ownership: vec![Ownership {
            source_file: path.to_string(),
        }],
        payload: schema.map_or_else(|| json!({}), |schema| json!({"schema": schema})),
    }
}

fn relation(
    id: &str,
    kind: RelationKind,
    from: &Entity,
    to: &Entity,
    schema: Option<&str>,
) -> Relation {
    Relation {
        id: RelationId(id.to_string()),
        kind,
        from: from.id.clone(),
        to: to.id.clone(),
        status: RelationStatus::Verified,
        confidence: 0.9,
        evidence: vec![Evidence {
            source_file: Some("src/link.rs".to_string()),
            line_start: Some(3),
            line_end: Some(3),
            extractor: Some("test".to_string()),
            commit_hash: None,
            confidence: 0.9,
            status: EvidenceStatus::Verified,
        }],
        ownership: vec![Ownership {
            source_file: "src/link.rs".to_string(),
        }],
        snapshot: SnapshotId("snap-1".to_string()),
        payload: schema.map_or_else(|| json!({}), |schema| json!({"schema": schema})),
    }
}

fn fixture() -> (CanonicalSnapshot, Entity) {
    let endpoint = entity(
        "endpoint",
        "api://GET:/users",
        EntityKind::ApiEndpoint,
        "openapi.yaml",
        None,
    );
    let handler = entity(
        "handler",
        "rust://users/list",
        EntityKind::Function,
        "src/users.rs",
        None,
    );
    let test = entity(
        "test",
        "rust-test://users/list",
        EntityKind::TestCase,
        "tests/users.rs",
        None,
    );
    let platform = entity(
        "fba",
        "rustok-fba://users",
        EntityKind::Other("rustok_fba_module".to_string()),
        "contracts/users-fba-registry.json",
        Some("rustok.fba.entity.v1"),
    );
    let relations = vec![
        relation(
            "implemented",
            RelationKind::ImplementedBy,
            &endpoint,
            &handler,
            None,
        ),
        relation("tested", RelationKind::TestedBy, &handler, &test, None),
        relation(
            "platform",
            RelationKind::Other("rustok_fba_owns".to_string()),
            &platform,
            &handler,
            Some("rustok.fba.relation.v1"),
        ),
    ];
    (
        CanonicalSnapshot {
            snapshot: Some(SnapshotId("snap-1".to_string())),
            entities: vec![endpoint.clone(), handler, test, platform],
            facts: Vec::new(),
            relations,
            diagnostics: Vec::new(),
        },
        endpoint,
    )
}

#[test]
fn builds_deterministic_relation_chains_and_test_coverage() {
    let (snapshot, endpoint) = fixture();
    let report = build_change_map(
        &snapshot,
        ChangeMapQuery {
            task: Some("change users API".to_string()),
            target: None,
            diff: false,
            changed_files: Vec::new(),
        },
        vec![Seed {
            id: endpoint.id,
            score: 1_000,
            reason: "task match".to_string(),
        }],
        ChangeMapLimits {
            max_entities: 10,
            max_files: 10,
            max_diagnostics: 10,
            max_depth: 3,
        },
    );

    assert_eq!(report.schema, CHANGE_MAP_SCHEMA_V1);
    let handler = report
        .items
        .iter()
        .find(|item| item.entity.id.0 == "handler")
        .unwrap();
    assert_eq!(handler.path[0].relation_id, "implemented");
    assert_eq!(handler.test_coverage.status, ChangeMapTestStatus::Linked);
    assert_eq!(handler.test_coverage.tests, ["rust-test://users/list"]);
    let platform = report
        .items
        .iter()
        .find(|item| item.entity.id.0 == "fba")
        .unwrap();
    assert!(
        platform
            .annotations
            .iter()
            .any(|item| item.source == "rustok")
    );
}

#[test]
fn reports_limits_missing_tests_and_open_diagnostics() {
    let (mut snapshot, endpoint) = fixture();
    snapshot.diagnostics.push(Diagnostic {
        id: DiagnosticId("diag-1".to_string()),
        kind: DiagnosticKind::UncoveredSymbol,
        severity: Severity::High,
        status: DiagnosticStatus::Open,
        title: "Missing test".to_string(),
        message: "Add a test".to_string(),
        entities: vec![endpoint.id.clone()],
        evidence: entity_evidence(&endpoint),
        ownership: endpoint.ownership.clone(),
        snapshot: SnapshotId("snap-1".to_string()),
        suggested_fix: None,
        payload: json!({}),
    });
    let report = build_change_map(
        &snapshot,
        ChangeMapQuery {
            task: None,
            target: Some(endpoint.stable_key.0.clone()),
            diff: false,
            changed_files: Vec::new(),
        },
        vec![Seed {
            id: endpoint.id,
            score: 1_200,
            reason: "target".to_string(),
        }],
        ChangeMapLimits {
            max_entities: 1,
            max_files: 1,
            max_diagnostics: 1,
            max_depth: 3,
        },
    );

    assert!(report.omitted.entities > 0);
    assert_eq!(report.returned.diagnostics, 1);
    assert_eq!(
        report.items[0].test_coverage.status,
        ChangeMapTestStatus::NotLinked
    );
    assert!(
        report.items[0]
            .reasons
            .iter()
            .any(|reason| reason.contains("no linked test"))
    );
}

#[test]
fn validates_options_requiring_task_target_or_diff() {
    let opts = ChangeMapOptions {
        root: PathBuf::from("."),
        task: None,
        target: None,
        diff: false,
        max_entities: 30,
        max_files: 20,
        max_diagnostics: 20,
        max_depth: 3,
    };
    assert!(validate_options(&opts).is_err());
}

#[test]
fn validates_options_accepting_empty_task() {
    let opts = ChangeMapOptions {
        root: PathBuf::from("."),
        task: Some("   ".to_string()),
        target: None,
        diff: false,
        max_entities: 30,
        max_files: 20,
        max_diagnostics: 20,
        max_depth: 3,
    };
    assert!(validate_options(&opts).is_err());
}

#[test]
fn validates_options_requiring_positive_limits() {
    let opts = ChangeMapOptions {
        root: PathBuf::from("."),
        task: Some("test".to_string()),
        target: None,
        diff: false,
        max_entities: 0,
        max_files: 20,
        max_diagnostics: 20,
        max_depth: 3,
    };
    assert!(validate_options(&opts).is_err());
    let opts = ChangeMapOptions {
        max_files: 0,
        ..opts
    };
    assert!(validate_options(&opts).is_err());
    let opts = ChangeMapOptions {
        max_entities: 30,
        max_diagnostics: 0,
        ..opts
    };
    assert!(validate_options(&opts).is_err());
}

#[test]
fn deduplicates_seeds_keeping_highest_score() {
    let (snapshot, endpoint) = fixture();
    let seeds = vec![
        Seed {
            id: endpoint.id.clone(),
            score: 800,
            reason: "low score".to_string(),
        },
        Seed {
            id: endpoint.id.clone(),
            score: 1_200,
            reason: "high score".to_string(),
        },
    ];
    let report = build_change_map(
        &snapshot,
        ChangeMapQuery {
            task: None,
            target: None,
            diff: false,
            changed_files: Vec::new(),
        },
        seeds,
        ChangeMapLimits {
            max_entities: 10,
            max_files: 10,
            max_diagnostics: 10,
            max_depth: 3,
        },
    );
    let endpoint_item = report
        .items
        .iter()
        .find(|item| item.entity.id == endpoint.id)
        .unwrap();
    assert_eq!(endpoint_item.score, 1_200);
    assert!(
        endpoint_item
            .reasons
            .iter()
            .any(|reason| reason.contains("high score")),
        "reasons: {:?}",
        endpoint_item.reasons
    );
    assert!(
        endpoint_item
            .reasons
            .iter()
            .any(|reason| reason.contains("low score")),
        "reasons: {:?}",
        endpoint_item.reasons
    );
}

#[test]
fn truncates_bfs_at_max_depth() {
    let endpoint = entity(
        "e1",
        "api://GET:/a",
        EntityKind::ApiEndpoint,
        "a.yaml",
        None,
    );
    let mid = entity("e2", "rust://a/b", EntityKind::Function, "a.rs", None);
    let deep = entity("e3", "rust://b/c", EntityKind::Function, "b.rs", None);
    let deeper = entity("e4", "rust://c/d", EntityKind::Function, "c.rs", None);
    let relations = vec![
        relation("r1", RelationKind::ImplementedBy, &endpoint, &mid, None),
        relation("r2", RelationKind::Calls, &mid, &deep, None),
        relation("r3", RelationKind::Calls, &deep, &deeper, None),
    ];
    let snapshot = CanonicalSnapshot {
        snapshot: Some(SnapshotId("snap-1".to_string())),
        entities: vec![endpoint.clone(), mid, deep, deeper],
        facts: Vec::new(),
        relations,
        diagnostics: Vec::new(),
    };
    let report = build_change_map(
        &snapshot,
        ChangeMapQuery {
            task: None,
            target: None,
            diff: false,
            changed_files: Vec::new(),
        },
        vec![Seed {
            id: endpoint.id,
            score: 1_000,
            reason: "seed".to_string(),
        }],
        ChangeMapLimits {
            max_entities: 10,
            max_files: 10,
            max_diagnostics: 10,
            max_depth: 2,
        },
    );
    let ids = report
        .items
        .iter()
        .map(|item| item.entity.id.0.as_str())
        .collect::<Vec<_>>();
    assert!(ids.contains(&"e1"));
    assert!(ids.contains(&"e2"));
    assert!(ids.contains(&"e3"));
    assert!(
        !ids.contains(&"e4"),
        "depth=2 should not reach e4, got: {ids:?}"
    );
}

#[test]
fn ranks_implemented_by_higher_than_contains() {
    let endpoint = entity(
        "e1",
        "api://GET:/x",
        EntityKind::ApiEndpoint,
        "api.yaml",
        None,
    );
    let function = entity(
        "e2",
        "rust://x/handle",
        EntityKind::Function,
        "handle.rs",
        None,
    );
    let module = entity("e3", "rust://x/mod", EntityKind::Module, "mod.rs", None);
    let relations = vec![
        relation(
            "impl",
            RelationKind::ImplementedBy,
            &endpoint,
            &function,
            None,
        ),
        relation("contains", RelationKind::Contains, &function, &module, None),
    ];
    let snapshot = CanonicalSnapshot {
        snapshot: Some(SnapshotId("snap-1".to_string())),
        entities: vec![endpoint.clone(), function, module],
        facts: Vec::new(),
        relations,
        diagnostics: Vec::new(),
    };
    let report = build_change_map(
        &snapshot,
        ChangeMapQuery {
            task: None,
            target: None,
            diff: false,
            changed_files: Vec::new(),
        },
        vec![Seed {
            id: endpoint.id,
            score: 1_000,
            reason: "seed".to_string(),
        }],
        ChangeMapLimits {
            max_entities: 10,
            max_files: 10,
            max_diagnostics: 10,
            max_depth: 3,
        },
    );
    let function_item = report
        .items
        .iter()
        .find(|item| item.entity.id.0 == "e2")
        .unwrap();
    let module_item = report
        .items
        .iter()
        .find(|item| item.entity.id.0 == "e3")
        .unwrap();
    assert!(
        function_item.score > module_item.score,
        "ImplementedBy should score higher than Contains: function={} module={}",
        function_item.score,
        module_item.score
    );
}

#[test]
fn diversifies_entities_across_files() {
    let first = entity(
        "e1",
        "api://GET:/a",
        EntityKind::ApiEndpoint,
        "same.yaml",
        None,
    );
    let second = entity(
        "e2",
        "api://GET:/b",
        EntityKind::ApiEndpoint,
        "same.yaml",
        None,
    );
    let third = entity("e3", "rust://c", EntityKind::Function, "other.rs", None);
    let snapshot = CanonicalSnapshot {
        snapshot: Some(SnapshotId("snap-1".to_string())),
        entities: vec![first.clone(), second.clone(), third.clone()],
        facts: Vec::new(),
        relations: Vec::new(),
        diagnostics: Vec::new(),
    };
    let report = build_change_map(
        &snapshot,
        ChangeMapQuery {
            task: None,
            target: None,
            diff: false,
            changed_files: Vec::new(),
        },
        vec![
            Seed {
                id: first.id.clone(),
                score: 1_000,
                reason: "seed1".to_string(),
            },
            Seed {
                id: second.id.clone(),
                score: 990,
                reason: "seed2".to_string(),
            },
            Seed {
                id: third.id.clone(),
                score: 980,
                reason: "seed3".to_string(),
            },
        ],
        ChangeMapLimits {
            max_entities: 2,
            max_files: 10,
            max_diagnostics: 10,
            max_depth: 1,
        },
    );
    assert_eq!(report.items.len(), 2);
    let files = report
        .items
        .iter()
        .flat_map(|item| item.files.iter())
        .map(String::as_str)
        .collect::<Vec<_>>();
    assert!(
        files.contains(&"same.yaml") && files.contains(&"other.rs"),
        "diversification should pick entities from different files, got: {files:?}"
    );
}

#[test]
fn builds_file_aggregation_from_items() {
    let endpoint = entity(
        "e1",
        "api://GET:/u",
        EntityKind::ApiEndpoint,
        "openapi.yaml",
        None,
    );
    let handler = entity("e2", "rust://u/h", EntityKind::Function, "src/u.rs", None);
    let relations = vec![relation(
        "r1",
        RelationKind::ImplementedBy,
        &endpoint,
        &handler,
        None,
    )];
    let snapshot = CanonicalSnapshot {
        snapshot: Some(SnapshotId("snap-1".to_string())),
        entities: vec![endpoint.clone(), handler],
        facts: Vec::new(),
        relations,
        diagnostics: Vec::new(),
    };
    let report = build_change_map(
        &snapshot,
        ChangeMapQuery {
            task: None,
            target: None,
            diff: false,
            changed_files: Vec::new(),
        },
        vec![Seed {
            id: endpoint.id,
            score: 1_000,
            reason: "seed".to_string(),
        }],
        ChangeMapLimits {
            max_entities: 10,
            max_files: 10,
            max_diagnostics: 10,
            max_depth: 2,
        },
    );
    assert_eq!(report.files.len(), 2);
    assert_eq!(report.returned.files, 2);
    assert_eq!(report.omitted.files, 0);
    let openapi_file = report
        .files
        .iter()
        .find(|file| file.path == "openapi.yaml")
        .unwrap();
    assert_eq!(openapi_file.rank, 1);
    assert!(openapi_file.score >= 900);
}

#[test]
fn respects_diagnostics_limit() {
    let (mut snapshot, endpoint) = fixture();
    for index in 0..5 {
        snapshot.diagnostics.push(Diagnostic {
            id: DiagnosticId(format!("diag-{index}")),
            kind: DiagnosticKind::UncoveredSymbol,
            severity: Severity::High,
            status: DiagnosticStatus::Open,
            title: format!("Issue {index}"),
            message: format!("Message {index}"),
            entities: vec![endpoint.id.clone()],
            evidence: entity_evidence(&endpoint),
            ownership: endpoint.ownership.clone(),
            snapshot: SnapshotId("snap-1".to_string()),
            suggested_fix: None,
            payload: json!({}),
        });
    }
    let report = build_change_map(
        &snapshot,
        ChangeMapQuery {
            task: None,
            target: Some(endpoint.stable_key.0.clone()),
            diff: false,
            changed_files: Vec::new(),
        },
        vec![Seed {
            id: endpoint.id,
            score: 1_200,
            reason: "target".to_string(),
        }],
        ChangeMapLimits {
            max_entities: 10,
            max_files: 10,
            max_diagnostics: 2,
            max_depth: 3,
        },
    );
    assert_eq!(report.returned.diagnostics, 2);
    assert_eq!(report.omitted.diagnostics, 3);
}
