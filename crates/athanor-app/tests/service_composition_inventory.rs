const APPLICATION_FACADE_SOURCE: &str = include_str!("../src/application_report_composition.rs");
const API_DIRECT_SOURCE: &str = include_str!("../src/application_report_composition/api_direct.rs");
const DOCS_ROOT_SOURCE: &str = include_str!("../src/docs.rs");
const DOCS_MODEL_SOURCE: &str = include_str!("../src/docs/model.rs");
const DOCS_CHECK_SOURCE: &str = include_str!("../src/docs/check.rs");
const DOCS_SERVICE_SOURCE: &str = include_str!("../src/docs/service.rs");
const DOCS_PROPOSAL_SOURCE: &str = include_str!("../src/docs/proposal.rs");
const DOCS_PROPOSAL_API_SOURCE: &str = include_str!("../src/docs/proposal/api.rs");
const DOCS_OPERATIONS_SOURCE: &str = include_str!("../src/docs/operations.rs");
const DOCS_API_ROOT_SOURCE: &str = include_str!("../src/docs/api_docs.rs");
const DOCS_API_CONTENT_SOURCE: &str = include_str!("../src/docs/api_docs/content.rs");
const DOCS_API_UPDATE_SOURCE: &str = include_str!("../src/docs/api_docs/update.rs");
const DOCS_API_NARRATIVE_SOURCE: &str = include_str!("../src/docs/api_docs/narrative.rs");
const DOCS_DIRECT_ROOT_SOURCE: &str =
    include_str!("../src/application_report_composition/docs_direct.rs");
const DOCS_DIRECT_SNAPSHOT_SOURCE: &str =
    include_str!("../src/application_report_composition/docs_direct/snapshot.rs");
const DOCS_DIRECT_CHECK_SOURCE: &str =
    include_str!("../src/application_report_composition/docs_direct/check.rs");
const DOCS_DIRECT_PROPOSE_SOURCE: &str =
    include_str!("../src/application_report_composition/docs_direct/propose.rs");
const DOCS_DIRECT_APPLY_SOURCE: &str =
    include_str!("../src/application_report_composition/docs_direct/apply.rs");
const REPAIR_FACADE_SOURCE: &str = include_str!("../src/repair_composition.rs");
const REPAIR_DIRECT_SOURCE: &str = include_str!("../src/repair_composition/direct.rs");

#[test]
fn api_and_repair_services_initialize_store_directly() {
    assert!(APPLICATION_FACADE_SOURCE.contains("api_direct::snapshot"));
    assert!(APPLICATION_FACADE_SOURCE.contains("api_direct::registry"));
    assert!(API_DIRECT_SOURCE.contains("composition.init_store"));
    assert!(REPAIR_FACADE_SOURCE.contains("direct::recover_index"));
    assert!(REPAIR_FACADE_SOURCE.contains("direct::repair_latest"));
    assert!(REPAIR_DIRECT_SOURCE.contains("composition.init_store"));
    for source in [
        API_DIRECT_SOURCE,
        REPAIR_FACADE_SOURCE,
        REPAIR_DIRECT_SOURCE,
    ] {
        assert!(!source.contains("with_store_composition"));
    }
}

#[test]
fn every_docs_direct_service_uses_explicit_composition() {
    for operation in [
        "docs_direct::check",
        "docs_direct::drift",
        "docs_direct::propose",
        "docs_direct::apply",
    ] {
        assert!(APPLICATION_FACADE_SOURCE.contains(operation));
    }
    for module in ["snapshot", "check", "propose", "apply"] {
        assert!(DOCS_DIRECT_ROOT_SOURCE.contains(&format!("mod {module};")));
    }
    assert!(DOCS_DIRECT_SNAPSHOT_SOURCE.contains("composition.init_store"));
    assert!(DOCS_DIRECT_CHECK_SOURCE.contains("build_docs_drift_report"));
    assert!(DOCS_DIRECT_PROPOSE_SOURCE.contains("build_docs_patch_proposal_from_snapshot"));
    for source in [
        APPLICATION_FACADE_SOURCE,
        DOCS_DIRECT_ROOT_SOURCE,
        DOCS_DIRECT_SNAPSHOT_SOURCE,
        DOCS_DIRECT_CHECK_SOURCE,
        DOCS_DIRECT_PROPOSE_SOURCE,
        DOCS_DIRECT_APPLY_SOURCE,
    ] {
        assert!(!source.contains("with_store_composition"));
        assert!(!source.contains("crate::store::init_store"));
    }
}

#[test]
fn docs_engine_is_conventional_and_has_no_legacy_include() {
    for module in [
        "api_docs",
        "check",
        "frontmatter",
        "model",
        "operations",
        "proposal",
        "service",
    ] {
        assert!(DOCS_ROOT_SOURCE.contains(&format!("mod {module};")));
    }
    for source in [
        DOCS_ROOT_SOURCE,
        DOCS_MODEL_SOURCE,
        DOCS_CHECK_SOURCE,
        DOCS_SERVICE_SOURCE,
        DOCS_PROPOSAL_SOURCE,
        DOCS_PROPOSAL_API_SOURCE,
        DOCS_OPERATIONS_SOURCE,
        DOCS_API_ROOT_SOURCE,
        DOCS_API_CONTENT_SOURCE,
        DOCS_API_UPDATE_SOURCE,
        DOCS_API_NARRATIVE_SOURCE,
    ] {
        assert!(!source.contains("include!("));
        assert!(!source.contains("legacy_impl"));
    }
    assert!(DOCS_PROPOSAL_SOURCE.contains("build_docs_patch_proposal_from_snapshot"));
}

#[test]
fn direct_service_and_docs_engine_modules_remain_bounded() {
    for (name, source, max_lines) in [
        ("application facade", APPLICATION_FACADE_SOURCE, 80),
        ("API direct", API_DIRECT_SOURCE, 280),
        ("Docs root", DOCS_ROOT_SOURCE, 40),
        ("Docs model", DOCS_MODEL_SOURCE, 140),
        ("Docs check", DOCS_CHECK_SOURCE, 260),
        ("Docs service", DOCS_SERVICE_SOURCE, 220),
        ("Docs proposal root", DOCS_PROPOSAL_SOURCE, 120),
        ("Docs proposal API", DOCS_PROPOSAL_API_SOURCE, 180),
        ("Docs operations", DOCS_OPERATIONS_SOURCE, 260),
        ("Docs API root", DOCS_API_ROOT_SOURCE, 40),
        ("Docs API content", DOCS_API_CONTENT_SOURCE, 380),
        ("Docs API update", DOCS_API_UPDATE_SOURCE, 260),
        ("Docs API narrative", DOCS_API_NARRATIVE_SOURCE, 260),
        ("Docs direct root", DOCS_DIRECT_ROOT_SOURCE, 20),
        ("Docs direct snapshot", DOCS_DIRECT_SNAPSHOT_SOURCE, 80),
        ("Docs direct check", DOCS_DIRECT_CHECK_SOURCE, 220),
        ("Docs direct propose", DOCS_DIRECT_PROPOSE_SOURCE, 140),
        ("Docs direct apply", DOCS_DIRECT_APPLY_SOURCE, 220),
        ("repair facade", REPAIR_FACADE_SOURCE, 40),
        ("repair direct", REPAIR_DIRECT_SOURCE, 280),
    ] {
        let lines = source.lines().count();
        assert!(lines <= max_lines, "{name} grew to {lines} lines");
    }
}
