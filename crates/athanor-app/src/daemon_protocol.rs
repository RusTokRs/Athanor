use anyhow::{Error, Result, bail};
use athanor_core::{CoreError, CoreErrorCode};
use serde_json::Value;

use crate::constant_time_token_eq;
use crate::daemon::{
    DAEMON_REQUEST_SCHEMA, DAEMON_REQUEST_SCHEMA_V1, DAEMON_REQUEST_SCHEMA_V2,
    DAEMON_REQUEST_SCHEMA_V3, DAEMON_RESPONSE_SCHEMA, DaemonCommand, DaemonError, DaemonErrorCode,
    DaemonRequest, DaemonResponse, DaemonState, DaemonTransport, MAX_PROTOCOL_BYTES,
    MIN_PROTOCOL_BYTES,
};
use crate::daemon_jobs_support::unix_time_ms;

pub(super) fn success_response(
    request_id: &str,
    project_id: &str,
    result: Value,
) -> DaemonResponse {
    DaemonResponse {
        schema: DAEMON_RESPONSE_SCHEMA.to_string(),
        request_id: request_id.to_string(),
        project_id: project_id.to_string(),
        ok: true,
        result: Some(result),
        error: None,
        error_details: None,
    }
}

pub(super) fn error_response(request_id: &str, project_id: &str, error: &str) -> DaemonResponse {
    let details = error_details(error);
    error_response_with_code(
        request_id,
        project_id,
        details.code,
        details.retryable,
        error,
    )
}

pub(super) fn error_response_with_code(
    request_id: &str,
    project_id: &str,
    code: DaemonErrorCode,
    retryable: bool,
    error: &str,
) -> DaemonResponse {
    DaemonResponse {
        schema: DAEMON_RESPONSE_SCHEMA.to_string(),
        request_id: request_id.to_string(),
        project_id: project_id.to_string(),
        ok: false,
        result: None,
        error: Some(error.to_string()),
        error_details: Some(DaemonError {
            code,
            message: error.to_string(),
            retryable,
            details: serde_json::Map::new(),
        }),
    }
}

pub(super) fn error_response_from_anyhow(
    request_id: &str,
    project_id: &str,
    error: &Error,
) -> DaemonResponse {
    if let Some(core_error) = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<CoreError>())
    {
        let code = match core_error.code() {
            CoreErrorCode::NotFound => DaemonErrorCode::NotFound,
            CoreErrorCode::InvalidInput => DaemonErrorCode::InvalidInput,
            CoreErrorCode::AdapterProtocol => DaemonErrorCode::AdapterProtocol,
            CoreErrorCode::AdapterExecution => DaemonErrorCode::AdapterExecution,
            CoreErrorCode::SnapshotNotCommitted => DaemonErrorCode::SnapshotNotCommitted,
            CoreErrorCode::Conflict => DaemonErrorCode::Conflict,
            CoreErrorCode::Busy => DaemonErrorCode::Busy,
            CoreErrorCode::Cancelled => DaemonErrorCode::Cancelled,
            CoreErrorCode::DeadlineExceeded => DaemonErrorCode::DeadlineExceeded,
        };
        return error_response_with_code(
            request_id,
            project_id,
            code,
            core_error.is_retryable(),
            &error.to_string(),
        );
    }
    error_response(request_id, project_id, &error.to_string())
}

fn error_details(message: &str) -> DaemonError {
    let normalized = message.to_ascii_lowercase();
    let (code, retryable) = if normalized.contains("authentication") {
        (DaemonErrorCode::Unauthorized, false)
    } else if normalized.contains("not found") {
        (DaemonErrorCode::NotFound, false)
    } else if normalized.contains("already queued") || normalized.contains("busy") {
        (DaemonErrorCode::Busy, true)
    } else if normalized.contains("cancel") {
        (DaemonErrorCode::Cancelled, false)
    } else if normalized.contains("timed out") || normalized.contains("deadline") {
        (DaemonErrorCode::DeadlineExceeded, true)
    } else if normalized.contains("must ") || normalized.contains("invalid") {
        (DaemonErrorCode::InvalidInput, false)
    } else {
        (DaemonErrorCode::Internal, false)
    };
    DaemonError {
        code,
        message: message.to_string(),
        retryable,
        details: serde_json::Map::new(),
    }
}

pub(super) fn serialize_response(
    response: DaemonResponse,
    max_response_bytes: u64,
) -> Result<Vec<u8>> {
    let response_json = serde_json::to_vec(&response)?;
    if response_json.len() as u64 <= max_response_bytes {
        return Ok(response_json);
    }

    let overflow = error_response(
        &response.request_id,
        &response.project_id,
        &format!(
            "daemon response exceeds size limit of {} bytes",
            max_response_bytes
        ),
    );
    let overflow_json = serde_json::to_vec(&overflow)?;
    if overflow_json.len() as u64 > max_response_bytes {
        bail!("daemon overflow error response exceeds response size limit");
    }
    Ok(overflow_json)
}

pub(super) fn validate_limit(name: &str, value: u64) -> Result<()> {
    if !(MIN_PROTOCOL_BYTES..=MAX_PROTOCOL_BYTES).contains(&value) {
        bail!("{name} must be between {MIN_PROTOCOL_BYTES} and {MAX_PROTOCOL_BYTES}");
    }
    Ok(())
}

pub(crate) fn validate_request_shape(request: &DaemonRequest) -> Result<()> {
    if request.schema != DAEMON_REQUEST_SCHEMA
        && request.schema != DAEMON_REQUEST_SCHEMA_V2
        && request.schema != DAEMON_REQUEST_SCHEMA_V3
        && request.schema != DAEMON_REQUEST_SCHEMA_V1
    {
        bail!("unsupported daemon request schema `{}`", request.schema);
    }
    if request.request_id.is_empty() || request.request_id.len() > 128 {
        bail!("daemon request_id must contain 1-128 characters");
    }
    if request.project_id.is_empty() {
        bail!("daemon project_id must not be empty");
    }
    let deadline = match &request.command {
        DaemonCommand::Index { deadline_unix_ms }
        | DaemonCommand::Generate { deadline_unix_ms }
        | DaemonCommand::Wiki { deadline_unix_ms }
        | DaemonCommand::HtmlReport { deadline_unix_ms }
        | DaemonCommand::Overview {
            deadline_unix_ms, ..
        }
        | DaemonCommand::Explain {
            deadline_unix_ms, ..
        }
        | DaemonCommand::Search {
            deadline_unix_ms, ..
        }
        | DaemonCommand::Context {
            deadline_unix_ms, ..
        }
        | DaemonCommand::ChangeMap {
            deadline_unix_ms, ..
        } => *deadline_unix_ms,
        _ => None,
    };
    if let Some(deadline) = deadline {
        if deadline <= unix_time_ms()? as u64 {
            bail!("daemon command deadline_unix_ms must be in the future");
        }
    }
    Ok(())
}

pub(crate) fn validate_request(state: &DaemonState, request: &DaemonRequest) -> Result<()> {
    validate_request_shape(request)?;
    if request.schema == DAEMON_REQUEST_SCHEMA_V1 {
        if !state.insecure_allow_v1 {
            bail!("daemon protocol v1 is disabled");
        }
        if state.endpoint.transport != DaemonTransport::Tcp
            || !state.endpoint.address.ip().is_loopback()
        {
            bail!("daemon protocol v1 is allowed only over loopback TCP");
        }
        return Ok(());
    }
    let supplied = request
        .auth_token
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("daemon authentication failed"))?;
    if !constant_time_token_eq(supplied, &state.auth_token) {
        bail!("daemon authentication failed");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::DaemonErrorCode;

    #[test]
    fn error_response_preserves_legacy_and_structured_error_contracts() {
        let response = error_response("request-1", "project-1", "daemon authentication failed");

        assert_eq!(
            response.error.as_deref(),
            Some("daemon authentication failed")
        );
        let details = response.error_details.expect("structured error details");
        assert_eq!(details.code, DaemonErrorCode::Unauthorized);
        assert!(!details.retryable);
        assert!(details.details.is_empty());
    }

    #[test]
    fn deadline_error_is_retryable_and_serializes_with_stable_code() {
        let response = error_response("request-1", "project-1", "request timed out");
        let json = serde_json::to_value(response).expect("serialize response");

        assert_eq!(json["error_details"]["code"], "deadline_exceeded");
        assert_eq!(json["error_details"]["retryable"], true);
    }

    #[test]
    fn core_error_chain_preserves_stable_code_and_retryability() {
        let error = anyhow::Error::new(athanor_core::CoreError::NotFound(
            "entity stable key".to_string(),
        ));
        let response = error_response_from_anyhow("request-1", "project-1", &error);
        let details = response.error_details.expect("structured error details");

        assert_eq!(details.code, DaemonErrorCode::NotFound);
        assert!(!details.retryable);
    }

    #[test]
    fn adapter_protocol_core_error_maps_to_public_protocol_code() {
        let error = anyhow::Error::new(athanor_core::CoreError::AdapterProtocol(
            "invalid JSON response".to_string(),
        ));
        let response = error_response_from_anyhow("request-1", "project-1", &error);
        let details = response.error_details.expect("structured error details");

        assert_eq!(details.code, DaemonErrorCode::AdapterProtocol);
        assert!(!details.retryable);
    }

    #[test]
    fn every_core_error_category_maps_to_a_stable_daemon_error() {
        let cases = [
            (
                CoreError::NotFound("entity".to_string()),
                DaemonErrorCode::NotFound,
                false,
            ),
            (
                CoreError::InvalidInput("query".to_string()),
                DaemonErrorCode::InvalidInput,
                false,
            ),
            (
                CoreError::AdapterProtocol("JSON".to_string()),
                DaemonErrorCode::AdapterProtocol,
                false,
            ),
            (
                CoreError::Adapter("exit status".to_string()),
                DaemonErrorCode::AdapterExecution,
                false,
            ),
            (
                CoreError::SnapshotNotCommitted("snap".to_string()),
                DaemonErrorCode::SnapshotNotCommitted,
                false,
            ),
            (
                CoreError::Conflict("stable key".to_string()),
                DaemonErrorCode::Conflict,
                false,
            ),
            (
                CoreError::Busy("index".to_string()),
                DaemonErrorCode::Busy,
                true,
            ),
            (
                CoreError::Cancelled("request".to_string()),
                DaemonErrorCode::Cancelled,
                false,
            ),
            (
                CoreError::DeadlineExceeded("request".to_string()),
                DaemonErrorCode::DeadlineExceeded,
                true,
            ),
        ];

        for (core_error, expected_code, expected_retryable) in cases {
            let error = Error::new(core_error);
            let response = error_response_from_anyhow("request-1", "project-1", &error);
            let details = response.error_details.expect("structured error details");

            assert_eq!(details.code, expected_code);
            assert_eq!(details.retryable, expected_retryable);
        }
    }
}
