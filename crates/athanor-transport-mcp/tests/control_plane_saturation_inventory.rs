const LIFECYCLE: &str = include_str!("../src/server/lifecycle.rs");
const PROTOCOL: &str = include_str!("../src/server/protocol.rs");
const OPERATION: &str = include_str!("../src/server/operation.rs");
const TYPES: &str = include_str!("../src/server/types.rs");
const SERVER_TESTS: &str = include_str!("../src/server/tests.rs");

#[test]
fn stdin_and_notifications_bypass_ordinary_request_saturation() {
    let line_branch = LIFECYCLE
        .find("line = lines.next_line()")
        .expect("stdin branch");
    let join_branch = LIFECYCLE
        .find("joined = requests.join_next()")
        .expect("request reaping branch");
    assert!(LIFECYCLE.contains("tokio::select! {\n                biased;"));
    assert!(
        line_branch < join_branch,
        "stdin must be selected before task reaping"
    );

    let notification = LIFECYCLE
        .find("if request.id.is_notification()")
        .expect("notification dispatch");
    let request_limit = LIFECYCLE
        .find("if requests.len() >= runtime.max_in_flight_requests")
        .expect("ordinary request admission");
    assert!(
        notification < request_limit,
        "control notifications must bypass ordinary request admission"
    );
    assert!(PROTOCOL.contains("notifications/cancelled"));
    assert!(PROTOCOL.contains("$/cancelRequest"));
}

#[test]
fn inline_responses_never_hold_the_only_stdin_reader() {
    assert!(LIFECYCLE.contains("fn admit_inline_response("));
    assert!(LIFECYCLE.contains("responses.try_send(response)"));
    assert!(LIFECYCLE.contains("TrySendError::Full(_)"));
    assert!(LIFECYCLE.contains("TrySendError::Closed(_)"));
    assert!(
        LIFECYCLE.contains("Ordinary request tasks retain bounded `send().await` backpressure")
    );
    assert!(
        LIFECYCLE
            .contains("send_response(&runtime.responses_tx, response_json(id, response)).await")
    );
    assert!(
        !LIFECYCLE
            .contains("try_send(response)\n            .context(\"MCP response queue is saturated")
    );
}

#[test]
fn disconnect_cancels_registered_operations_before_task_drain() {
    assert!(
        LIFECYCLE.contains("None => close_stdin(&runtime.active_reads, &mut stdin_open).await")
    );
    assert!(LIFECYCLE.contains(
        "pub(super) async fn close_stdin(active_reads: &ActiveReads, stdin_open: &mut bool)"
    ));
    assert!(LIFECYCLE.contains("*stdin_open = false;\n    cancel_all(active_reads).await;"));
    assert!(OPERATION.contains("pub(super) async fn cancel_all"));
}

#[test]
fn saturation_and_disconnect_regressions_are_present() {
    for regression in [
        "cancellation_notification_bypasses_saturated_request_limit",
        "full_request_and_response_capacity_does_not_starve_cancellation",
        "inline_protocol_error_does_not_block_saturated_control_plane",
        "stdin_close_cancels_registered_operations_while_requests_are_saturated",
        "saturated_ordinary_request_gets_retryable_server_busy_error",
    ] {
        assert!(
            SERVER_TESTS.contains(regression),
            "missing MCP control-plane regression {regression}"
        );
    }
}

#[test]
fn request_runtime_owns_lifecycle_dependencies_and_lint_fixes() {
    assert!(TYPES.contains("pub(super) struct RequestRuntime"));
    for field in [
        "pub(super) root: Arc<PathBuf>",
        "pub(super) composition: Arc<RuntimeComposition>",
        "pub(super) active_reads: ActiveReads",
        "pub(super) session: SessionState",
        "pub(super) responses_tx: mpsc::Sender<String>",
        "pub(super) max_in_flight_requests: usize",
    ] {
        assert!(
            TYPES.contains(field),
            "missing request runtime field {field}"
        );
    }
    assert!(LIFECYCLE.contains(
        "pub(super) async fn process_line(\n    runtime: &RequestRuntime,\n    requests: &mut RequestTasks,\n    line: String,"
    ));
    assert!(!LIFECYCLE.contains("root: &Arc<PathBuf>"));
    assert!(OPERATION.contains("root: &Path,"));
    assert!(!OPERATION.contains("root: &PathBuf,"));
    assert!(PROTOCOL.contains("root: &Path,"));
    assert!(TYPES.contains("pub(super) error: Box<RpcError>"));
}

#[test]
fn control_plane_owners_remain_bounded() {
    assert!(TYPES.contains("DEFAULT_MAX_IN_FLIGHT_REQUESTS"));
    assert!(TYPES.contains("DEFAULT_RESPONSE_QUEUE_CAPACITY"));
    for (name, source, max_lines) in [
        ("lifecycle", LIFECYCLE, 280),
        ("protocol", PROTOCOL, 260),
        ("operation", OPERATION, 340),
        ("server tests", SERVER_TESTS, 460),
    ] {
        let lines = source.lines().count();
        assert!(lines <= max_lines, "{name} grew to {lines} lines");
    }
}
