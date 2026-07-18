use std::sync::Arc;

use serde_json::Value;

use crate::daemon::{
    DAEMON_PROTOCOL_VERSION, DAEMON_REQUEST_SCHEMA_V1, DaemonCommand, DaemonErrorCode,
    DaemonJobKind, DaemonLifecycleState, DaemonRequest, DaemonResponse, DaemonState,
};
use crate::daemon_job_cancellation::cancel as cancel_daemon_job;
use crate::daemon_job_registry::{get as get_daemon_job, list as list_daemon_jobs};
use crate::daemon_job_state::has_active;
use crate::daemon_jobs_support::{is_valid_job_id, unix_time_ms};
use crate::daemon_lifecycle::{active_job_count, current as lifecycle};
use crate::daemon_operation::context as daemon_operation_context;
use crate::daemon_protocol::{
    error_response_from_anyhow, error_response_with_code, success_response, validate_request,
    validate_request_shape,
};

/// Handles control-plane and write commands after the derived/read dispatchers decline them.
pub(crate) async fn execute(
    state: Arc<DaemonState>,
    request: DaemonRequest,
) -> (DaemonResponse, bool) {
    if let Err(error) = validate_request(&state, &request) {
        let code = if validate_request_shape(&request).is_err() {
            DaemonErrorCode::InvalidInput
        } else if request.schema == DAEMON_REQUEST_SCHEMA_V1 {
            DaemonErrorCode::Forbidden
        } else {
            DaemonErrorCode::Unauthorized
        };
        return (
            error_response_with_code(
                &request.request_id,
                &state.endpoint.project_id,
                code,
                false,
                &error.to_string(),
            ),
            false,
        );
    }
    if request.project_id != state.endpoint.project_id {
        return (
            error_response_with_code(
                &request.request_id,
                &state.endpoint.project_id,
                DaemonErrorCode::InvalidInput,
                false,
                &format!(
                    "request project `{}` does not match daemon project `{}`",
                    request.project_id, state.endpoint.project_id
                ),
            ),
            false,
        );
    }
    if lifecycle(&state) != DaemonLifecycleState::Running
        && !matches!(
            &request.command,
            DaemonCommand::Status | DaemonCommand::Jobs { .. } | DaemonCommand::Job { .. }
        )
    {
        return (
            error_response_with_code(
                &request.request_id,
                &state.endpoint.project_id,
                DaemonErrorCode::Busy,
                true,
                "daemon is stopping and does not accept new work",
            ),
            false,
        );
    }

    match request.command {
        DaemonCommand::Status => (
            success_response(
                &request.request_id,
                &state.endpoint.project_id,
                serde_json::json!({
                    "status": lifecycle(&state),
                    "protocol_version": DAEMON_PROTOCOL_VERSION,
                    "athanor_version": env!("CARGO_PKG_VERSION"),
                    "uptime_ms": unix_time_ms()
                        .unwrap_or_default()
                        .saturating_sub(state.endpoint.started_at_unix_ms),
                    "active_jobs": active_job_count(&state).unwrap_or_default(),
                    "cache": crate::daemon_queries::cache_status(&state),
                    "last_successful_index": state.last_successful_index
                        .lock()
                        .ok()
                        .and_then(|snapshot| snapshot.clone()),
                    "endpoint": &state.endpoint,
                }),
            ),
            false,
        ),
        DaemonCommand::Jobs { limit } => {
            if limit == 0 || limit > 100 {
                return invalid_input(
                    &state,
                    &request.request_id,
                    "jobs limit must be between 1 and 100",
                );
            }
            match list_daemon_jobs(&state, limit) {
                Ok(report) => (
                    success_response(
                        &request.request_id,
                        &state.endpoint.project_id,
                        serde_json::to_value(report).unwrap_or(Value::Null),
                    ),
                    false,
                ),
                Err(error) => failed(&state, &request.request_id, &error),
            }
        }
        DaemonCommand::Job { job_id } => match get_daemon_job(&state, &job_id) {
            Ok(job) => (
                success_response(
                    &request.request_id,
                    &state.endpoint.project_id,
                    serde_json::to_value(job).unwrap_or(Value::Null),
                ),
                false,
            ),
            Err(error) => {
                let code = if is_valid_job_id(&job_id) {
                    DaemonErrorCode::NotFound
                } else {
                    DaemonErrorCode::InvalidInput
                };
                (
                    error_response_with_code(
                        &request.request_id,
                        &state.endpoint.project_id,
                        code,
                        false,
                        &error.to_string(),
                    ),
                    false,
                )
            }
        },
        DaemonCommand::Cancel { job_id } => match cancel_daemon_job(&state, &job_id) {
            Ok(job) => (
                success_response(
                    &request.request_id,
                    &state.endpoint.project_id,
                    serde_json::to_value(job).unwrap_or(Value::Null),
                ),
                false,
            ),
            Err(error) => {
                let code = if !is_valid_job_id(&job_id) {
                    DaemonErrorCode::InvalidInput
                } else if error.to_string().contains("was not found") {
                    DaemonErrorCode::NotFound
                } else {
                    DaemonErrorCode::Conflict
                };
                (
                    error_response_with_code(
                        &request.request_id,
                        &state.endpoint.project_id,
                        code,
                        false,
                        &error.to_string(),
                    ),
                    false,
                )
            }
        },
        DaemonCommand::Index { deadline_unix_ms } => {
            let operation =
                daemon_operation_context("index", &request.request_id, deadline_unix_ms);
            match crate::daemon_write_jobs::start_index(
                &state,
                "index project".to_string(),
                operation,
            ) {
                Ok(job) => success_job(&state, &request.request_id, job),
                Err(error) => (
                    error_response_with_code(
                        &request.request_id,
                        &state.endpoint.project_id,
                        DaemonErrorCode::Busy,
                        true,
                        &error.to_string(),
                    ),
                    false,
                ),
            }
        }
        DaemonCommand::Generate { deadline_unix_ms } => {
            if has_active(&state, DaemonJobKind::Generate).unwrap_or(false) {
                return busy(
                    &state,
                    &request.request_id,
                    "generate job is already queued or running",
                );
            }
            match crate::daemon_write_jobs::start_generate(
                &state,
                &request.request_id,
                deadline_unix_ms,
            ) {
                Ok(job) => success_optional_job(&state, &request.request_id, job),
                Err(error) => failed(&state, &request.request_id, &error),
            }
        }
        DaemonCommand::Wiki { deadline_unix_ms } => {
            if has_active(&state, DaemonJobKind::Wiki).unwrap_or(false) {
                return busy(
                    &state,
                    &request.request_id,
                    "wiki job is already queued or running",
                );
            }
            match crate::daemon_write_jobs::start_wiki(
                &state,
                &request.request_id,
                deadline_unix_ms,
            ) {
                Ok(job) => success_optional_job(&state, &request.request_id, job),
                Err(error) => failed(&state, &request.request_id, &error),
            }
        }
        DaemonCommand::HtmlReport { deadline_unix_ms } => {
            if has_active(&state, DaemonJobKind::HtmlReport).unwrap_or(false) {
                return busy(
                    &state,
                    &request.request_id,
                    "HTML report job is already queued or running",
                );
            }
            match crate::daemon_write_jobs::start_html_report(
                &state,
                &request.request_id,
                deadline_unix_ms,
            ) {
                Ok(job) => success_optional_job(&state, &request.request_id, job),
                Err(error) => failed(&state, &request.request_id, &error),
            }
        }
        DaemonCommand::Shutdown => (
            success_response(
                &request.request_id,
                &state.endpoint.project_id,
                serde_json::json!({"status": "stopping"}),
            ),
            true,
        ),
        DaemonCommand::Overview { .. }
        | DaemonCommand::Explain { .. }
        | DaemonCommand::Search { .. }
        | DaemonCommand::Context { .. }
        | DaemonCommand::ChangeMap { .. } => (
            error_response_with_code(
                &request.request_id,
                &state.endpoint.project_id,
                DaemonErrorCode::Internal,
                false,
                "daemon command reached the wrong dispatcher",
            ),
            false,
        ),
    }
}

fn success_job(
    state: &DaemonState,
    request_id: &str,
    job: crate::daemon::DaemonJob,
) -> (DaemonResponse, bool) {
    (
        success_response(
            request_id,
            &state.endpoint.project_id,
            serde_json::to_value(job).unwrap_or(Value::Null),
        ),
        false,
    )
}

fn success_optional_job(
    state: &DaemonState,
    request_id: &str,
    job: Option<crate::daemon::DaemonJob>,
) -> (DaemonResponse, bool) {
    (
        success_response(
            request_id,
            &state.endpoint.project_id,
            serde_json::to_value(job).unwrap_or(Value::Null),
        ),
        false,
    )
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

fn busy(state: &DaemonState, request_id: &str, message: &str) -> (DaemonResponse, bool) {
    (
        error_response_with_code(
            request_id,
            &state.endpoint.project_id,
            DaemonErrorCode::Busy,
            true,
            message,
        ),
        false,
    )
}

fn failed(
    state: &DaemonState,
    request_id: &str,
    error: &anyhow::Error,
) -> (DaemonResponse, bool) {
    (
        error_response_from_anyhow(request_id, &state.endpoint.project_id, error),
        false,
    )
}
