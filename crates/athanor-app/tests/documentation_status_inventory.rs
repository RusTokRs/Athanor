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
fn aggregate_status_documents_separate_source_and_execution_evidence() {
    for (name, source) in [
        ("documentation index", DOCS_INDEX),
        ("roadmap", ROADMAP),
        ("pipeline", PIPELINE),
    ] {
        assert!(source.contains("status: active"), "{name} must remain active");
        assert!(!source.contains("status: verified"));
        assert!(!source.contains("last_verified_snapshot:"));
        assert!(
            source.contains("execution evidence")
                || source.contains("Implemented")
                || source.contains("implementation evidence"),
            "{name} must separate source state from execution evidence"
        );
    }
}

#[test]
fn documentation_entrypoint_routes_to_current_owners_and_commands() {
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
        assert!(DOCS_INDEX.contains(target), "documentation map omits {target}");
    }
    for command in [
        "ath docs generate-architecture . --snapshot <EXACT-COMMITTED-SNAPSHOT>",
        "ath docs architecture current .",
        "ath docs architecture manifest . --json",
        "ath docs architecture validation . --json",
        "documentation_architecture_inspection_inventory",
        "documentation_architecture_cli",
    ] {
        assert!(DOCS_INDEX.contains(command), "documentation map omits {command}");
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
fn implementation_plan_and_roadmap_match_slice_1c_status() {
    for invariant in [
        "### 4.1 `DOCGEN-001` — evidence-backed documentation generation",
        "| `DOCGEN-001` | P2 | `[-] in progress` |",
        "Slice 1C1",
        "Slice 1C2",
        "4f567271ed6d38d30b3c15dc6999aa33152a9312",
        "30015689753",
        "30015691399",
        "30015689363",
        "cargo test -p ath --test documentation_architecture_cli --locked",
    ] {
        assert!(PLAN.contains(invariant), "implementation plan omits {invariant}");
    }
    for invariant in [
        "## Active Work",
        "### `DOCGEN-001`",
        "Slice 1C1",
        "Slice 1C2 implemented",
        "exact matrix pending",
        "ath docs generate-architecture",
    ] {
        assert!(ROADMAP.contains(invariant), "roadmap omits {invariant}");
    }
    assert!(!PLAN.contains("`DOCGEN-001` | P2 | `[x] verified`"));
}

#[test]
fn documentation_generation_plan_matches_current_boundaries() {
    for invariant in [
        "# Evidence-Backed Documentation Generation Plan",
        "exact committed snapshot",
        "athanor.documentation_generation_request.v1",
        "athanor.documentation_generation_manifest.v1",
        "athanor.documentation_outline.v1",
        "athanor.documentation_context.v1",
        "athanor.documentation_citation.v1",
        "athanor.documentation_draft.v1",
        "athanor.documentation_validation_report.v1",
        "athanor.documentation_current.v1",
        "Slice 1C1",
        "Slice 1C2",
        "ath docs generate-architecture",
        "ath docs architecture current",
        "The existing coordinated `ath generate` command is unchanged",
    ] {
        assert!(DOCGEN_PLAN.contains(invariant), "docgen plan omits {invariant}");
    }
}

#[test]
fn documentation_json_inventory_matches_publication_boundaries() {
    for invariant in [
        "`NON_PUBLIC_JSON_CONTRACTS` contains 32 descriptors",
        "26 current documents",
        "athanor.documentation_current.v1",
        "CurrentDocumentationGeneration",
        "athanor.documentation_validation_report.v1",
        "four intermediate documentation types",
        "documentation_architecture_publication_inventory",
    ] {
        assert!(JSON_INVENTORY.contains(invariant), "JSON inventory omits {invariant}");
    }
}

#[test]
fn release_and_mcp_status_owners_remain_current() {
    for invariant in [
        "# Release Procedure",
        "athanor/verification-matrix",
        "athanor/appsec",
        "athanor/store-conformance",
        "CHANGELOG.md",
        "CycloneDX SBOM",
        "Never replace assets",
    ] {
        assert!(RELEASE_GUIDE.contains(invariant));
    }
    for invariant in [
        "## MCP Request And Control-Plane Lifecycle",
        "### Control Input Priority",
        "### Saturated Response Queue",
        "### Disconnect",
        "nonblocking admission",
    ] {
        assert!(OPERATION_CONTEXT.contains(invariant));
    }
}

#[test]
fn removed_monoliths_and_false_surfaces_do_not_return() {
    for stale in [
        "crates/athanor-app/src/graph.rs",
        "crates/athanor-app/src/check.rs",
        "crates/athanor-app/src/api.rs",
        "Legacy library entry points remain",
        "latest snapshot fallback",
    ] {
        for (name, source) in [
            ("documentation index", DOCS_INDEX),
            ("roadmap", ROADMAP),
            ("pipeline", PIPELINE),
        ] {
            assert!(!source.contains(stale), "{name} contains stale claim {stale}");
        }
    }
    assert!(DOCS_INDEX.contains("has no latest fallback"));
    assert!(ROADMAP.contains("existing coordinated `ath generate` command is unchanged"));
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
