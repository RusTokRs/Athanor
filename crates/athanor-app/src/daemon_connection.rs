use std::io::ErrorKind;
use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};

use crate::daemon::{
    DAEMON_REQUEST_SCHEMA_V2, DAEMON_RESPONSE_SCHEMA_V2, DAEMON_RESPONSE_SCHEMA_V3,
    DaemonErrorCode, DaemonRequest, DaemonState, execute_request,
};
use crate::daemon_protocol::{error_response_with_code, serialize_response, validate_request};

/// Handles one authenticated daemon connection, including bounded request and response I/O.
pub(super) async fn handle<S>(mut stream: S, state: Arc<DaemonState>) -> Result<bool>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut line = String::new();
    let bytes = BufReader::new(&mut stream)
        .take(state.endpoint.max_request_bytes + 1)
        .read_line(&mut line)
        .await
        .context("failed to read daemon request")?;
    let (response, shutdown) = if bytes == 0 {
        (
            error_response_with_code(
                "",
                &state.endpoint.project_id,
                DaemonErrorCode::InvalidInput,
                false,
                "empty daemon request",
            ),
            false,
        )
    } else if bytes as u64 > state.endpoint.max_request_bytes {
        (
            error_response_with_code(
                "",
                &state.endpoint.project_id,
                DaemonErrorCode::InvalidInput,
                false,
                "daemon request exceeds size limit",
            ),
            false,
        )
    } else {
        match serde_json::from_str::<DaemonRequest>(&line) {
            Ok(request) => {
                let response_schema = if request.schema == DAEMON_REQUEST_SCHEMA_V2 {
                    DAEMON_RESPONSE_SCHEMA_V2
                } else {
                    DAEMON_RESPONSE_SCHEMA_V3
                };
                let (mut response, shutdown) = execute_request(Arc::clone(&state), request).await;
                response.schema = response_schema.to_string();
                (response, shutdown)
            }
            Err(error) => (
                error_response_with_code(
                    "",
                    &state.endpoint.project_id,
                    DaemonErrorCode::InvalidInput,
                    false,
                    &format!("invalid daemon request JSON: {error}"),
                ),
                false,
            ),
        }
    };
    let response_json = serialize_response(response, state.endpoint.max_response_bytes)?;
    if let Err(error) = stream.write_all(&response_json).await {
        if is_client_disconnect(&error) {
            return Ok(false);
        }
        return Err(error).context("failed to write daemon response");
    }
    if let Err(error) = stream.shutdown().await {
        if is_client_disconnect(&error) {
            return Ok(false);
        }
        return Err(error.into());
    }
    Ok(shutdown)
}

/// Rejects one excess connection without revealing daemon capacity before authentication succeeds.
pub(super) async fn handle_busy<S>(mut stream: S, state: &DaemonState) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut line = String::new();
    let _ = BufReader::new(&mut stream)
        .take(state.endpoint.max_request_bytes + 1)
        .read_line(&mut line)
        .await;
    let parsed = serde_json::from_str::<DaemonRequest>(&line);
    let request_id = parsed
        .as_ref()
        .map(|request| request.request_id.clone())
        .unwrap_or_default();
    let (code, retryable, message) = match parsed.as_ref() {
        Ok(request) if validate_request(state, request).is_ok() => (
            DaemonErrorCode::Busy,
            true,
            "daemon is busy; maximum concurrent request limit reached",
        ),
        _ => (
            DaemonErrorCode::Unauthorized,
            false,
            "daemon authentication failed",
        ),
    };
    let response = error_response_with_code(
        &request_id,
        &state.endpoint.project_id,
        code,
        retryable,
        message,
    );
    stream
        .write_all(&serialize_response(
            response,
            state.endpoint.max_response_bytes,
        )?)
        .await
        .context("failed to write daemon busy response")?;
    stream.shutdown().await?;
    Ok(())
}

fn is_client_disconnect(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        ErrorKind::BrokenPipe
            | ErrorKind::ConnectionAborted
            | ErrorKind::ConnectionReset
            | ErrorKind::NotConnected
    )
}
