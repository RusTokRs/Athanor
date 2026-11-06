use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use athanor_core::{CoreResult, SearchDocument, SearchQuery, SearchResult};
use tokio::sync::Notify;

use crate::daemon::{
    DAEMON_REQUEST_SCHEMA, DaemonCommand, DaemonErrorCode, DaemonJobKind, DaemonJobStatus,
    DaemonRequest,
};
use crate::daemon_jobs_support::unix_time_ms;
use crate::daemon_query_test_support::DaemonFixture;

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
    let fixture = DaemonFixture::new("cancel").await;
    fixture.set_search_index(Arc::new(BlockingIndex {
        started: Arc::clone(&started),
        release: Arc::clone(&release),
    }));
    let state = Arc::clone(&fixture.state);
    let task = tokio::spawn(crate::daemon_read_dispatch::execute(
        Arc::clone(&state),
        search_request("cancel-read", None),
    ));
    tokio::time::timeout(Duration::from_secs(10), started.notified())
        .await
        .expect("search reached backend before cancellation");

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
    let fixture = DaemonFixture::new("deadline").await;
    fixture.set_search_index(Arc::new(BlockingIndex {
        started: Arc::clone(&started),
        release: Arc::new(Notify::new()),
    }));
    let state = Arc::clone(&fixture.state);
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
