use std::future::Future;
use std::sync::Arc;

use anyhow::Result;
use athanor_core::{CoreError, OperationContext, OperationContextCancellation};
use serde::Serialize;
use serde_json::Value;

use crate::daemon::{
    DaemonCommand, DaemonErrorCode, DaemonJobKind, DaemonJobStatus, DaemonLifecycleState,
    DaemonRequest, DaemonResponse, DaemonState,
};
use crate::daemon_job_scheduler::start_cancellable_with_operation;
use crate::daemon_job_state::{begin_or_finish_failed, finish, finish_cancellable_error};
use crate::daemon_lifecycle::current as lifecycle;
use crate::daemon_operation::context as operation_context;
use crate::daemon_protocol::{
    error_response_from_anyhow, error_response_with_code, success_response, validate_request,
};

/// Routes read-only commands through the operation-context path and delegates every other command to the
/// established daemon dispatcher unchanged.
pub(crate) async fn execute(
    state: Arc<DaemonState>,
    request: DaemonRequest,
) -> (DaemonResponse, bool) {
    if !is_context_aware_read(&request.command)
        || validate_request(&state, &request).is_err()
        || request.project_id != state.endpoint.project_id
        || lifecycle(&state) != DaemonLifecycleState::Running
    {
        return crate::daemon::execute_request(state, request).await;
    }

    let request_id = request.request_id;
    match request.command {
        DaemonCommand::Overview {
            top,
            deadline_unix_ms,
        } => {
            if top == 0 || top > 100 {
                return invalid_input(&state, &request_id, "overview top must be between 1 and 100");
            }
            let operation = operation_context("overview", &request_id, deadline_unix_ms);
            let job_id = match start_read_job(
                &state,
                DaemonJobKind::Overview,
                format!("overview top={top}"),
                &operation,
            ) {
                Ok(job_id) => job_id,
                Err(error) => return failed_start(&state, &request_id, &error),
            };
            let result = within_operation_deadline(
                &operation,
                crate::daemon_queries::overview_with_operation_context(&state, top, &operation),
            )
            .await;
            finish_read(&state, &request_id, &job_id, result)
        }
        DaemonCommand::Explain {
            stable_key,
            deadline_unix_ms,
        } => {
            if stable_key.trim().is_empty() {
                return invalid_input(&state, &request_id, "entity stable key must not be empty");
            }
            let stable_key = stable_key.trim().to_string();
            let operation = operation_context("explain", &request_id, deadline_unix_ms);
            let job_id = match start_read_job(
                &state,
                DaemonJobKind::Explain,
                format!("explain stable_key={stable_key}"),
                &operation,
            ) {
                Ok(job_id) => job_id,
                Err(error) => return failed_start(&state, &request_id, &error),
            };
            let result = within_operation_deadline(
                &operation,
                crate::daemon_queries::explain_with_operation_context(
                    &state,
                    &stable_key,
                    &operation,
                ),
            )
            .await;
            finish_read(&state, &request_id, &job_id, result)
        }
        DaemonCommand::Search {
            query,
            limit,
            deadline_unix_ms,
        } => {
            if query.trim().is_empty() {
                return invalid_input(&state, &request_id, "search query must not be empty");
            }
            if limit == 0 || limit > 100 {
                return invalid_input(&state, &request_id, "search limit must be between 1 and 100");
            }
            let query = query.trim().to_string();
            let operation = operation_context("search", &request_id, deadline_unix_ms);
            let job_id = match start_read_job(
                &state,
                DaemonJobKind::Search,
                format!("search query={query} limit={limit}"),
                &operation,
            ) {
                Ok(job_id) => job_id,
                Err(error) => return failed_start(&state, &request_id, &error),
            };
            let result = within_operation_deadline(
                &operation,
                crate::daemon_queries::search_with_operation_context(
                    &state,
                    query,
                    limit,
                    &operation,
                ),
            )
            .await;
            finish_read(&state, &request_id, &job_id, result)
        }
        DaemonCommand::Context {
            task,
            diff: false,
            level,
            limits,
            deadline_unix_ms,
        } => {
            if task.trim().is_empty() {
                return invalid_input(&state, &request_id, "context task must not be empty");
            }
            let operation = operation_context("context", &request_id, deadline_unix_ms);
            let job_id = match start_read_job(
                &state,
                DaemonJobKind::Context,
                format!("context task={} diff=false", task.trim()),
                &operation,
            ) {
                Ok(job_id) => job_id,
                Err(error) => return failed_start(&state, &request_id, &error),
            };
            let result = within_operation_deadline(
                &operation,
                crate::daemon_queries::context_with_operation_context(
                    &state,
                    &task,
                    level,
                    &limits,
                    &operation,
                ),
            )
            .await;
            finish_read(&state, &request_id, &job_id, result)
        }
        _ => unreachable!("context-aware read predicate and dispatch match diverged"),
    }
}

fn is_context_aware_read(command: &DaemonCommand) -> bool {
    matches!(
        command,
        DaemonCommand::Overview { .. }
            | DaemonCommand::Explain { .. }
            | DaemonCommand::Search { .. }
            | DaemonCommand::Context { diff: false, .. }
    )
}

fn start_read_job(
    state: &DaemonState,
    kind: DaemonJobKind,
    description: String,
    operation: &OperationContext,
) -> Result<String> {
    operation.check_active().map_err(anyhow::Error::new)?;
    let (job_id, _cancellation) =
        start_cancellable_with_operation(state, kind, description, operation)?;
    if !begin_or_finish_failed(state, &job_id) {
        anyhow::bail!("daemon read job `{job_id}` could not start");
    }
    Ok(job_id)
}

async fn within_operation_deadline<T>(
    operation: &OperationContext,
    future: impl Future<Output = Result<T>>,
) -> Result<T> {
    operation.check_active().map_err(anyhow::Error::new)?;
    let Some(remaining) = operation.remaining() else {
        return future.await;
    };
    if remaining.is_zero() {
        return Err(deadline_error(operation));
    }
    match tokio::time::timeout(remaining, future).await {
        Ok(result) => result,
        Err(_) => Err(deadline_error(operation)),
    }
}

fn deadline_error(operation: &OperationContext) -> anyhow::Error {
    let identity = operation.operation_id.as_deref().unwrap_or("daemon read");
    anyhow::Error::new(CoreError::DeadlineExceeded(format!(
        "{identity} exceeded its configured deadline"
    )))
}

fn finish_read<T: Serialize>(
    state: &DaemonState,
    request_id: &str,
    job_id: &str,
    result: Result<T>,
) -> (DaemonResponse, bool) {
    match result {
        Ok(result) => {
            let _ = finish(
                state,
                job_id,
                DaemonJobStatus::Succeeded,
                None,
                None,
            );
            (
                success_response(
                    request_id,
                    &state.endpoint.project_id,
                    serde_json::to_value(result).unwrap_or(Value::Null),
                ),
                false,
            )
        }
        Err(error) => {
            let response =
                error_response_from_anyhow(request_id, &state.endpoint.project_id, &error);
            let _ = finish_cancellable_error(state, job_id, error);
            (response, false)
        }
    }
}

fn invalid_input(
    state: &DaemonState,
    request_id: &str,
    message: &str,
) -> (DaemonResponse, bool) {
    (
        error_response_with_code(
            request_id,
            &state.endpoint.project_id,
            DaemonErrorCode::InvalidInput,
            false,
            message,
        ),
        false,
    )
}

fn failed_start(
    state: &DaemonState,
    request_id: &str,
    error: &anyhow::Error,
) -> (DaemonResponse, bool) {
    (
        error_response_from_anyhow(request_id, &state.endpoint.project_id, error),
        false,
    )
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::time::Duration;

    use async_trait::async_trait;
    use athanor_core::{
        CanonicalSnapshot, CoreResult, SearchDocument, SearchQuery, SearchResult,
    };
    use athanor_domain::SnapshotId;
    use tokio::sync::Notify;

    use super::*;
    use crate::daemon::{
        CachedSearchIndex, DAEMON_ENDPOINT_SCHEMA, DAEMON_PROTOCOL_VERSION, DAEMON_REQUEST_SCHEMA,
        DaemonEndpoint, DaemonTransport,
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
        let task = tokio::spawn(execute(
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

        let (response, shutdown) = execute(
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
            composition: None,
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
}
