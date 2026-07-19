const MCP_DISPATCH_SOURCE: &str = include_str!("../src/tools/dispatch.rs");
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
    assert!(MCP_DISPATCH_SOURCE.contains(
        "a transport-level cancellation that races after commit must not replace that success"
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
        "pre_commit_cancellation_remains_an_error",
        "committed_terminal_errors_are_reconciled_to_success",
        "cancellation_after_commit_does_not_override_success",
    ] {
        assert!(APP_CANCELLATION_REGRESSIONS.contains(regression));
    }
    assert!(APP_PRECOMMIT_REGRESSION.contains(
        "cancelled_canonical_publish_restores_previous_artifacts_and_aborts_snapshot"
    ));
}

#[test]
fn transactional_cancellation_owners_remain_bounded() {
    for (name, source, max_lines) in [
        ("MCP dispatch", MCP_DISPATCH_SOURCE, 320),
        ("Store publication", STORE_PUBLICATION_SOURCE, 120),
        (
            "Store publication cancellation regressions",
            APP_CANCELLATION_REGRESSIONS,
            360,
        ),
    ] {
        let lines = source.lines().count();
        assert!(lines <= max_lines, "{name} grew to {lines} lines");
    }
}
