//! Per-request latest-committed selection must not trust process-local snapshot state.

use std::fs;

use athanor_core::{CoreError, KnowledgeStore, OperationContext, OperationContextCancellation};
use athanor_domain::{ContextLevel, GenerationId, RepoId, SnapshotBase, SnapshotId};
use serde_json::{Value, json};

use crate::daemon::{
    DAEMON_REQUEST_SCHEMA, DaemonCommand, DaemonErrorCode, DaemonRequest, DaemonResponse,
};
use crate::daemon_query_test_support::DaemonFixture;

#[tokio::test]
async fn warmed_daemon_reads_follow_external_commits_without_restart() {
    let fixture = DaemonFixture::new("external").await;
    assert_eq!(
        read(&fixture, overview()).await["snapshot"],
        fixture.initial_snapshot
    );
    let first_search = read(&fixture, search()).await;
    let old_key = symbol_key(&first_search, "freshness_alpha");
    assert_eq!(
        read(&fixture, context()).await["payload"]["snapshot"],
        fixture.initial_snapshot
    );
    assert_eq!(
        read(&fixture, explain(&old_key)).await["snapshot"],
        fixture.initial_snapshot
    );

    let updated = fixture.publish("freshness_beta").await;
    assert_ne!(updated, fixture.initial_snapshot);
    assert_eq!(read(&fixture, overview()).await["snapshot"], updated);
    let search_report = read(&fixture, search()).await;
    assert_eq!(search_report["snapshot"], updated);
    let new_key = symbol_key(&search_report, "freshness_beta");
    assert!(
        !search_report["results"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["name"] == "freshness_alpha")
    );
    assert_eq!(
        read(&fixture, context()).await["payload"]["snapshot"],
        updated
    );
    assert_eq!(read(&fixture, explain(&new_key)).await["snapshot"], updated);
    let removed = try_read(&fixture, explain(&old_key)).await;
    assert!(!removed.ok);
    assert_eq!(
        removed.error_details.unwrap().code,
        DaemonErrorCode::NotFound
    );
    assert_eq!(
        fixture
            .state
            .search_index_cache
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .snapshot_id,
        updated
    );
    assert!(
        fixture
            .state
            .last_successful_index
            .lock()
            .unwrap()
            .is_none()
    );
    let status = read(&fixture, DaemonCommand::Status).await;
    assert_eq!(status["cache"]["last_observed_snapshot"], updated);
    assert_eq!(status["cache"]["snapshot_loaded"], true);
}

#[tokio::test]
async fn unchanged_snapshot_reuses_bounded_derived_reports() {
    let fixture = DaemonFixture::new("unchanged").await;
    let initial_status = read(&fixture, DaemonCommand::Status).await;
    assert_eq!(
        initial_status["cache"]["last_observed_snapshot"],
        Value::Null
    );
    assert_eq!(initial_status["cache"]["snapshot_loaded"], false);
    let overview_before = read(&fixture, overview()).await;
    let context_before = read(&fixture, context()).await;
    assert_eq!(read(&fixture, overview()).await, overview_before);
    assert_eq!(read(&fixture, context()).await, context_before);
    assert_eq!(fixture.state.overview_cache.lock().unwrap().len(), 1);
    assert_eq!(fixture.state.context_cache.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn prepared_external_snapshot_is_hidden_until_commit() {
    let fixture = DaemonFixture::new("prepared").await;
    read(&fixture, overview()).await;
    let store = store(&fixture).await;
    let pending = store
        .begin_snapshot(RepoId("fixture".into()), base(&fixture))
        .await
        .unwrap();
    store.prepare_snapshot(pending.clone()).await.unwrap();
    assert_eq!(
        read(&fixture, overview()).await["snapshot"],
        fixture.initial_snapshot
    );
    store.commit_snapshot(pending.clone()).await.unwrap();
    let updated = read(&fixture, overview()).await;
    assert_eq!(updated["snapshot"], pending.0);
    assert_eq!(updated["totals"]["entities"], 0);
}

#[tokio::test]
async fn unavailable_latest_never_falls_back_to_a_warmed_snapshot_or_repairs_pointer() {
    for scenario in [
        "missing",
        "malformed",
        "missing-target",
        "uncommitted-target",
    ] {
        let fixture = DaemonFixture::new(scenario).await;
        read(&fixture, overview()).await;
        let pointer = fixture.latest_pointer();
        match scenario {
            "missing" => fs::remove_file(&pointer).unwrap(),
            "malformed" => fs::write(&pointer, b"{broken").unwrap(),
            _ => {
                let target = if scenario == "uncommitted-target" {
                    store(&fixture)
                        .await
                        .begin_snapshot(RepoId("fixture".into()), base(&fixture))
                        .await
                        .unwrap()
                } else {
                    SnapshotId("snap_missing".into())
                };
                fs::write(
                    &pointer,
                    serde_json::to_vec(&json!({
                        "schema": "athanor.canonical_latest.v1",
                        "snapshot": target,
                        "generation": GenerationId::for_snapshot(&target),
                    }))
                    .unwrap(),
                )
                .unwrap();
            }
        }
        let before = fs::read(&pointer).ok();
        for command in [overview(), search(), context()] {
            let response = try_read(&fixture, command).await;
            assert!(!response.ok, "{scenario}: stale cache was served");
            assert!(response.result.is_none());
            assert!(response.error.is_some());
        }
        assert_eq!(
            fs::read(&pointer).ok(),
            before,
            "{scenario}: a read repaired latest"
        );
        assert_eq!(
            read(&fixture, DaemonCommand::Status).await["cache"]["last_observed_snapshot"],
            fixture.initial_snapshot
        );
    }
}

#[tokio::test]
async fn warmed_read_state_does_not_bypass_cancellation_or_deadline() {
    let fixture = DaemonFixture::new("operation").await;
    read(&fixture, overview()).await;
    let operation = OperationContext::new("daemon-freshness-cancelled");
    let cancellation = operation.cancellation_handle().unwrap();
    cancellation.cancel();
    let cancelled =
        crate::daemon_queries::latest_snapshot_with_operation_context(&fixture.state, &operation)
            .await
            .unwrap_err();
    assert!(cancelled.chain().any(|cause| matches!(
        cause.downcast_ref::<CoreError>(),
        Some(CoreError::Cancelled(_))
    )));
    let expired = OperationContext::new("daemon-freshness-expired").with_deadline_unix_ms(0);
    let error =
        crate::daemon_queries::latest_snapshot_with_operation_context(&fixture.state, &expired)
            .await
            .unwrap_err();
    assert!(error.chain().any(|cause| matches!(
        cause.downcast_ref::<CoreError>(),
        Some(CoreError::DeadlineExceeded(_))
    )));
}

#[tokio::test]
async fn legacy_latest_pointer_remains_readable_after_cache_refresh() {
    let fixture = DaemonFixture::new("legacy").await;
    read(&fixture, overview()).await;
    let updated = fixture.publish("freshness_beta").await;
    fs::write(
        fixture.latest_pointer(),
        serde_json::to_vec(&json!({"snapshot": updated})).unwrap(),
    )
    .unwrap();
    assert_eq!(read(&fixture, overview()).await["snapshot"], updated);
}

async fn store(fixture: &DaemonFixture) -> crate::AthanorStore {
    fixture
        .state
        .composition
        .init_store(
            &fixture.state.endpoint.root,
            &crate::ProjectConfig::default(),
        )
        .await
        .unwrap()
}

fn base(fixture: &DaemonFixture) -> SnapshotBase {
    SnapshotBase {
        branch: None,
        commit: None,
        parent_snapshot: Some(SnapshotId(fixture.initial_snapshot.clone())),
        working_tree: true,
    }
}

async fn try_read(fixture: &DaemonFixture, command: DaemonCommand) -> DaemonResponse {
    let (response, shutdown) = crate::daemon_read_dispatch::execute(
        fixture.state.clone(),
        DaemonRequest {
            schema: DAEMON_REQUEST_SCHEMA.to_string(),
            request_id: fixture.request_id(),
            project_id: fixture.state.endpoint.project_id.clone(),
            auth_token: Some(fixture.state.auth_token.clone()),
            command,
        },
    )
    .await;
    assert!(!shutdown);
    response
}

async fn read(fixture: &DaemonFixture, command: DaemonCommand) -> Value {
    let response = try_read(fixture, command).await;
    assert!(response.ok, "{response:?}");
    response.result.unwrap()
}

fn symbol_key(report: &Value, name: &str) -> String {
    report["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["name"] == name)
        .unwrap_or_else(|| panic!("missing {name}: {report}"))
        .get("stable_key")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string()
}

fn overview() -> DaemonCommand {
    DaemonCommand::Overview {
        top: 3,
        deadline_unix_ms: None,
    }
}

fn search() -> DaemonCommand {
    DaemonCommand::Search {
        query: "freshness".into(),
        limit: 10,
        deadline_unix_ms: None,
    }
}

fn context() -> DaemonCommand {
    DaemonCommand::Context {
        task: "freshness".into(),
        diff: false,
        level: ContextLevel::Normal,
        limits: crate::ContextLimitOverrides {
            max_tokens: Some(1000),
            max_files: Some(3),
            max_entities: Some(4),
            ..Default::default()
        },
        deadline_unix_ms: None,
    }
}

fn explain(stable_key: &str) -> DaemonCommand {
    DaemonCommand::Explain {
        stable_key: stable_key.into(),
        deadline_unix_ms: None,
    }
}
