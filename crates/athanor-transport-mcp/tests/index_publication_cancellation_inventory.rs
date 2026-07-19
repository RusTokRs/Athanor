const MCP_DISPATCH_SOURCE: &str = include_str!("../src/tools/dispatch.rs");
const MCP_OPERATION_SOURCE: &str = include_str!("../src/server/operation.rs");
const STORE_PUBLICATION_SOURCE: &str =
    include_str!("../../athanor-app/src/store/publication.rs");
const CORE_PUBLICATION_SOURCE: &str =
    include_str!("../../athanor-core/src/atomic_publication.rs");
const APP_CANCELLATION_REGRESSIONS: &str =
    include_str!("../../athanor-app/src/store_publication_cancellation_tests.rs");
const APP_PRECOMMIT_REGRESSION: &str =
    include_str!("../../athanor-app/src/index_publication_fault_tests.rs");

#[test]
fn mcp_index_uses_operation_aware_durable_success_path() {
    assert!(MCP_DISPATCH_SOURCE.contains(
        "index_project_with_composition_and_operation_context"
    ));
    assert!(!MCP_DISPATCH_SOURCE.contains(
        "athanor_app::index_project_with_composition("
    ));
    assert!(MCP_DISPATCH_SOURCE.contains(
        "return Ok(serde_json::to_string_pretty(&report)?);"
    ));

    assert!(MCP_OPERATION_SOURCE.contains("is_durable_commit_tool(tool_name)"));
    assert!(MCP_OPERATION_SOURCE.contains(
        "run_registered_durable_operation(active_reads, request_key, operation, future)"
    ));
    assert!(MCP_OPERATION_SOURCE.contains("matches!(tool_name, \"index\")"));
    assert!(MCP_OPERATION_SOURCE.contains(
        "does not poll or postflight-check the operation after the future returns"
    ));
    assert!(MCP_OPERATION_SOURCE.contains(
        "durable_operation_preserves_success_after_registered_cancellation"
    ));
}

#[test]
fn atomic_publication_reconciles_only_terminal_operation_errors() {
    assert!(STORE_PUBLICATION_SOURCE.contains(
        "CoreError::Cancelled(_) | CoreError::DeadlineExceeded(_)"
    ));
    assert!(STORE_PUBLICATION_SOURCE.contains("load_snapshot(snapshot).await"));
    assert!(STORE_PUBLICATION_SOURCE.contains(
        "canonical.snapshot.as_ref() == Some(snapshot)"
    ));
    assert!(CORE_PUBLICATION_SOURCE.contains(
        "They are not checked again after a successful publish"
    ));
}

#[test]
fn cancellation_matrix_covers_pre_commit_commit_race_and_post_commit() {
    for regression in [
        "pre_commit_cancellation_rolls_back_and_remains_an_error",
        "committed_terminal_errors_are_reconciled_to_publication_success",
        "cancellation_after_commit_does_not_override_publication_success",
    ] {
        assert!(APP_CANCELLATION_REGRESSIONS.contains(regression));
    }
    assert!(APP_CANCELLATION_REGRESSIONS.contains("assert_publication_journals_cleared"));
    assert!(APP_CANCELLATION_REGRESSIONS.contains("IndexCurrent::load"));
    assert!(APP_PRECOMMIT_REGRESSION.contains(
        "cancelled_canonical_publish_restores_previous_artifacts_and_aborts_snapshot"
    ));
}

#[test]
fn transactional_cancellation_owners_remain_bounded() {
    for (name, source, max_lines) in [
        ("MCP dispatch", MCP_DISPATCH_SOURCE, 320),
        ("MCP operation", MCP_OPERATION_SOURCE, 340),
        ("Store publication", STORE_PUBLICATION_SOURCE, 120),
        (
            "Store publication cancellation regressions",
            APP_CANCELLATION_REGRESSIONS,
            430,
        ),
    ] {
        let lines = source.lines().count();
        assert!(lines <= max_lines, "{name} grew to {lines} lines");
    }
}
