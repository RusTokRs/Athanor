const DOCS_INDEX: &str = include_str!("../../../docs/README.md");
const ROADMAP: &str = include_str!("../../../docs/development/roadmap-status.md");
const PIPELINE: &str = include_str!("../../../docs/architecture/pipeline.md");
const OPERATION_CONTEXT: &str =
    include_str!("../../../docs/development/direct-operation-context.md");
const RELEASE_GUIDE: &str = include_str!("../../../docs/development/release.md");
const JSON_INVENTORY: &str = include_str!("../../../docs/development/json-contract-inventory.md");
const DOCGEN_PLAN: &str =
    include_str!("../../../docs/development/evidence-backed-documentation-generation-plan.md");
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
            "{name} must remain active"
        );
        assert!(!source.contains("status: verified"));
        assert!(!source.contains("last_verified_snapshot:"));
        assert!(!source.contains("Status: verified."));
        assert!(
            source.contains("implementation evidence")
                || source.contains("Implemented means")
                || source.contains("implemented, not verified")
                || source.contains("execution evidence"),
            "{name} must distinguish source state from execution evidence"
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
        "development/release.md",
        "development/evidence-backed-documentation-generation-plan.md",
    ] {
        assert!(
            DOCS_INDEX.contains(target),
            "documentation map omits {target}"
        );
    }
    for command in [
        "cargo test -p athanor-app --test release_readiness_inventory --locked",
        "cargo test -p athanor-app --test documentation_generation_contract_inventory --locked",
        "cargo test -p athanor-app --test documentation_generation_slice0b_inventory --locked",
        "cargo test -p athanor-app --test documentation_architecture_profile_inventory --locked",
        "cargo test -p athanor-app --test documentation_architecture_publication_inventory --locked",
    ] {
        assert!(
            DOCS_INDEX.contains(command),
            "documentation map omits {command}"
        );
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
    for owner in [
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
        assert!(PIPELINE.contains(owner), "pipeline omits {owner}");
    }
}

#[test]
fn implementation_plan_matches_documentation_status() {
    for completed in [
        "### 3.4 `DOC-001` / `DOC-002` — documentation status hygiene",
        "### 3.5 `MCP-004` — control-plane responsiveness",
        "| `DOC-001` | P3 | `[x] implemented` |",
        "| `DOC-002` | P3 | `[x] implemented` |",
        "| `MCP-004` | P1 | `[x] implemented` |",
        "| `API-001` | P1 | `[x] verified` |",
        "| `REL-001` | P1 | `[x] verified` |",
        "cargo test -p athanor-app --test documentation_status_inventory --locked",
        "cargo test -p athanor-app --test release_readiness_inventory --locked",
        "cargo test -p athanor-transport-mcp --test control_plane_saturation_inventory --locked",
    ] {
        assert!(PLAN.contains(completed), "plan is missing {completed}");
    }
    assert!(PLAN.contains("### 4.1 `DOCGEN-001` — evidence-backed documentation generation"));
    assert!(PLAN.contains("### 4.2 Product backlog"));
    assert!(PLAN.contains("| `DOCGEN-001` | P2 | `[-] in progress` |"));
    assert!(!PLAN.contains("| `REL-001` | P1 | `[-] in progress` |"));

    let active_work = ROADMAP.find("## Active Work").expect("roadmap active work");
    let product_backlog = ROADMAP.find("## Product Backlog").expect("roadmap backlog");
    assert!(active_work < product_backlog);
    let active = &ROADMAP[active_work..product_backlog];
    for invariant in ["### `DOCGEN-001`", "Slice 1A", "Slice 1B", "Slice 1C"] {
        assert!(active.contains(invariant), "active work omits {invariant}");
    }
    assert!(active.contains("0cfeca8ad4dc3c0632246afa01e43372f4ec3d71"));
    assert!(!active.contains("verified on"));
}

#[test]
fn documentation_generation_plan_matches_the_bounded_slice() {
    for invariant in [
        "# Evidence-Backed Documentation Generation Plan",
        "canonical snapshot remains the only truth source",
        "athanor.documentation_generation_request.v1",
        "athanor.documentation_generation_manifest.v1",
        "athanor.documentation_outline.v1",
        "athanor.documentation_context.v1",
        "athanor.documentation_citation.v1",
        "athanor.documentation_draft.v1",
        "athanor.documentation_validation_report.v1",
        "athanor.documentation_current.v1",
        "No reference becomes a dependency",
        "Slice 1A — Implemented And Execution-Confirmed",
        "Slice 1B — Implemented And Execution-Confirmed",
        "Slice 1C — Next",
        "The existing coordinated `ath generate` command is unchanged",
    ] {
        assert!(
            DOCGEN_PLAN.contains(invariant) || PLAN.contains(invariant),
            "documentation generation status omits {invariant}"
        );
    }
    for command in [
        "documentation_architecture_profile_inventory",
        "documentation_architecture_publication_inventory",
    ] {
        assert!(PLAN.contains(command) && DOCS_INDEX.contains(command));
    }
    assert!(!PLAN.contains("`DOCGEN-001` | P2 | `[x] verified`"));
}

#[test]
fn documentation_json_inventory_matches_publication_boundaries() {
    for invariant in [
        "`NON_PUBLIC_JSON_CONTRACTS` contains 32 descriptors",
        "26 current documents",
        "athanor.documentation_current.v1",
        "CurrentDocumentationGeneration",
        "athanor.documentation_validation_report.v1",
        "Twenty canonical literals",
        "four intermediate documentation types",
        "documentation_architecture_publication_inventory",
    ] {
        assert!(
            JSON_INVENTORY.contains(invariant),
            "JSON inventory omits {invariant}"
        );
    }
    assert!(!JSON_INVENTORY.contains("`NON_PUBLIC_JSON_CONTRACTS` retains 30"));
    assert!(!JSON_INVENTORY.contains("five Slice 0B intermediate documentation contracts"));
}

#[test]
fn release_runbook_matches_the_repository_owned_contract() {
    for invariant in [
        "# Release Procedure",
        "athanor/verification-matrix",
        "athanor/appsec",
        "athanor/store-conformance",
        "apps/ath/Cargo.toml",
        "apps/athd/Cargo.toml",
        "CHANGELOG.md",
        "v<package.version>",
        "release-notes.md",
        "CycloneDX SBOM",
        "Never replace assets",
    ] {
        assert!(
            RELEASE_GUIDE.contains(invariant),
            "release guide omits {invariant}"
        );
    }
    assert!(RELEASE_GUIDE.contains("status: active"));
    assert!(!RELEASE_GUIDE.contains("status: verified"));
}

#[test]
fn mcp_control_plane_documentation_matches_implementation_status() {
    for invariant in [
        "## MCP Request And Control-Plane Lifecycle",
        "### Control Input Priority",
        "### Saturated Response Queue",
        "### Disconnect",
        "nonblocking admission",
        "Ordinary request tasks retain bounded",
        "cargo test -p athanor-transport-mcp --test control_plane_saturation_inventory --locked",
    ] {
        assert!(OPERATION_CONTEXT.contains(invariant));
    }
}

#[test]
fn removed_monoliths_and_false_docgen_surfaces_do_not_return() {
    for stale in [
        "crates/athanor-app/src/graph.rs",
        "crates/athanor-app/src/check.rs",
        "crates/athanor-app/src/api.rs",
        "Legacy library entry points remain",
        "- `check_project`:",
        "- `snapshot_api_contract`:",
    ] {
        for (name, source) in [
            ("documentation index", DOCS_INDEX),
            ("roadmap", ROADMAP),
            ("pipeline", PIPELINE),
        ] {
            assert!(
                !source.contains(stale),
                "{name} contains stale claim {stale}"
            );
        }
    }
    assert!(DOCS_INDEX.contains("supported `ath` command"));
    assert!(DOCS_INDEX.contains("Slice 1C owns that"));
    assert!(ROADMAP.contains("No CLI, daemon, MCP, provider, or store-loading entrypoint"));
}

#[test]
fn architecture_status_documents_remain_bounded() {
    for (name, source, max_lines) in [
        ("documentation index", DOCS_INDEX, 225),
        ("roadmap", ROADMAP, 220),
        ("pipeline", PIPELINE, 380),
        ("operation context", OPERATION_CONTEXT, 260),
        ("release guide", RELEASE_GUIDE, 180),
        ("documentation generation plan", DOCGEN_PLAN, 260),
        ("implementation plan", PLAN, 260),
    ] {
        let lines = source.lines().count();
        assert!(lines <= max_lines, "{name} grew to {lines} lines");
    }
}
