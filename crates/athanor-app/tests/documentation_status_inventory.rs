const DOCS_INDEX: &str = include_str!("../../../docs/README.md");
const ROADMAP: &str = include_str!("../../../docs/development/roadmap-status.md");
const PIPELINE: &str = include_str!("../../../docs/architecture/pipeline.md");
const PLAN: &str = include_str!("../../../athanor_implementation_plan_ru.md");

#[test]
fn aggregate_status_documents_do_not_claim_unexecuted_verification() {
    for (name, source) in [
        ("documentation index", DOCS_INDEX),
        ("roadmap", ROADMAP),
        ("pipeline", PIPELINE),
    ] {
        assert!(
            source.contains("status: active"),
            "{name} must remain an active status document"
        );
        assert!(
            !source.contains("status: verified"),
            "{name} must not use snapshot verification as current-commit evidence"
        );
        assert!(
            !source.contains("last_verified_snapshot:"),
            "{name} must not carry stale snapshot verification metadata"
        );
        assert!(
            !source.contains("Status: verified."),
            "{name} must not contain aggregate verified claims without one-commit execution evidence"
        );
        assert!(
            source.contains("implementation evidence")
                || source.contains("Implemented means")
                || source.contains("implemented, not verified"),
            "{name} must distinguish implementation from execution evidence"
        );
    }
}

#[test]
fn documentation_entrypoint_routes_to_current_status_owners() {
    for target in [
        "development/roadmap-status.md",
        "architecture/pipeline.md",
        "../athanor_implementation_plan_ru.md",
        "development/json-contract-inventory.md",
        "development/legacy-runtime-compatibility.md",
        "development/direct-operation-context.md",
    ] {
        assert!(DOCS_INDEX.contains(target), "documentation map omits {target}");
    }
    assert!(!DOCS_INDEX.contains("current verified implementation status"));
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
        assert!(
            PIPELINE.contains(current_owner),
            "pipeline omits current owner {current_owner}"
        );
    }
}

#[test]
fn implementation_plan_matches_documentation_status() {
    for completed in [
        "### 3.4 `DOC-001` / `DOC-002` — documentation status hygiene",
        "| `DOC-001` | P3 | `[x] implemented` |",
        "| `DOC-002` | P3 | `[x] implemented` |",
        "cargo test -p athanor-app --test documentation_status_inventory --locked",
    ] {
        assert!(PLAN.contains(completed), "plan is missing {completed}");
    }
    assert!(PLAN.contains("### 4.1 `MCP-004` — control-plane responsiveness"));
    assert!(ROADMAP.contains("### `DOC-001` / `DOC-002`"));
    assert!(ROADMAP.contains("### `MCP-004`"));

    let docs_package = ROADMAP
        .find("### `DOC-001` / `DOC-002`")
        .expect("roadmap docs package");
    let active_work = ROADMAP.find("## Active Work").expect("roadmap active work");
    assert!(docs_package < active_work, "completed docs package remains active");
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
        for (name, source) in [
            ("documentation index", DOCS_INDEX),
            ("roadmap", ROADMAP),
            ("pipeline", PIPELINE),
        ] {
            assert!(!source.contains(stale), "{name} contains stale claim {stale}");
        }
    }
}

#[test]
fn architecture_status_documents_remain_bounded() {
    for (name, source, max_lines) in [
        ("documentation index", DOCS_INDEX, 220),
        ("roadmap", ROADMAP, 240),
        ("pipeline", PIPELINE, 380),
        ("implementation plan", PLAN, 320),
    ] {
        let lines = source.lines().count();
        assert!(lines <= max_lines, "{name} grew to {lines} lines");
    }
}
