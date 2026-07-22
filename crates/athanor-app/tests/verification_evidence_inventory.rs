use std::fs;
use std::path::Path;

use athanor_app::{AUTOMATION_JSON_CONTRACTS, VERIFICATION_EVIDENCE_SCHEMA_V1};
use serde_json::Value;

const APPSEC_WORKFLOW: &str = include_str!("../../../.github/workflows/appsec.yml");
const CI_WORKFLOW: &str = include_str!("../../../.github/workflows/ci.yml");
const EVIDENCE_WORKFLOW: &str =
    include_str!("../../../.github/workflows/verification-evidence.yml");
const STORE_CONFORMANCE_WORKFLOW: &str =
    include_str!("../../../.github/workflows/store-conformance.yml");
const CI_GUIDE: &str = include_str!("../../../docs/development/ci.md");
const PLAN: &str = include_str!("../../../athanor_implementation_plan_ru.md");

#[test]
fn successful_main_ci_is_the_only_evidence_publisher() {
    let normalized_workflow = EVIDENCE_WORKFLOW.replace("\r\n", "\n");
    for required in [
        "workflow_run:",
        "workflows: [\"CI\"]",
        "types: [completed]",
        "permissions:\n  contents: write",
        "github.event.workflow_run.conclusion == 'success'",
        "github.event.workflow_run.event == 'push'",
        "github.event.workflow_run.head_branch == 'main'",
        "github.event.workflow_run.head_sha",
        "github.event.workflow_run.id",
        "github.event.workflow_run.html_url",
        VERIFICATION_EVIDENCE_SCHEMA_V1,
        "docs/development/verification-evidence.json",
    ] {
        assert!(
            normalized_workflow.contains(required),
            "verification evidence workflow omits {required}"
        );
    }

    for forbidden in [
        "pull_request_target",
        "workflow_dispatch:",
        "conclusion != 'failure'",
    ] {
        assert!(
            !normalized_workflow.contains(forbidden),
            "verification evidence workflow contains unsafe trigger/condition {forbidden}"
        );
    }
}

#[test]
fn automation_registry_owns_the_evidence_document() {
    assert_eq!(AUTOMATION_JSON_CONTRACTS.len(), 1);
    let contract = AUTOMATION_JSON_CONTRACTS[0];
    assert_eq!(contract.schema, VERIFICATION_EVIDENCE_SCHEMA_V1);
    assert_eq!(
        contract.owner,
        ".github/workflows/verification-evidence.yml"
    );
    for field in [
        "schema",
        "workflow",
        "head_sha",
        "run_id",
        "run_url",
        "conclusion",
        "completed_at",
        "matrix",
    ] {
        assert!(
            contract.required_fields.contains(&field),
            "evidence contract omits required field {field}"
        );
    }
}

#[test]
fn evidence_only_commit_cannot_create_a_ci_loop() {
    assert!(CI_WORKFLOW.contains("paths-ignore:"));
    assert!(CI_WORKFLOW.contains("docs/development/verification-evidence.json"));
    assert!(EVIDENCE_WORKFLOW.contains("git add docs/development/verification-evidence.json"));
    assert!(EVIDENCE_WORKFLOW.contains("chore(verification): record CI evidence [skip ci]"));
    assert!(!EVIDENCE_WORKFLOW.contains("git add ."));
}

#[test]
fn ci_publishes_exact_status_after_all_matrix_jobs() {
    for required in [
        "verification-status:",
        "if: ${{ always() && github.event_name == 'push' && github.ref == 'refs/heads/main' }}",
        "needs: [security, quality, feature-matrix, coverage]",
        "statuses: write",
        "SECURITY_RESULT: ${{ needs.security.result }}",
        "QUALITY_RESULT: ${{ needs.quality.result }}",
        "FEATURE_RESULT: ${{ needs.feature-matrix.result }}",
        "COVERAGE_RESULT: ${{ needs.coverage.result }}",
        "context\": \"athanor/verification-matrix",
        "$GITHUB_API_URL/repos/$GITHUB_REPOSITORY/statuses/$GITHUB_SHA",
    ] {
        assert!(
            CI_WORKFLOW.contains(required),
            "CI exact status owner omits {required}"
        );
    }
    assert!(CI_WORKFLOW.contains("Athanor verification matrix passed"));
    assert!(CI_WORKFLOW.contains("Athanor verification matrix failed"));
}

#[test]
fn appsec_and_store_publish_exact_status_after_required_jobs() {
    for required in [
        "exact-status:",
        "name: Exact AppSec status",
        "if: ${{ always() && github.event_name == 'push' && github.ref == 'refs/heads/main' }}",
        "needs: [dependency-review, codeql, secrets, workflow-audit]",
        "statuses: write",
        "DEPENDENCY_RESULT: ${{ needs.dependency-review.result }}",
        "CODEQL_RESULT: ${{ needs.codeql.result }}",
        "SECRETS_RESULT: ${{ needs.secrets.result }}",
        "WORKFLOW_AUDIT_RESULT: ${{ needs.workflow-audit.result }}",
        "context\": \"athanor/appsec",
        "Athanor AppSec checks passed",
        "Athanor AppSec checks failed",
        "$GITHUB_API_URL/repos/$GITHUB_REPOSITORY/statuses/$GITHUB_SHA",
    ] {
        assert!(
            APPSEC_WORKFLOW.contains(required),
            "AppSec exact status owner omits {required}"
        );
    }
    assert!(APPSEC_WORKFLOW.contains(
        "$DEPENDENCY_RESULT\" != \"success\" && \"$DEPENDENCY_RESULT\" != \"skipped\""
    ));

    for required in [
        "exact-status:",
        "name: Exact Store Conformance status",
        "if: ${{ always() && github.event_name == 'push' && github.ref == 'refs/heads/main' }}",
        "needs: [backend, surrealdb-remote]",
        "statuses: write",
        "BACKEND_RESULT: ${{ needs.backend.result }}",
        "REMOTE_RESULT: ${{ needs.surrealdb-remote.result }}",
        "context\": \"athanor/store-conformance",
        "Athanor store conformance passed",
        "Athanor store conformance failed",
        "$GITHUB_API_URL/repos/$GITHUB_REPOSITORY/statuses/$GITHUB_SHA",
    ] {
        assert!(
            STORE_CONFORMANCE_WORKFLOW.contains(required),
            "Store Conformance exact status owner omits {required}"
        );
    }
}

#[test]
fn workflow_records_the_matrix_claimed_by_the_ci_guide() {
    for command in [
        "cargo-deny check",
        "cargo fmt --all -- --check",
        "cargo test --workspace --quiet --locked",
        "cargo clippy --workspace --all-targets --locked -- -D warnings",
        "cargo run -p ath --quiet --locked -- index .",
        "cargo run -p ath --quiet --locked -- docs check",
        "feature matrix: default, store-surreal, js-ts-precision, all-features",
        "source coverage baseline",
    ] {
        assert!(
            EVIDENCE_WORKFLOW.contains(command),
            "evidence matrix omits {command}"
        );
    }

    for invariant in [
        "Workflow YAML is implementation evidence, not execution evidence.",
        VERIFICATION_EVIDENCE_SCHEMA_V1,
        "the exact CI `head_sha`",
        "Only successful `push` runs whose `head_branch` is `main`",
        "athanor/verification-matrix",
        "legacy commit status",
        "implemented, not verified",
        "athanor/appsec",
        "athanor/store-conformance",
    ] {
        assert!(CI_GUIDE.contains(invariant), "CI guide omits {invariant}");
    }
}

#[test]
fn optional_recorded_evidence_is_strictly_validated() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("athanor-app must live under <workspace>/crates");
    let path = workspace.join("docs/development/verification-evidence.json");

    if !path.exists() {
        assert!(
            PLAN.contains("| `VERIFY-001` | P1 | `[!] blocked` |"),
            "missing evidence requires blocked VERIFY-001 status"
        );
        return;
    }

    let bytes = fs::read(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    let payload: Value = serde_json::from_slice(&bytes).unwrap_or_else(|error| {
        panic!("invalid verification evidence {}: {error}", path.display())
    });

    assert_eq!(
        payload.get("schema").and_then(Value::as_str),
        Some(VERIFICATION_EVIDENCE_SCHEMA_V1)
    );
    assert_eq!(payload.get("workflow").and_then(Value::as_str), Some("CI"));
    assert_eq!(
        payload.get("conclusion").and_then(Value::as_str),
        Some("success")
    );

    let head_sha = payload
        .get("head_sha")
        .and_then(Value::as_str)
        .expect("verification evidence head_sha");
    assert_eq!(head_sha.len(), 40);
    assert!(
        head_sha
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    );

    let run_id = payload.get("run_id").and_then(Value::as_u64);
    assert!(
        run_id.is_some_and(|id| id > 0),
        "verification evidence requires a positive workflow run id"
    );

    let run_url = payload.get("run_url").and_then(Value::as_str);
    assert!(
        run_url.is_some_and(|url| {
            url.starts_with("https://github.com/RusTokRs/Athanor/actions/runs/")
        }),
        "verification evidence requires the canonical repository run URL"
    );

    let completed_at = payload.get("completed_at").and_then(Value::as_str);
    assert!(
        completed_at.is_some_and(|timestamp| !timestamp.trim().is_empty()),
        "verification evidence requires completion time"
    );

    let matrix = payload
        .get("matrix")
        .and_then(Value::as_array)
        .expect("verification evidence matrix");
    assert!(matrix.len() >= 8);
}

#[test]
fn verification_evidence_owners_remain_bounded() {
    for (name, source, max_lines) in [
        ("CI workflow", CI_WORKFLOW, 330),
        ("evidence workflow", EVIDENCE_WORKFLOW, 100),
        ("CI guide", CI_GUIDE, 240),
    ] {
        let lines = source.lines().count();
        assert!(lines <= max_lines, "{name} grew to {lines} lines");
    }
}
