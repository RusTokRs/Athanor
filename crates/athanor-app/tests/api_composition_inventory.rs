const API_ROOT: &str = include_str!("../src/api.rs");
const API_MODEL: &str = include_str!("../src/api/model.rs");
const API_SNAPSHOT: &str = include_str!("../src/api/snapshot.rs");
const API_DIFF: &str = include_str!("../src/api/diff.rs");
const API_RETENTION: &str = include_str!("../src/api/retention.rs");
const API_TESTS: &str = include_str!("../src/api/tests.rs");
const APPLICATION_REPORT_COMPOSITION: &str =
    include_str!("../src/application_report_composition.rs");
const API_DIRECT: &str = include_str!("../src/application_report_composition/api_direct.rs");
const API_CLI: &str = include_str!("../../../apps/ath/src/api_cli.rs");

#[test]
fn api_root_has_conventional_bounded_owners() {
    for module in ["diff", "model", "retention", "snapshot"] {
        assert!(API_ROOT.contains(&format!("mod {module};")));
    }
    for export in [
        "pub use diff::diff_api_contracts;",
        "pub use model::{",
        "pub use retention::cleanup_api_contracts;",
        "pub(crate) use snapshot::publish_api_contract_snapshot;",
    ] {
        assert!(API_ROOT.contains(export));
    }
    let normalized_root = API_ROOT.replace("\r\n", "\n");
    assert!(normalized_root.contains("#[cfg(test)]\nmod tests;"));
    assert!(!API_ROOT.contains("include!("));
    assert!(!API_ROOT.contains("facade"));
}

#[test]
fn api_snapshot_execution_requires_explicit_composition() {
    assert!(
        APPLICATION_REPORT_COMPOSITION
            .contains("pub async fn snapshot_api_contract_with_composition(")
    );
    assert!(!API_ROOT.contains("pub async fn snapshot_api_contract("));
    assert!(!API_ROOT.contains("crate::store::init_store"));
    assert!(!API_ROOT.contains("Option<&RuntimeComposition>"));

    assert!(API_DIRECT.contains("pub(super) async fn snapshot("));
    assert!(API_DIRECT.contains("composition: &RuntimeComposition"));
    assert!(API_DIRECT.contains("composition.init_store(&root, &config)"));
    assert!(API_DIRECT.contains("publish_api_contract_snapshot("));
    assert!(!API_DIRECT.contains("crate::store::init_store"));
    assert!(!API_DIRECT.contains("Option<&RuntimeComposition>"));
    assert!(!API_DIRECT.contains("fn build_contract("));
    assert!(!API_DIRECT.contains("fn contract_items("));
    assert!(!API_DIRECT.contains("fn write_immutable("));
    assert!(!API_DIRECT.contains("fn maybe_cleanup("));
}

#[test]
fn active_cli_uses_composition_snapshot_entrypoint() {
    assert!(API_CLI.contains("snapshot_api_contract_with_composition("));
    assert!(!API_CLI.contains("athanor_app::snapshot_api_contract("));
}

#[test]
fn api_modules_remain_bounded() {
    for (name, source, max_lines) in [
        ("API root", API_ROOT, 35),
        ("API model", API_MODEL, 190),
        ("API snapshot", API_SNAPSHOT, 160),
        ("API diff", API_DIFF, 430),
        ("API retention", API_RETENTION, 260),
        ("API tests", API_TESTS, 520),
        ("API composition direct", API_DIRECT, 180),
    ] {
        let lines = source.lines().count();
        assert!(lines <= max_lines, "{name} grew to {lines} lines");
    }
}
