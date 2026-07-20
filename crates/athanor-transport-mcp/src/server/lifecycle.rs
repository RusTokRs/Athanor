use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use athanor_app::RuntimeComposition;
use tokio::io::{
    self, AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader,
};
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::{Mutex, mpsc};
use tokio::task::JoinError;

use super::operation::cancel_all;
use super::protocol::{
    handle_initialize, handle_notification, handle_request, parse_request, response_json,
};
use super::types::{
    ActiveReads, DispatchError, JsonRpcRequest, McpServerLimits, McpSessionPhase, RequestTasks,
    RpcError, SessionState,
};

/// Runs the MCP stdio server with explicit runtime dependencies.
pub async fn run_mcp_server_with_composition(
    root: PathBuf,
    composition: RuntimeComposition,
) -> Result<()> {
    run_mcp_server_io(
        root,
        Arc::new(composition),
        BufReader::new(io::stdin()),
        io::stdout(),
        McpServerLimits::default(),
    )
    .await
}

pub(super) async fn run_mcp_server_io<R, W>(
    root: PathBuf,
    composition: Arc<RuntimeComposition>,
    reader: R,
    writer: W,
    limits: McpServerLimits,
) -> Result<()>
where
    R: AsyncBufRead + Unpin,
    W: AsyncWrite + Send + Unpin + 'static,
{
    let limits = limits.validate()?;
    let root = Arc::new(root);
    let active_reads = ActiveReads::default();
    let session = Arc::new(Mutex::new(McpSessionPhase::AwaitingInitialize));
    let (responses_tx, responses_rx) = mpsc::channel::<String>(limits.response_queue_capacity);
    let writer_task = tokio::spawn(write_responses(writer, responses_rx));
    let mut requests = RequestTasks::new();
    let mut lines = reader.lines();
    let mut stdin_open = true;

    eprintln!("Athanor MCP server starting in {}", root.display());

    while stdin_open || !requests.is_empty() {
        if stdin_open {
            tokio::select! {
                biased;
                line = lines.next_line() => {
                    match line? {
                        Some(line) => {
                            process_line(
                                &root,
                                &composition,
                                &active_reads,
                                &session,
                                &responses_tx,
                                &mut requests,
                                limits.max_in_flight_requests,
                                line,
                            )
                            .await?;
                        }
                        None => close_stdin(&active_reads, &mut stdin_open).await,
                    }
                }
                joined = requests.join_next(), if !requests.is_empty() => {
                    log_request_task(joined.expect("guarded non-empty request set"));
                }
            }
        } else if let Some(joined) = requests.join_next().await {
            log_request_task(joined);
        } else {
            break;
        }
    }

    cancel_all(&active_reads).await;
    drop(responses_tx);
    writer_task
        .await
        .context("MCP response writer task terminated unexpectedly")?
        .context("failed to write MCP response stream")?;
    Ok(())
}

pub(super) async fn process_line(
    root: &Arc<PathBuf>,
    composition: &Arc<RuntimeComposition>,
    active_reads: &ActiveReads,
    session: &SessionState,
    responses_tx: &mpsc::Sender<String>,
    requests: &mut RequestTasks,
    max_in_flight_requests: usize,
    line: String,
) -> Result<()> {
    if line.trim().is_empty() {
        return Ok(());
    }

    let request = match parse_request(&line) {
        Ok(request) => request,
        Err(failure) => {
            admit_inline_response(responses_tx, failure.response(), "protocol error");
            return Ok(());
        }
    };

    // Notifications are control-plane input. They bypass ordinary request and response capacity.
    if request.id.is_notification() {
        handle_notification(active_reads, session, request).await;
        return Ok(());
    }

    // Initialize is session control and does not consume an ordinary request slot. Its immediate
    // response is admitted without awaiting a saturated writer queue so later cancellation and EOF
    // remain observable.
    if request.method == "initialize" {
        let id = request.id.response_value();
        let result = handle_initialize(request.params, session).await;
        admit_inline_response(responses_tx, response_json(id, result), "initialize");
        return Ok(());
    }

    if requests.len() >= max_in_flight_requests {
        let id = request
            .id
            .into_value()
            .expect("ordinary requests contain explicit ids");
        let response = response_json(
            id,
            Err(DispatchError::Protocol(RpcError::server_busy(
                max_in_flight_requests,
            ))),
        );
        admit_inline_response(responses_tx, response, "overload rejection");
        return Ok(());
    }

    requests.spawn(run_request_task(
        Arc::clone(root),
        Arc::clone(composition),
        Arc::clone(active_reads),
        Arc::clone(session),
        responses_tx.clone(),
        request,
    ));
    Ok(())
}

async fn run_request_task(
    root: Arc<PathBuf>,
    composition: Arc<RuntimeComposition>,
    active_reads: ActiveReads,
    session: SessionState,
    responses_tx: mpsc::Sender<String>,
    request: JsonRpcRequest,
) -> Result<()> {
    let id = request
        .id
        .into_value()
        .expect("request tasks only contain explicit ids");
    let response = handle_request(
        &root,
        &composition,
        request.method,
        request.params,
        &id,
        &active_reads,
        &session,
    )
    .await;
    send_response(&responses_tx, response_json(id, response)).await
}

pub(super) async fn close_stdin(active_reads: &ActiveReads, stdin_open: &mut bool) {
    *stdin_open = false;
    cancel_all(active_reads).await;
}

/// Admits responses produced directly by the reader/control loop without awaiting queue capacity.
///
/// Ordinary request tasks retain bounded `send().await` backpressure. Inline responses must not hold
/// the only stdin reader while the queue is full, because that would starve cancellation and EOF.
fn admit_inline_response(
    responses: &mpsc::Sender<String>,
    response: String,
    response_kind: &str,
) {
    match responses.try_send(response) {
        Ok(()) => {}
        Err(TrySendError::Full(_)) => eprintln!(
            "MCP {response_kind} response dropped because the bounded response queue is saturated"
        ),
        Err(TrySendError::Closed(_)) => eprintln!(
            "MCP {response_kind} response dropped because the response writer is unavailable"
        ),
    }
}

async fn write_responses<W>(
    mut writer: W,
    mut responses: mpsc::Receiver<String>,
) -> io::Result<()>
where
    W: AsyncWrite + Unpin,
{
    while let Some(response) = responses.recv().await {
        writer.write_all(response.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;
    }
    writer.flush().await
}

async fn send_response(responses: &mpsc::Sender<String>, response: String) -> Result<()> {
    responses
        .send(response)
        .await
        .context("MCP response writer is unavailable")
}

pub(super) fn log_request_task(joined: std::result::Result<Result<()>, JoinError>) {
    match joined {
        Ok(Ok(())) => {}
        Ok(Err(error)) => eprintln!("MCP request response failed: {error:#}"),
        Err(error) => eprintln!("MCP request task terminated unexpectedly: {error}"),
    }
}
