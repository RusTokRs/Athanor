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
