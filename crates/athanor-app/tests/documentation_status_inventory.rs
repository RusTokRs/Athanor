const ROADMAP: &str = include_str!("../../../docs/development/roadmap-status.md");
const PIPELINE: &str = include_str!("../../../docs/architecture/pipeline.md");

#[test]
fn aggregate_status_documents_do_not_claim_unexecuted_verification() {
    for (name, source) in [("roadmap", ROADMAP), ("pipeline", PIPELINE)] {
        assert!(source.contains("status: active"), "{name} must remain an active status document");
        assert!(!source.contains("status: verified"), "{name} must not use snapshot verification as current-commit evidence");
        assert!(!source.contains("last_verified_snapshot:"), "{name} must not carry stale snapshot verification metadata");
        assert!(!source.contains("Status: verified."), "{name} must not contain aggregate verified claims without one-commit execution evidence");
        assert!(source.contains("implemented, not verified") || source.contains("Implemented means"));
    }
}

#[test]
fn pipeline_separates_current_target_and_history() {
    for heading in [
        "## Current Architecture",
        "## Target Architecture",
        "## Historical Notes",
        "## Verification Matrix",
    ] {
        assert!(PIPELINE.contains(heading), "pipeline is missing {heading}");
    }

    for current_owner in [
        "index_runtime.rs",
        "pipeline_source.rs",
        "pipeline_extract.rs",
        "pipeline_link.rs",
        "pipeline_check.rs",
        "pipeline_support.rs",
        "index_publication.rs",
        "index_publication_snapshot.rs",
        "RuntimeComposition::init_store",
        "graph/model.rs",
        "check/execution.rs",
        "api/snapshot.rs",
    ] {
        assert!(PIPELINE.contains(current_owner), "pipeline omits current owner {current_owner}");
    }
}

#[test]
fn removed_monoliths_and_legacy_services_do_not_return_to_status_docs() {
    for stale in [
        "crates/athanor-app/src/graph.rs",
        "crates/athanor-app/src/check.rs",
        "crates/athanor-app/src/api.rs",
        "Legacy library entry points remain",
        "Coordinating that canonical publication with the separate index-state and read-model publications remains a planned app-layer transaction boundary",
        "- `check_project`:",
        "- `snapshot_api_contract`:",
    ] {
        assert!(!ROADMAP.contains(stale), "roadmap contains stale architecture claim {stale}");
        assert!(!PIPELINE.contains(stale), "pipeline contains stale architecture claim {stale}");
    }
}

#[test]
fn architecture_status_documents_remain_bounded() {
    for (name, source, max_lines) in [
        ("roadmap", ROADMAP, 240),
        ("pipeline", PIPELINE, 380),
    ] {
        let lines = source.lines().count();
        assert!(lines <= max_lines, "{name} grew to {lines} lines");
    }
}
