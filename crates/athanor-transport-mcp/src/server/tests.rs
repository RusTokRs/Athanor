use std::future::pending;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::Result;
use athanor_core::{CoreError, OperationContext, OperationContextCancellation};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{Mutex, mpsc};

use super::lifecycle::{
    close_stdin, log_request_task, process_line, run_mcp_server_io,
};
use super::operation::{
    cancel_notification, request_key, run_registered_drained_read, run_registered_read,
};
use super::types::{
    ActiveReads, DEFAULT_MAX_IN_FLIGHT_REQUESTS, DEFAULT_RESPONSE_QUEUE_CAPACITY, McpServerLimits,
    McpSessionPhase, RequestTasks, SessionState,
};

#[tokio::test]
async fn protocol_session_negotiates_before_tools_are_available() {
    let responses = exchange(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}"#,
        r#"{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
    ])
    .await;
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0]["id"], 1);
    assert_eq!(responses[1]["id"], 2);
    assert!(responses[1]["result"]["tools"].is_array());
}

#[tokio::test]
async fn protocol_errors_use_standard_json_rpc_codes() {
    assert_eq!(exchange(&["not-json"]).await[0]["error"]["code"], -32700);
    assert_eq!(
        exchange(&[
            r#"{"jsonrpc":"1.0","id":7,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}"#,
        ])
        .await[0]["error"]["code"],
        -32600
    );
    assert_eq!(
        exchange(&[r#"{"jsonrpc":"2.0","id":8,"method":"tools/list"}"#])
            .await[0]["error"]["code"],
        -32002
    );
}

#[tokio::test]
async fn omitted_id_is_notification_but_explicit_null_receives_response() {
    let notification = exchange(&[
        r#"{"jsonrpc":"2.0","method":"unknown/notification"}"#,
    ])
    .await;
    assert!(notification.is_empty());
    let explicit_null = exchange(&[
        r#"{"jsonrpc":"2.0","id":null,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}"#,
    ])
    .await;
    assert_eq!(explicit_null.len(), 1);
    assert!(explicit_null[0]["id"].is_null());
}

#[tokio::test]
async fn bounded_response_queue_applies_backpressure() {
    let (sender, mut receiver) = mpsc::channel(1);
    sender.send("first".to_string()).await.unwrap();
    let second = sender.send("second".to_string());
    tokio::pin!(second);
    assert!(
        tokio::time::timeout(Duration::from_millis(10), &mut second)
            .await
            .is_err()
    );
    assert_eq!(receiver.recv().await.as_deref(), Some("first"));
    tokio::time::timeout(Duration::from_secs(1), &mut second)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn completed_request_tasks_are_reaped() {
    let mut requests = RequestTasks::new();
    requests.spawn(async { Ok(()) });
    let joined = requests.join_next().await.expect("completed request task");
    log_request_task(joined);
    assert!(requests.is_empty());
}

#[tokio::test]
async fn cancellation_notification_bypasses_saturated_request_limit() {
    let active_reads = ActiveReads::default();
    let operation = OperationContext::new("mcp:\"saturated\"");
    let cancellation = operation.cancellation_handle().unwrap();
    active_reads
        .lock()
        .await
        .insert("\"saturated\"".to_string(), cancellation);

    let mut requests = RequestTasks::new();
    requests.spawn(pending::<Result<()>>());
    let (responses_tx, _responses_rx) = mpsc::channel(1);
    let session: SessionState = Arc::new(Mutex::new(McpSessionPhase::Ready));
    let root = Arc::new(PathBuf::from("."));
    let composition = Arc::new(athanor_runtime_defaults::production());

    process_line(
        &root,
        &composition,
        &active_reads,
        &session,
        &responses_tx,
        &mut requests,
        1,
        r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":"saturated"}}"#.to_string(),
    )
    .await
    .unwrap();

    assert!(operation.is_cancelled());
    requests.abort_all();
}

#[tokio::test]
async fn full_request_and_response_capacity_does_not_starve_cancellation() {
    let active_reads = ActiveReads::default();
    let operation = OperationContext::new("mcp:\"fully-saturated\"");
    let cancellation = operation.cancellation_handle().unwrap();
    active_reads
        .lock()
        .await
        .insert("\"fully-saturated\"".to_string(), cancellation);

    let mut requests = RequestTasks::new();
    requests.spawn(pending::<Result<()>>());
    let (responses_tx, mut responses_rx) = mpsc::channel(1);
    responses_tx.send("occupied".to_string()).await.unwrap();
    let session: SessionState = Arc::new(Mutex::new(McpSessionPhase::Ready));
    let root = Arc::new(PathBuf::from("."));
    let composition = Arc::new(athanor_runtime_defaults::production());

    process_line(
        &root,
        &composition,
        &active_reads,
        &session,
        &responses_tx,
        &mut requests,
        1,
        r#"{"jsonrpc":"2.0","id":9,"method":"tools/list"}"#.to_string(),
    )
    .await
    .expect("overload rejection must not terminate the reader loop");

    process_line(
        &root,
        &composition,
        &active_reads,
        &session,
        &responses_tx,
        &mut requests,
        1,
        r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":"fully-saturated"}}"#.to_string(),
    )
    .await
    .expect("cancellation must bypass saturated response capacity");

    assert!(operation.is_cancelled());
    assert_eq!(responses_rx.recv().await.as_deref(), Some("occupied"));
    assert!(responses_rx.try_recv().is_err());
    requests.abort_all();
}

#[tokio::test]
async fn inline_protocol_error_does_not_block_saturated_control_plane() {
    let active_reads = ActiveReads::default();
    let operation = OperationContext::new("mcp:\"protocol-saturated\"");
    let cancellation = operation.cancellation_handle().unwrap();
    active_reads
        .lock()
        .await
        .insert("\"protocol-saturated\"".to_string(), cancellation);

    let mut requests = RequestTasks::new();
    requests.spawn(pending::<Result<()>>());
    let (responses_tx, _responses_rx) = mpsc::channel(1);
    responses_tx.send("occupied".to_string()).await.unwrap();
    let session: SessionState = Arc::new(Mutex::new(McpSessionPhase::Ready));
    let root = Arc::new(PathBuf::from("."));
    let composition = Arc::new(athanor_runtime_defaults::production());

    process_line(
        &root,
        &composition,
        &active_reads,
        &session,
        &responses_tx,
        &mut requests,
        1,
        "not-json".to_string(),
    )
    .await
    .expect("inline protocol response must fail open under queue saturation");

    process_line(
        &root,
        &composition,
        &active_reads,
        &session,
        &responses_tx,
        &mut requests,
        1,
        r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":"protocol-saturated"}}"#.to_string(),
    )
    .await
    .unwrap();

    assert!(operation.is_cancelled());
    requests.abort_all();
}

#[tokio::test]
async fn stdin_close_cancels_registered_operations_while_requests_are_saturated() {
    let active_reads = ActiveReads::default();
    let operation = OperationContext::new("mcp:\"disconnect-saturated\"");
    let cancellation = operation.cancellation_handle().unwrap();
    active_reads
        .lock()
        .await
        .insert("\"disconnect-saturated\"".to_string(), cancellation);
    let mut requests = RequestTasks::new();
    requests.spawn(pending::<Result<()>>());
    let mut stdin_open = true;

    close_stdin(&active_reads, &mut stdin_open).await;

    assert!(!stdin_open);
    assert_eq!(requests.len(), 1);
    assert!(operation.is_cancelled());
    requests.abort_all();
}

#[tokio::test]
async fn saturated_ordinary_request_gets_retryable_server_busy_error() {
    let active_reads = ActiveReads::default();
    let mut requests = RequestTasks::new();
    requests.spawn(pending::<Result<()>>());
    let (responses_tx, mut responses_rx) = mpsc::channel(1);
    let session: SessionState = Arc::new(Mutex::new(McpSessionPhase::Ready));
    let root = Arc::new(PathBuf::from("."));
    let composition = Arc::new(athanor_runtime_defaults::production());

    process_line(
        &root,
        &composition,
        &active_reads,
        &session,
        &responses_tx,
        &mut requests,
        1,
        r#"{"jsonrpc":"2.0","id":9,"method":"tools/list"}"#.to_string(),
    )
    .await
    .unwrap();

    let response: Value = serde_json::from_str(&responses_rx.recv().await.unwrap()).unwrap();
    assert_eq!(response["error"]["code"], -32001);
    assert_eq!(response["error"]["data"]["retryable"], true);
    assert_eq!(response["error"]["data"]["limit"], 1);
    requests.abort_all();
}

#[tokio::test]
async fn cancellation_notification_terminates_registered_read() {
    let active_reads = ActiveReads::default();
    let operation = OperationContext::new("mcp:\"request-1\"");
    let task_active = Arc::clone(&active_reads);
    let task = tokio::spawn(async move {
        run_registered_read(
            &task_active,
            "\"request-1\"".to_string(),
            operation,
            pending::<Result<()>>(),
        )
        .await
    });
    wait_until_registered(&active_reads, "\"request-1\"").await;
    cancel_notification(
        &active_reads,
        Some(&json!({ "requestId": "request-1" })),
    )
    .await;
    let error = task.await.unwrap().expect_err("cancelled read must fail");
    assert!(error.chain().any(|cause| matches!(
        cause.downcast_ref::<CoreError>(),
        Some(CoreError::Cancelled(_))
    )));
    assert!(active_reads.lock().await.is_empty());
}

#[tokio::test]
async fn drained_read_waits_for_operation_future_cleanup() {
    let active_reads = ActiveReads::default();
    let operation = OperationContext::new("mcp:\"drained\"");
    let execution_operation = operation.clone();
    let completed = Arc::new(AtomicBool::new(false));
    let completed_in_future = Arc::clone(&completed);
    let task_active = Arc::clone(&active_reads);
    let task = tokio::spawn(async move {
        run_registered_drained_read(
            &task_active,
            "\"drained\"".to_string(),
            operation,
            async move {
                while !execution_operation.is_cancelled() {
                    tokio::task::yield_now().await;
                }
                completed_in_future.store(true, Ordering::Release);
                execution_operation
                    .check_active()
                    .map_err(anyhow::Error::new)?;
                Ok::<_, anyhow::Error>(())
            },
        )
        .await
    });
    wait_until_registered(&active_reads, "\"drained\"").await;
    cancel_notification(&active_reads, Some(&json!({ "requestId": "drained" }))).await;
    let error = task.await.unwrap().expect_err("cancelled read must fail");
    assert!(completed.load(Ordering::Acquire));
    assert!(error.chain().any(|cause| matches!(
        cause.downcast_ref::<CoreError>(),
        Some(CoreError::Cancelled(_))
    )));
}

#[test]
fn default_server_limits_are_bounded() {
    let limits = McpServerLimits::default().validate().unwrap();
    assert_eq!(limits.max_in_flight_requests, DEFAULT_MAX_IN_FLIGHT_REQUESTS);
    assert_eq!(limits.response_queue_capacity, DEFAULT_RESPONSE_QUEUE_CAPACITY);
}

#[test]
fn request_ids_distinguish_omitted_from_explicit_null() {
    assert_eq!(request_key(&json!(7)).unwrap(), "7");
    assert_eq!(request_key(&json!("seven")).unwrap(), "\"seven\"");
    assert_eq!(request_key(&Value::Null).unwrap(), "null");
    assert!(request_key(&json!(true)).is_err());
}

async fn wait_until_registered(active_reads: &ActiveReads, key: &str) {
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if active_reads.lock().await.contains_key(key) {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("read registered");
}

async fn exchange(lines: &[&str]) -> Vec<Value> {
    let (mut input_client, input_server) = tokio::io::duplex(16 * 1024);
    let (output_server, mut output_client) = tokio::io::duplex(64 * 1024);
    let composition = Arc::new(athanor_runtime_defaults::production());
    let server = tokio::spawn(run_mcp_server_io(
        PathBuf::from("."),
        composition,
        BufReader::new(input_server),
        output_server,
        McpServerLimits {
            max_in_flight_requests: 4,
            response_queue_capacity: 4,
        },
    ));

    for line in lines {
        input_client.write_all(line.as_bytes()).await.unwrap();
        input_client.write_all(b"\n").await.unwrap();
    }
    input_client.shutdown().await.unwrap();

    let mut output = String::new();
    output_client.read_to_string(&mut output).await.unwrap();
    server.await.unwrap().unwrap();
    output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
