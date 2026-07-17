const APPLICATION_FACADE_SOURCE: &str =
    include_str!("../src/application_report_composition.rs");
const API_DIRECT_SOURCE: &str =
    include_str!("../src/application_report_composition/api_direct.rs");
const DOCS_DIRECT_ROOT_SOURCE: &str =
    include_str!("../src/application_report_composition/docs_direct.rs");
const DOCS_DIRECT_SNAPSHOT_SOURCE: &str =
    include_str!("../src/application_report_composition/docs_direct/snapshot.rs");
const DOCS_DIRECT_CHECK_SOURCE: &str =
    include_str!("../src/application_report_composition/docs_direct/check.rs");
const DOCS_DIRECT_APPLY_SOURCE: &str =
    include_str!("../src/application_report_composition/docs_direct/apply.rs");
const REPAIR_FACADE_SOURCE: &str = include_str!("../src/repair_composition.rs");
const REPAIR_DIRECT_SOURCE: &str = include_str!("../src/repair_composition/direct.rs");

#[test]
fn api_services_initialize_store_directly_from_composition() {
    assert!(APPLICATION_FACADE_SOURCE.contains("api_direct::snapshot"));
    assert!(APPLICATION_FACADE_SOURCE.contains("api_direct::registry"));
    assert!(API_DIRECT_SOURCE.contains("composition.init_store"));
    assert!(!API_DIRECT_SOURCE.contains("with_store_composition"));
    assert!(!API_DIRECT_SOURCE.contains("crate::store::init_store"));
}

#[test]
fn repair_services_initialize_store_directly_from_composition() {
    assert!(REPAIR_FACADE_SOURCE.contains("direct::recover_index"));
    assert!(REPAIR_FACADE_SOURCE.contains("direct::repair_latest"));
    assert!(REPAIR_DIRECT_SOURCE.contains("composition.init_store"));
    assert!(!REPAIR_FACADE_SOURCE.contains("with_store_composition"));
    assert!(!REPAIR_DIRECT_SOURCE.contains("with_store_composition"));
    assert!(!REPAIR_DIRECT_SOURCE.contains("crate::store::init_store"));
}

#[test]
fn docs_check_drift_and_apply_initialize_store_directly() {
    for operation in [
        "docs_direct::check",
        "docs_direct::drift",
        "docs_direct::apply",
    ] {
        assert!(APPLICATION_FACADE_SOURCE.contains(operation));
    }
    assert!(DOCS_DIRECT_ROOT_SOURCE.contains("mod snapshot;"));
    assert!(DOCS_DIRECT_ROOT_SOURCE.contains("mod check;"));
    assert!(DOCS_DIRECT_ROOT_SOURCE.contains("mod apply;"));
    assert!(DOCS_DIRECT_SNAPSHOT_SOURCE.contains("composition.init_store"));
    assert!(DOCS_DIRECT_CHECK_SOURCE.contains("build_docs_drift_report"));
    for source in [
        DOCS_DIRECT_ROOT_SOURCE,
        DOCS_DIRECT_SNAPSHOT_SOURCE,
        DOCS_DIRECT_CHECK_SOURCE,
        DOCS_DIRECT_APPLY_SOURCE,
    ] {
        assert!(!source.contains("with_store_composition"));
        assert!(!source.contains("crate::store::init_store"));
    }
}

#[test]
fn docs_propose_fix_is_the_only_remaining_task_local_service_bridge() {
    assert_eq!(
        APPLICATION_FACADE_SOURCE
            .matches("with_store_composition")
            .count(),
        2,
        "one import plus docs propose-fix should remain"
    );
    assert!(APPLICATION_FACADE_SOURCE.contains("docs_propose_fix(options)"));
    for removed in [
        "check_docs(options)",
        "docs_drift(options)",
        "docs_apply_patch(options)",
    ] {
        assert!(!APPLICATION_FACADE_SOURCE.contains(removed));
    }
}

#[test]
fn direct_service_composition_modules_remain_bounded() {
    for (name, source, max_lines) in [
        ("application facade", APPLICATION_FACADE_SOURCE, 80),
        ("API direct", API_DIRECT_SOURCE, 280),
        ("Docs direct root", DOCS_DIRECT_ROOT_SOURCE, 20),
        ("Docs direct snapshot", DOCS_DIRECT_SNAPSHOT_SOURCE, 80),
        ("Docs direct check", DOCS_DIRECT_CHECK_SOURCE, 220),
        ("Docs direct apply", DOCS_DIRECT_APPLY_SOURCE, 220),
        ("repair facade", REPAIR_FACADE_SOURCE, 40),
        ("repair direct", REPAIR_DIRECT_SOURCE, 280),
    ] {
        let lines = source.lines().count();
        assert!(lines <= max_lines, "{name} grew to {lines} lines");
    }
}
