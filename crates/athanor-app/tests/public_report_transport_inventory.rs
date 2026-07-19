const CLI_INDEX_SOURCE: &str = include_str!("../../../apps/ath/src/index_cli.rs");
const DAEMON_WRITE_SOURCE: &str = include_str!("../src/daemon_write_jobs.rs");
const DAEMON_PARITY_TESTS: &str = include_str!("../src/daemon_write_job_contract_tests.rs");
const MCP_DISPATCH_SOURCE: &str =
    include_str!("../../athanor-transport-mcp/src/tools/dispatch.rs");
const MCP_INDEX_INVENTORY: &str =
    include_str!("../../athanor-transport-mcp/tests/index_publication_cancellation_inventory.rs");

#[test]
fn index_report_uses_one_typed_payload_across_cli_daemon_and_mcp() {
    assert!(CLI_INDEX_SOURCE.contains("fn render_index(report: &IndexReport"));
    assert!(CLI_INDEX_SOURCE.contains("serde_json::to_string_pretty(report)"));

    assert!(DAEMON_WRITE_SOURCE.contains(
        "pub(crate) fn index_job_result(report: &IndexReport)"
    ));
    assert!(DAEMON_WRITE_SOURCE.contains("serialize_job_result(report)"));

    assert!(MCP_DISPATCH_SOURCE.contains(
        "index_project_with_composition_and_operation_context"
    ));
    assert!(MCP_DISPATCH_SOURCE.contains(
        "return Ok(serde_json::to_string_pretty(&report)?);"
    ));

    assert!(MCP_INDEX_INVENTORY.contains(
        "cli_daemon_and_mcp_share_the_public_index_report_payload"
    ));
    assert!(MCP_INDEX_INVENTORY.contains(
        "daemon_index_result_matches_public_index_report_shape"
    ));
    assert!(MCP_INDEX_INVENTORY.contains("assert_eq!(daemon, direct)"));
}

#[test]
fn daemon_write_reports_match_direct_public_contract_serialization() {
    for regression in [
        "daemon_index_result_matches_public_index_report_shape",
        "daemon_generation_result_matches_public_generation_report_shape",
        "daemon_html_result_matches_public_html_report_shape",
        "daemon_wiki_result_matches_public_wiki_report_shape",
    ] {
        assert!(
            DAEMON_PARITY_TESTS.contains(regression),
            "missing daemon payload parity regression {regression}"
        );
    }
    assert!(DAEMON_PARITY_TESTS.contains("assert_eq!(daemon, direct)"));
}

#[test]
fn transport_parity_inventory_remains_bounded() {
    for (name, source, max_lines) in [
        ("CLI index", CLI_INDEX_SOURCE, 280),
        ("daemon write jobs", DAEMON_WRITE_SOURCE, 390),
        ("daemon parity tests", DAEMON_PARITY_TESTS, 180),
        ("MCP dispatch", MCP_DISPATCH_SOURCE, 320),
        ("MCP index inventory", MCP_INDEX_INVENTORY, 150),
    ] {
        let lines = source.lines().count();
        assert!(lines <= max_lines, "{name} grew to {lines} lines");
    }
}
