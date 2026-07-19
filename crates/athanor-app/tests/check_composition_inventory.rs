const CHECK_ROOT: &str = include_str!("../src/check.rs");
const CHECK_MODEL: &str = include_str!("../src/check/model.rs");
const CHECK_EXECUTION: &str = include_str!("../src/check/execution.rs");
const CHECK_DIAGNOSTICS: &str = include_str!("../src/check/diagnostics.rs");
const CHECK_AFFECTED: &str = include_str!("../src/check/affected.rs");
const CHECK_TESTS: &str = include_str!("../src/check/tests.rs");
const DIRECT_CHECK_CLI: &str = include_str!("../../../apps/ath/src/direct_check_cli.rs");

#[test]
fn check_root_has_conventional_bounded_owners() {
    for module in ["affected", "diagnostics", "execution", "model"] {
        assert!(CHECK_ROOT.contains(&format!("mod {module};")));
    }
    for export in [
        "pub use diagnostics::build_check_report;",
        "pub use execution::{",
        "pub use model::{",
    ] {
        assert!(CHECK_ROOT.contains(export));
    }
    assert!(CHECK_ROOT.contains("#[cfg(test)]\nmod tests;"));
    assert!(!CHECK_ROOT.contains("include!("));
    assert!(!CHECK_ROOT.contains("facade"));
}

#[test]
fn check_execution_requires_explicit_composition() {
    for entrypoint in [
        "pub async fn check_project_with_composition(",
        "pub async fn check_affected_with_composition(",
        "pub async fn check_operations_docs_with_composition(",
    ] {
        assert!(CHECK_EXECUTION.contains(entrypoint));
    }
    for legacy in [
        "pub async fn check_project(",
        "pub async fn check_affected(",
        "pub async fn check_operations_docs(",
    ] {
        assert!(!CHECK_EXECUTION.contains(legacy));
        assert!(!CHECK_ROOT.contains(legacy));
    }
    assert!(CHECK_EXECUTION.contains("composition: &RuntimeComposition"));
    assert!(CHECK_EXECUTION.contains("composition.init_store(&root, &config)"));
    assert!(!CHECK_EXECUTION.contains("Option<&RuntimeComposition>"));
    assert!(!CHECK_EXECUTION.contains("crate::store::init_store"));
    assert!(!CHECK_EXECUTION.contains("match composition"));
}

#[test]
fn active_cli_uses_only_composition_check_entrypoints() {
    assert!(DIRECT_CHECK_CLI.contains("check_project_with_composition("));
    assert!(DIRECT_CHECK_CLI.contains("check_affected_with_composition("));
    assert!(!DIRECT_CHECK_CLI.contains("athanor_app::check_project("));
    assert!(!DIRECT_CHECK_CLI.contains("athanor_app::check_affected("));
}

#[test]
fn check_modules_remain_bounded() {
    for (name, source, max_lines) in [
        ("Check root", CHECK_ROOT, 30),
        ("Check model", CHECK_MODEL, 130),
        ("Check execution", CHECK_EXECUTION, 140),
        ("Check diagnostics", CHECK_DIAGNOSTICS, 240),
        ("Check affected", CHECK_AFFECTED, 340),
        ("Check tests", CHECK_TESTS, 560),
    ] {
        let lines = source.lines().count();
        assert!(lines <= max_lines, "{name} grew to {lines} lines");
    }
}
