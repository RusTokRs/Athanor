use std::future::Future;
use std::sync::Arc;

use anyhow::Result;
use athanor_core::{CoreError, OperationContext, OperationContextCancellation};
use serde::Serialize;
use serde_json::Value;

use crate::daemon::{
    DaemonCommand, DaemonJobKind, DaemonJobStatus, DaemonLifecycleState, DaemonRequest,
    DaemonResponse, DaemonState,
};
use crate::daemon_job_scheduler::start_cancellable_with_operation;
use crate::daemon_job_state::{begin_or_finish_failed, finish, finish_cancellable_error};
use crate::daemon_lifecycle::current as lifecycle;
use crate::daemon_operation::context as operation_context;
use crate::daemon_protocol::{error_response_from_anyhow, success_response, validate_request};
use crate::derived_read_operation::{
    change_map_project_with_composition_and_operation_context,
    change_map_project_with_operation_context,
    context_project_with_composition_and_operation_context,
    context_project_with_operation_context,
};

/// Intercepts derived read commands that still execute through compatibility application services.
/// Every other request is delegated to the established operation-aware read dispatcher.
pub(crate) async fn execute(
    state: Arc<DaemonState>,
    request: DaemonRequest,
) -> (DaemonResponse, bool) {
    if !is_derived_read(&request.command)
        || validate_request(&state, &request).is_err()
        || request.project_id != state.endpoint.project_id
        || lifecycle(&state) != DaemonLifecycleState::Running
    {
        return crate::daemon_read_dispatch::execute(state, request).await;
    }

    let request_id = request.request_id;
    match request.command {
        DaemonCommand::Context {
            task,
            diff: true,
            level,
            limits,
            deadline_unix_ms,
        } => {
            let operation = operation_context("context", &request_id, deadline_unix_ms);
            let job_id = match start_read_job(
                &state,
                DaemonJobKind::Context,
                format!("context task={} diff=true", task.trim()),
                &operation,
            ) {
                Ok(job_id) => job_id,
                Err(error) => return failed_start(&state, &request_id, &error),
            };
            let options = crate::ContextOptions {
                root: state.endpoint.root.clone(),
                task,
                diff: true,
                level,
                limits,
            };
            let result = match crate::daemon_queries::composition(&state) {
                Some(composition) => {
                    context_project_with_composition_and_operation_context(
                        options,
                        &composition,
                        &operation,
                    )
                    .await
                }
                None => context_project_with_operation_context(options, &operation).await,
            };
            finish_read(&state, &request_id, &job_id, result)
        }
        DaemonCommand::ChangeMap {
            task,
            target,
            diff,
            max_entities,
            max_files,
            max_diagnostics,
            max_depth,
            deadline_unix_ms,
        } => {
            let operation = operation_context("change_map", &request_id, deadline_unix_ms);
            let job_id = match start_read_job(
                &state,
                DaemonJobKind::ChangeMap,
                format!("change-map task={task:?} target={target:?} diff={diff}"),
                &operation,
            ) {
                Ok(job_id) => job_id,
                Err(error) => return failed_start(&state, &request_id, &error),
            };
            let options = crate::ChangeMapOptions {
                root: state.endpoint.root.clone(),
                task,
                target,
                diff,
                max_entities,
                max_files,
                max_diagnostics,
                max_depth,
            };
            let result = match crate::daemon_queries::composition(&state) {
                Some(composition) => {
                    within_operation_deadline(
                        &operation,
                        change_map_project_with_composition_and_operation_context(
                            options,
                            &composition,
                            &operation,
                        ),
                    )
                    .await
                }
                None => {
                    within_operation_deadline(
                        &operation,
                        change_map_project_with_operation_context(options, &operation),
                    )
                    .await
                }
            };
            finish_read(&state, &request_id, &job_id, result)
        }
        _ => unreachable!("derived read predicate and dispatch match diverged"),
    }
}

fn is_derived_read(command: &DaemonCommand) -> bool {
    matches!(
        command,
        DaemonCommand::Context { diff: true, .. } | DaemonCommand::ChangeMap { .. }
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
        anyhow::bail!("daemon derived read job `{job_id}` could not start");
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
    let identity = operation
        .operation_id
        .as_deref()
        .unwrap_or("daemon derived read");
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
    use athanor_domain::ContextLevel;

    use super::*;

    #[test]
    fn intercepts_only_diff_context_and_change_map() {
        assert!(is_derived_read(&DaemonCommand::Context {
            task: String::new(),
            diff: true,
            level: ContextLevel::Normal,
            limits: crate::ContextLimitOverrides::default(),
            deadline_unix_ms: None,
        }));
        assert!(!is_derived_read(&DaemonCommand::Context {
            task: "task".to_string(),
            diff: false,
            level: ContextLevel::Normal,
            limits: crate::ContextLimitOverrides::default(),
            deadline_unix_ms: None,
        }));
        assert!(is_derived_read(&DaemonCommand::ChangeMap {
            task: Some("task".to_string()),
            target: None,
            diff: false,
            max_entities: 10,
            max_files: 10,
            max_diagnostics: 10,
            max_depth: 1,
            deadline_unix_ms: None,
        }));
    }
}
