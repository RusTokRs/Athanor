const DAEMON_SOURCE: &str = include_str!("../src/daemon.rs");
const QUERIES_SOURCE: &str = include_str!("../src/daemon_queries.rs");
const DERIVED_READ_SOURCE: &str = include_str!("../src/daemon_derived_read_dispatch.rs");
const WRITE_JOBS_SOURCE: &str = include_str!("../src/daemon_write_jobs.rs");
const ATHD_SOURCE: &str = include_str!("../../../apps/athd/src/main.rs");

#[test]
fn daemon_read_paths_require_explicit_runtime_composition() {
    assert!(ATHD_SOURCE.contains("serve_daemon_with_composition"));

    assert!(QUERIES_SOURCE.contains("composition.init_store"));
    assert!(QUERIES_SOURCE.contains("composition.build_search_index_with_operation_context"));
    assert!(QUERIES_SOURCE.contains("daemon runtime composition is unavailable"));
    assert!(!QUERIES_SOURCE.contains("use crate::store::init_store"));
    assert!(!QUERIES_SOURCE.contains("get_or_build_search_index_with_operation_context"));
    assert!(!QUERIES_SOURCE.contains("None => init_store"));

    assert!(DERIVED_READ_SOURCE
        .contains("context_project_with_composition_and_operation_context"));
    assert!(DERIVED_READ_SOURCE
        .contains("change_map_project_with_composition_and_operation_context"));
    assert!(DERIVED_READ_SOURCE.contains("daemon runtime composition is unavailable"));
    assert!(!DERIVED_READ_SOURCE.contains("context_project_with_operation_context,"));
    assert!(!DERIVED_READ_SOURCE.contains("change_map_project_with_operation_context,"));
}

#[test]
fn daemon_write_jobs_require_explicit_runtime_composition() {
    for symbol in [
        "index_project_cancellable_with_composition_and_operation_context",
        "generate_project_cancellable_with_composition_and_operation_context",
        "project_wiki_cancellable_with_composition_and_operation_context",
        "project_html_report_cancellable_with_composition_and_operation_context",
    ] {
        assert!(WRITE_JOBS_SOURCE.contains(symbol));
    }
    assert!(WRITE_JOBS_SOURCE.contains("fn required_composition"));
    assert!(WRITE_JOBS_SOURCE.contains("daemon runtime composition is unavailable"));
    assert!(!WRITE_JOBS_SOURCE.contains("match composition"));
    assert!(!WRITE_JOBS_SOURCE.contains("index_project_cancellable_with_operation_context,"));
    assert!(!WRITE_JOBS_SOURCE.contains("generate_project_cancellable_with_operation_context,"));
    assert!(!WRITE_JOBS_SOURCE.contains("project_wiki_cancellable_with_operation_context,"));
    assert!(!WRITE_JOBS_SOURCE.contains("project_html_report_cancellable_with_operation_context,"));
}

#[test]
fn remaining_daemon_host_debt_is_explicit() {
    assert!(DAEMON_SOURCE.contains("composition: Option<RuntimeComposition>"));
    assert!(DAEMON_SOURCE.contains("serve_daemon_inner(options, None)"));
}
