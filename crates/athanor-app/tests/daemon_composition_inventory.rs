const APP_LIB_SOURCE: &str = include_str!("../src/lib.rs");
const DAEMON_SOURCE: &str = include_str!("../src/daemon.rs");
const COMMAND_DISPATCH_SOURCE: &str = include_str!("../src/daemon_command_dispatch.rs");
const QUERIES_SOURCE: &str = include_str!("../src/daemon_queries.rs");
const DERIVED_READ_SOURCE: &str = include_str!("../src/daemon_derived_read_dispatch.rs");
const READ_DISPATCH_SOURCE: &str = include_str!("../src/daemon_read_dispatch.rs");
const READ_TEST_SOURCE: &str = include_str!("../src/daemon_read_dispatch_tests.rs");
const WRITE_JOBS_SOURCE: &str = include_str!("../src/daemon_write_jobs.rs");
const WRITE_TEST_SOURCE: &str = include_str!("../src/daemon_write_job_contract_tests.rs");
const ATHD_SOURCE: &str = include_str!("../../../apps/athd/src/main.rs");

#[test]
fn daemon_host_requires_explicit_runtime_composition() {
    assert!(ATHD_SOURCE.contains("serve_daemon_with_composition"));
    assert!(DAEMON_SOURCE.contains("composition: RuntimeComposition"));
    assert!(DAEMON_SOURCE.contains("pub async fn serve_daemon_with_composition"));
    assert!(!DAEMON_SOURCE.contains("composition: Option<RuntimeComposition>"));
    assert!(!DAEMON_SOURCE.contains("pub async fn serve_daemon("));
    assert!(!DAEMON_SOURCE.contains("serve_daemon_inner"));
    assert!(!DAEMON_SOURCE.contains("execute_request"));
}

#[test]
fn daemon_query_and_derived_read_paths_use_total_composition() {
    assert!(QUERIES_SOURCE.contains("fn composition(state: &DaemonState) -> RuntimeComposition"));
    assert!(QUERIES_SOURCE.contains("composition.init_store"));
    assert!(QUERIES_SOURCE.contains("composition.build_search_index_with_operation_context"));
    assert!(!QUERIES_SOURCE.contains("daemon runtime composition is unavailable"));
    assert!(!QUERIES_SOURCE.contains("use crate::store::init_store"));
    assert!(!QUERIES_SOURCE.contains("get_or_build_search_index_with_operation_context"));

    assert!(DERIVED_READ_SOURCE
        .contains("context_project_with_composition_and_operation_context"));
    assert!(DERIVED_READ_SOURCE
        .contains("change_map_project_with_composition_and_operation_context"));
    assert!(!DERIVED_READ_SOURCE.contains("match crate::daemon_queries::composition"));
    assert!(!DERIVED_READ_SOURCE.contains("daemon runtime composition is unavailable"));
    assert!(!DERIVED_READ_SOURCE.contains("context_project_with_operation_context,"));
    assert!(!DERIVED_READ_SOURCE.contains("change_map_project_with_operation_context,"));
}

#[test]
fn daemon_write_jobs_use_total_composition() {
    for symbol in [
        "index_project_cancellable_with_composition_and_operation_context",
        "generate_project_cancellable_with_composition_and_operation_context",
        "project_wiki_cancellable_with_composition_and_operation_context",
        "project_html_report_cancellable_with_composition_and_operation_context",
    ] {
        assert!(WRITE_JOBS_SOURCE.contains(symbol));
    }
    assert!(WRITE_JOBS_SOURCE.contains(
        "fn required_composition(state: &DaemonState) -> crate::RuntimeComposition"
    ));
    assert!(!WRITE_JOBS_SOURCE.contains("daemon runtime composition is unavailable"));
    assert!(!WRITE_JOBS_SOURCE.contains("required_composition(state)?"));
    assert!(!WRITE_JOBS_SOURCE.contains("match composition"));
    assert!(!WRITE_JOBS_SOURCE.contains("index_project_cancellable_with_operation_context,"));
    assert!(!WRITE_JOBS_SOURCE.contains("generate_project_cancellable_with_operation_context,"));
    assert!(!WRITE_JOBS_SOURCE.contains("project_wiki_cancellable_with_operation_context,"));
    assert!(!WRITE_JOBS_SOURCE.contains("project_html_report_cancellable_with_operation_context,"));
}

#[test]
fn daemon_dispatch_ownership_is_bounded() {
    assert!(APP_LIB_SOURCE.contains("mod daemon_command_dispatch;"));
    assert!(APP_LIB_SOURCE.contains("mod daemon_read_dispatch_tests;"));
    assert!(APP_LIB_SOURCE.contains("mod daemon_write_job_contract_tests;"));
    assert!(READ_DISPATCH_SOURCE.contains("crate::daemon_command_dispatch::execute"));
    assert!(!READ_DISPATCH_SOURCE.contains("crate::daemon::execute_request"));
    assert!(COMMAND_DISPATCH_SOURCE.contains("Handles control-plane and write commands"));
    assert!(COMMAND_DISPATCH_SOURCE.contains("&request.command"));
    assert!(READ_TEST_SOURCE.contains("cancel_request_terminates_running_read_job"));
    assert!(READ_TEST_SOURCE.contains("hard_deadline_returns_stable_error_and_fails_job"));
    assert!(READ_TEST_SOURCE.contains("composition: crate::test_runtime::composition()"));
    assert!(WRITE_TEST_SOURCE.contains("daemon_index_result_matches_public_index_report_shape"));
    assert!(WRITE_TEST_SOURCE.contains("daemon_generation_result_matches_public_generation_report_shape"));
    assert!(WRITE_TEST_SOURCE.contains("daemon_html_result_matches_public_html_report_shape"));
    assert!(WRITE_TEST_SOURCE.contains("daemon_wiki_result_matches_public_wiki_report_shape"));

    for (name, source, max_lines) in [
        ("daemon host", DAEMON_SOURCE, 900),
        ("command dispatcher", COMMAND_DISPATCH_SOURCE, 380),
        ("read dispatcher", READ_DISPATCH_SOURCE, 300),
        ("derived read dispatcher", DERIVED_READ_SOURCE, 280),
        ("daemon queries", QUERIES_SOURCE, 380),
        ("daemon write jobs", WRITE_JOBS_SOURCE, 380),
        ("daemon read tests", READ_TEST_SOURCE, 230),
        ("daemon write contract tests", WRITE_TEST_SOURCE, 160),
    ] {
        let lines = source.lines().count();
        assert!(lines <= max_lines, "{name} grew to {lines} lines");
    }
}
