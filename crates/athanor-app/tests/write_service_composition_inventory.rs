const APP_LIB_SOURCE: &str = include_str!("../src/lib.rs");
const INDEX_SOURCE: &str = include_str!("../src/index_runtime.rs");
const GENERATION_SOURCE: &str = include_str!("../src/generation.rs");
const WIKI_SOURCE: &str = include_str!("../src/wiki.rs");
const REPORT_SOURCE: &str = include_str!("../src/report.rs");
const BENCH_SOURCE: &str = include_str!("../src/bench.rs");
const PROJECTION_SOURCE: &str = include_str!("../src/projection.rs");
const INDEX_CLI_SOURCE: &str = include_str!("../../../apps/ath/src/index_cli.rs");
const GENERATION_CLI_SOURCE: &str =
    include_str!("../../../apps/ath/src/direct_generation_cli.rs");

#[test]
fn public_write_services_are_composition_only() {
    for (source, required) in [
        (INDEX_SOURCE, "index_project_with_composition"),
        (GENERATION_SOURCE, "generate_project_with_composition"),
        (WIKI_SOURCE, "project_wiki_with_composition"),
        (REPORT_SOURCE, "project_html_report_with_composition"),
        (BENCH_SOURCE, "benchmark_index_with_composition"),
    ] {
        assert!(source.contains(required));
        assert!(!source.contains("Option<RuntimeComposition>"));
        assert!(!source.contains("Option<&RuntimeComposition>"));
        assert!(!source.contains("explicit RuntimeComposition is required"));
    }

    for removed in [
        "pub async fn index_project(",
        "pub async fn index_project_with_operation_context(",
        "pub async fn index_project_cancellable(",
        "pub async fn index_project_cancellable_with_operation_context(",
        "pub async fn generate_project(",
        "pub async fn generate_project_with_operation_context(",
        "pub async fn generate_project_cancellable(",
        "pub async fn generate_project_cancellable_with_operation_context(",
        "pub async fn project_wiki(",
        "pub async fn project_wiki_with_operation_context(",
        "pub async fn project_wiki_cancellable(",
        "pub async fn project_wiki_cancellable_with_operation_context(",
        "pub async fn project_html_report(",
        "pub async fn project_html_report_with_operation_context(",
        "pub async fn project_html_report_cancellable(",
        "pub async fn project_html_report_cancellable_with_operation_context(",
        "pub async fn benchmark_index(",
    ] {
        assert!(
            ![
                INDEX_SOURCE,
                GENERATION_SOURCE,
                WIKI_SOURCE,
                REPORT_SOURCE,
                BENCH_SOURCE,
            ]
            .iter()
            .any(|source| source.contains(removed)),
            "removed write-service API returned: {removed}"
        );
    }
}

#[test]
fn write_service_cores_have_no_runtime_or_projector_fallbacks() {
    assert!(INDEX_SOURCE.contains("composition.init_store"));
    assert!(INDEX_SOURCE.contains("RuntimeBuilder::from_composition"));
    assert!(!INDEX_SOURCE.contains("crate::store::init_store"));
    assert!(!INDEX_SOURCE.contains("RuntimeBuilder::new(root)"));
    assert!(!INDEX_SOURCE.contains("match composition"));

    assert!(GENERATION_SOURCE.contains("composition.init_store"));
    assert!(GENERATION_SOURCE.contains("wiki_composition"));
    assert!(GENERATION_SOURCE.contains(".project_wiki"));
    assert!(GENERATION_SOURCE.contains("html_composition"));
    assert!(GENERATION_SOURCE.contains(".project_html"));
    assert!(!GENERATION_SOURCE.contains("crate::store::init_store"));
    assert!(!GENERATION_SOURCE.contains("project_wiki_payload"));
    assert!(!GENERATION_SOURCE.contains("project_html_payload"));
    assert!(!GENERATION_SOURCE.contains("match wiki_composition"));
    assert!(!GENERATION_SOURCE.contains("match html_composition"));

    for source in [WIKI_SOURCE, REPORT_SOURCE] {
        assert!(source.contains("composition.init_store"));
        assert!(!source.contains("crate::store::init_store"));
        assert!(!source.contains("match composition"));
    }
    assert!(WIKI_SOURCE.contains(".project_wiki"));
    assert!(REPORT_SOURCE.contains(".project_html"));
    assert!(!PROJECTION_SOURCE.contains("project_wiki_payload"));
    assert!(!PROJECTION_SOURCE.contains("project_html_payload"));
}

#[test]
fn stable_reexports_and_hosts_use_composition_apis() {
    for symbol in [
        "index_project_with_composition",
        "index_project_with_composition_and_operation_context",
        "index_project_cancellable_with_composition",
        "index_project_cancellable_with_composition_and_operation_context",
    ] {
        assert!(APP_LIB_SOURCE.contains(symbol));
    }
    for removed in [
        "IndexReportMetrics, index_project,",
        "index_project_cancellable_with_operation_context",
        "index_project_with_operation_context",
    ] {
        assert!(!APP_LIB_SOURCE.contains(removed));
    }

    assert!(INDEX_CLI_SOURCE.contains("index_project_with_composition"));
    assert!(INDEX_CLI_SOURCE.contains("benchmark_index_with_composition"));
    assert!(GENERATION_CLI_SOURCE.contains("generate_project_with_composition"));
    assert!(GENERATION_CLI_SOURCE.contains("project_wiki_with_composition"));
    assert!(GENERATION_CLI_SOURCE.contains("project_html_report_with_composition"));
}

#[test]
fn write_service_owners_remain_bounded() {
    for (name, source, max_lines) in [
        ("index", INDEX_SOURCE, 600),
        ("generation", GENERATION_SOURCE, 650),
        ("wiki", WIKI_SOURCE, 260),
        ("HTML report", REPORT_SOURCE, 230),
        ("benchmark", BENCH_SOURCE, 240),
        ("projection contract", PROJECTION_SOURCE, 20),
    ] {
        let lines = source.lines().count();
        assert!(lines <= max_lines, "{name} grew to {lines} lines");
    }
}
