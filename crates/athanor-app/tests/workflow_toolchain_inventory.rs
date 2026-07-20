const WORKSPACE_MANIFEST: &str = include_str!("../../../Cargo.toml");
const SETUP_RUST: &str = include_str!("../../../.github/actions/setup-rust/action.yml");
const CI_WORKFLOW: &str = include_str!("../../../.github/workflows/ci.yml");
const PRODUCTION_WORKFLOW: &str = include_str!("../../../.github/workflows/production.yml");
const RELEASE_WORKFLOW: &str = include_str!("../../../.github/workflows/release.yml");

#[test]
fn workspace_msrv_has_one_repository_owned_actions_setup() {
    assert!(WORKSPACE_MANIFEST.contains("rust-version = \"1.95\""));
    for required in [
        "args=(toolchain install 1.95.0 --profile minimal --no-self-update)",
        "rustup default 1.95.0",
        "for attempt in 1 2 3",
        "COMPONENTS: ${{ inputs.components }}",
        "TARGETS: ${{ inputs.targets }}",
        "rustc --version --verbose",
        "cargo --version",
    ] {
        assert!(SETUP_RUST.contains(required), "Rust setup omits {required}");
    }
}

#[test]
fn active_workflows_use_the_local_setup_without_version_encoded_action_pins() {
    for (name, workflow, minimum_uses) in [
        ("CI", CI_WORKFLOW, 4),
        ("production", PRODUCTION_WORKFLOW, 2),
        ("release", RELEASE_WORKFLOW, 2),
    ] {
        assert!(
            !workflow.contains("dtolnay/rust-toolchain@"),
            "{name} retains a version-encoded rust-toolchain action"
        );
        let uses = workflow.matches("uses: ./.github/actions/setup-rust").count();
        assert!(
            uses >= minimum_uses,
            "{name} has only {uses} repository-owned Rust setup uses"
        );
    }
}

#[test]
fn failure_diagnostics_are_concise_and_retrievable() {
    for required in [
        "cargo-deny-0.20.2-x86_64-unknown-linux-musl.tar.gz",
        "tee \"$RUNNER_TEMP/cargo-deny.log\"",
        "name: cargo-deny-diagnostics",
        "cargo fmt --all -- --check 2>&1 | tee \"$RUNNER_TEMP/cargo-fmt.log\"",
        "name: cargo-fmt-diagnostics",
        "tee \"$RUNNER_TEMP/feature-check.log\"",
        "name: default-feature-diagnostics",
        "failure() && steps.cargo-deny.outcome == 'failure'",
        "failure() && steps.format.outcome == 'failure' && runner.os == 'Linux'",
        "failure() && steps.feature-check.outcome == 'failure' && matrix.features == 'default'",
        "retention-days: 14",
    ] {
        assert!(CI_WORKFLOW.contains(required), "CI omits {required}");
    }
    assert!(!CI_WORKFLOW.contains("EmbarkStudios/cargo-deny-action@"));
}

#[test]
fn workflow_toolchain_owners_remain_bounded() {
    for (name, source, max_lines) in [
        ("Rust setup", SETUP_RUST, 60),
        ("CI", CI_WORKFLOW, 330),
        ("production", PRODUCTION_WORKFLOW, 110),
        ("release", RELEASE_WORKFLOW, 190),
    ] {
        let lines = source.lines().count();
        assert!(lines <= max_lines, "{name} grew to {lines} lines");
    }
}
