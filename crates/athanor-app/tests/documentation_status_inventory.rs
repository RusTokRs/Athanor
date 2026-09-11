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

const SLICE_1C_SHA: &str = "042d02ac6b4c89d90a5b76c818098eb0c6b41920";
const SLICE_1C_CI: &str = "30025932615";
const SLICE_1C_APPSEC: &str = "30025931953";
const SLICE_1C_STORE: &str = "30025932704";
const RUSTOK_EVALUATION_SHA: &str = "5e0b28099c48e22bdc172fa57b6d51db9e6efb7b";
const RUSTOK_EVALUATION_RUN: &str = "30029451096";
const RUSTOK_PROBE_SHA: &str = "12a8687c5d098ab05a5988508816aad5f0dc3e23";
const RUSTOK_PROBE_RUN: &str = "30030131126";
const RUSTOK_CITATION_FAILURE: &str =
    "documentation draft citations must contain between 1 and 256 entries";

#[test]
fn status_documents_are_active_and_separate_source_from_execution_state() {
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
                || source.contains("does not prove")
                || source.contains("Exact package evidence"),
            "{name} must distinguish source state from execution evidence"
        );
    }
}

#[test]
fn documentation_map_keeps_current_owners_and_cli_entrypoints() {
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
fn pipeline_retains_explicit_architecture_boundaries_and_owners() {
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
        "RuntimeComposition::init_store",
    ] {
        assert!(PIPELINE.contains(owner), "pipeline omits {owner}");
    }
}

#[test]
fn implementation_and_roadmap_record_slice_1c_execution_evidence() {
    for invariant in [
        "Slice 1C1",
        "Slice 1C2",
        SLICE_1C_SHA,
        SLICE_1C_CI,
        SLICE_1C_APPSEC,
        SLICE_1C_STORE,
        "cargo test -p ath --test documentation_architecture_cli --locked",
    ] {
        assert!(PLAN.contains(invariant), "implementation plan omits {invariant}");
    }
    for invariant in [
        "Slice 1C1",
        "Slice 1C2",
        "30025932615",
        "30025931953",
        "30025932704",
        "30029451096",
        "citation budgeting",
    ] {
        assert!(ROADMAP.contains(invariant), "roadmap omits {invariant}");
    }
    assert!(
        ROADMAP.contains(SLICE_1C_SHA) || ROADMAP.contains("042d02ac…"),
        "roadmap omits Slice 1C2 source evidence"
    );
}

#[test]
fn rustok_failure_and_repair_evidence_are_recorded_by_the_correct_owners() {
    assert!(PLAN.contains(RUSTOK_EVALUATION_SHA));
    assert!(PLAN.contains(RUSTOK_EVALUATION_RUN));
    assert!(PLAN.contains(RUSTOK_CITATION_FAILURE));
    assert!(PLAN.contains(RUSTOK_PROBE_SHA));
    assert!(PLAN.contains(RUSTOK_PROBE_RUN));

    assert!(ROADMAP.contains(RUSTOK_EVALUATION_RUN));
    assert!(ROADMAP.contains("citation budgeting"));

    assert!(DOCGEN_PLAN.contains(RUSTOK_EVALUATION_RUN));
    assert!(DOCGEN_PLAN.contains("First Rustok failure"));
    assert!(DOCGEN_PLAN.contains("repaired evaluation"));
    assert!(DOCGEN_PLAN.contains("workflow_dispatch"));
    assert!(DOCGEN_PLAN.contains("DOCUMENTATION_REFERENCE_LIMIT"));
}

#[test]
fn docgen_plan_uses_evidence_shape_not_exact_prose() {
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
        "Architecture baselines: Slices 0A–0B",
        "1C1 `4f567271…`",
        "1C2 `042d02ac…`",
        "ath docs generate-architecture",
        "ath docs architecture current",
        "The existing coordinated `ath generate` command is",
    ] {
        assert!(DOCGEN_PLAN.contains(invariant), "docgen plan omits {invariant}");
    }
    assert!(DOCGEN_PLAN.contains("First Rustok failure"));
    assert!(DOCGEN_PLAN.contains("repaired evaluation"));
    assert!(DOCGEN_PLAN.contains("Completeness progression"));
}

#[test]
fn json_release_and_operation_status_owners_remain_present() {
    for invariant in [
        "`NON_PUBLIC_JSON_CONTRACTS` contains 32 descriptors",
        "26 current documents",
        "athanor.documentation_current.v1",
        "CurrentDocumentationGeneration",
        "athanor.documentation_validation_report.v1",
        "four intermediate documentation types",
    ] {
        assert!(JSON_INVENTORY.contains(invariant), "JSON inventory omits {invariant}");
    }
    for invariant in [
        "# Release Procedure",
        "athanor/verification-matrix",
        "athanor/appsec",
        "athanor/store-conformance",
        "CHANGELOG.md",
        "CycloneDX SBOM",
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
    assert!(ROADMAP.contains("coordinated `ath generate`"));
    assert!(DOCGEN_PLAN.contains("coordinated `ath generate`"));
}

#[test]
fn documentation_sources_stay_bounded() {
    for (name, source, max_lines) in [
        ("documentation index", DOCS_INDEX, 225),
        ("roadmap", ROADMAP, 220),
        ("pipeline", PIPELINE, 380),
        ("operation context", OPERATION_CONTEXT, 260),
        ("release guide", RELEASE_GUIDE, 180),
        ("documentation generation plan", DOCGEN_PLAN, 320),
        ("implementation plan", PLAN, 320),
    ] {
        assert!(
            source.lines().count() <= max_lines,
            "{name} exceeded bounded documentation size"
        );
    }
}
