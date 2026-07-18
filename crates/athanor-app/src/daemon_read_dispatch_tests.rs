use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use athanor_core::{CanonicalSnapshot, CoreResult, SearchDocument, SearchQuery, SearchResult};
use athanor_domain::SnapshotId;
use tokio::sync::Notify;

use crate::daemon::{
    CachedSearchIndex, DAEMON_ENDPOINT_SCHEMA, DAEMON_PROTOCOL_VERSION, DAEMON_REQUEST_SCHEMA,
    DaemonCommand, DaemonEndpoint, DaemonErrorCode, DaemonJobKind, DaemonJobStatus,
    DaemonLifecycleState, DaemonRequest, DaemonState, DaemonTransport,
};
use crate::daemon_jobs_support::unix_time_ms;
use crate::daemon_runtime::BoundedCache;

struct BlockingIndex {
    started: Arc<Notify>,
    release: Arc<Notify>,
}

#[async_trait]
impl athanor_core::SearchIndex for BlockingIndex {
    async fn index_document(&self, _doc: SearchDocument) -> CoreResult<()> {
        Ok(())
    }

    async fn remove_document(&self, _id: &str) -> CoreResult<()> {
        Ok(())
    }

    async fn search(&self, _query: SearchQuery) -> CoreResult<Vec<SearchResult>> {
        self.started.notify_one();
        self.release.notified().await;
        Ok(Vec::new())
    }
}

#[tokio::test]
async fn cancel_request_terminates_running_read_job() {
    let started = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let state = test_state(Arc::new(BlockingIndex {
        started: Arc::clone(&started),
        release: Arc::clone(&release),
    }));
    let task = tokio::spawn(crate::daemon_read_dispatch::execute(
        Arc::clone(&state),
        search_request("cancel-read", None),
    ));
    started.notified().await;

    let job_id = state
        .jobs
        .lock()
        .unwrap()
        .iter()
        .find(|job| job.kind == DaemonJobKind::Search)
        .expect("running search job")
        .id
        .clone();
    let cancelled = crate::daemon_job_cancellation::cancel(&state, &job_id).unwrap();
    assert_eq!(cancelled.status, DaemonJobStatus::Cancelling);
    release.notify_one();

    let (response, shutdown) = task.await.unwrap();
    assert!(!shutdown);
    assert!(!response.ok);
    assert_eq!(
        response.error_details.as_ref().map(|details| details.code),
        Some(DaemonErrorCode::Cancelled)
    );
    assert_eq!(
        state
            .jobs
            .lock()
            .unwrap()
            .iter()
            .find(|job| job.id == job_id)
            .map(|job| job.status.clone()),
        Some(DaemonJobStatus::Cancelled)
    );
}

#[tokio::test]
async fn hard_deadline_returns_stable_error_and_fails_job() {
    let started = Arc::new(Notify::new());
    let state = test_state(Arc::new(BlockingIndex {
        started: Arc::clone(&started),
        release: Arc::new(Notify::new()),
    }));
    let deadline = (unix_time_ms().unwrap() as u64).saturating_add(250);

    let (response, shutdown) = crate::daemon_read_dispatch::execute(
        Arc::clone(&state),
        search_request("deadline-read", Some(deadline)),
    )
    .await;

    assert!(!shutdown);
    assert!(!response.ok);
    assert_eq!(
        response.error_details.as_ref().map(|details| details.code),
        Some(DaemonErrorCode::DeadlineExceeded)
    );
    assert_eq!(
        state
            .jobs
            .lock()
            .unwrap()
            .iter()
            .find(|job| job.kind == DaemonJobKind::Search)
            .map(|job| job.status.clone()),
        Some(DaemonJobStatus::Failed)
    );
    tokio::time::timeout(Duration::from_millis(10), started.notified())
        .await
        .expect("search reached backend before deadline");
}

fn search_request(request_id: &str, deadline_unix_ms: Option<u64>) -> DaemonRequest {
    DaemonRequest {
        schema: DAEMON_REQUEST_SCHEMA.to_string(),
        request_id: request_id.to_string(),
        project_id: "project".to_string(),
        auth_token: Some("token".to_string()),
        command: DaemonCommand::Search {
            query: "entity".to_string(),
            limit: 10,
            deadline_unix_ms,
        },
    }
}

fn test_state(index: Arc<dyn athanor_core::SearchIndex>) -> Arc<DaemonState> {
    let root = PathBuf::from(".");
    let snapshot_id = "snap_test".to_string();
    Arc::new(DaemonState {
        composition: crate::test_runtime::composition(),
        endpoint: DaemonEndpoint {
            schema: DAEMON_ENDPOINT_SCHEMA.to_string(),
            protocol_version: DAEMON_PROTOCOL_VERSION,
            athanor_version: "test".to_string(),
            runtime_id: "runtime-test".to_string(),
            token_path: PathBuf::from("token"),
            project_id: "project".to_string(),
            root,
            registry_path: PathBuf::from("registry"),
            address: "127.0.0.1:1".parse().unwrap(),
            transport: DaemonTransport::Tcp,
            local_socket_path: None,
            windows_pipe_name: None,
            pid: 1,
            started_at_unix_ms: 0,
            max_concurrent_requests: 8,
            max_job_history: 32,
            watch: false,
            watch_poll: false,
            debounce_ms: 250,
            max_request_bytes: 1024 * 1024,
            max_response_bytes: 1024 * 1024,
        },
        auth_token: "token".to_string(),
        insecure_allow_v1: false,
        lifecycle: Mutex::new(DaemonLifecycleState::Running),
        last_successful_index: Mutex::new(None),
        jobs: Mutex::new(Vec::new()),
        next_job_sequence: Mutex::new(1),
        max_job_history: 32,
        latest_snapshot_cache: Mutex::new(Some(CanonicalSnapshot {
            snapshot: Some(SnapshotId(snapshot_id.clone())),
            ..CanonicalSnapshot::default()
        })),
        search_index_cache: Mutex::new(Some(CachedSearchIndex { snapshot_id, index })),
        overview_cache: Mutex::new(BoundedCache::new(8)),
        context_cache: Mutex::new(BoundedCache::new(8)),
        cancellation_tokens: Mutex::new(HashMap::new()),
    })
}
