const APPLICATION_FACADE_SOURCE: &str =
    include_str!("../src/application_report_composition.rs");
const API_DIRECT_SOURCE: &str =
    include_str!("../src/application_report_composition/api_direct.rs");
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
fn docs_are_the_only_remaining_task_local_service_bridge() {
    assert_eq!(
        APPLICATION_FACADE_SOURCE
            .matches("with_store_composition")
            .count(),
        5,
        "one import plus four Docs operations should remain"
    );
    for operation in [
        "check_docs(options)",
        "docs_drift(options)",
        "docs_propose_fix(options)",
        "docs_apply_patch(options)",
    ] {
        assert!(APPLICATION_FACADE_SOURCE.contains(operation));
    }
}

#[test]
fn direct_service_composition_modules_remain_bounded() {
    for (name, source, max_lines) in [
        ("application facade", APPLICATION_FACADE_SOURCE, 80),
        ("API direct", API_DIRECT_SOURCE, 280),
        ("repair facade", REPAIR_FACADE_SOURCE, 40),
        ("repair direct", REPAIR_DIRECT_SOURCE, 280),
    ] {
        let lines = source.lines().count();
        assert!(lines <= max_lines, "{name} grew to {lines} lines");
    }
}
