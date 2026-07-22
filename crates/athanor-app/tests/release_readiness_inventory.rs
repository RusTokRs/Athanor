const RELEASE_WORKFLOW: &str = include_str!("../../../.github/workflows/release.yml");
const RELEASE_GUARD: &str = include_str!("../../../scripts/verify_release_version.py");
const CHANGELOG: &str = include_str!("../../../CHANGELOG.md");
const ATH_MANIFEST: &str = include_str!("../../../apps/ath/Cargo.toml");
const ATHD_MANIFEST: &str = include_str!("../../../apps/athd/Cargo.toml");

fn package_version(manifest: &str) -> &str {
    manifest
        .lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix("version = \"")
                .and_then(|value| value.strip_suffix('"'))
        })
        .expect("release package manifest must define a direct version")
}

#[test]
fn release_workflow_gates_all_artifact_jobs_on_the_tag_contract() {
    for required in [
        "release-contract:",
        "name: Verify release contract",
        "RELEASE_TAG: ${{ github.ref_name }}",
        "python3 scripts/verify_release_version.py",
        "--tag \"$RELEASE_TAG\"",
        "apps/ath/Cargo.toml",
        "apps/athd/Cargo.toml",
        "cp target/${{ matrix.target }}/release/ath target/${{ matrix.target }}/release/athd README.md CHANGELOG.md LICENSE install.sh",
        "README.md,CHANGELOG.md,LICENSE,install.ps1",
    ] {
        assert!(
            RELEASE_WORKFLOW.contains(required),
            "release workflow omits {required}"
        );
    }

    assert_eq!(
        RELEASE_WORKFLOW.matches("needs: release-contract").count(),
        2,
        "both build and SBOM jobs must wait for the release contract"
    );
    assert!(RELEASE_WORKFLOW.contains("needs: [build, sbom]"));
    assert!(RELEASE_WORKFLOW.contains("needs: verify"));
}

#[test]
fn release_guard_fails_closed_on_invalid_or_mismatched_versions() {
    for required in [
        "SEMVER = re.compile(",
        "release tag must start with 'v'",
        "release tag is not v<semver>",
        "does not define package.version",
        "has non-semver package.version",
        "does not match release packages",
        "release package versions disagree",
        "return 1",
    ] {
        assert!(RELEASE_GUARD.contains(required), "release guard omits {required}");
    }

    assert!(!RELEASE_GUARD.contains("except Exception"));
    assert!(!RELEASE_GUARD.contains("return 0\n    except"));
}

#[test]
fn release_packages_and_changelog_share_the_current_version() {
    let ath_version = package_version(ATH_MANIFEST);
    let athd_version = package_version(ATHD_MANIFEST);
    assert_eq!(ath_version, athd_version, "release package versions diverged");

    let heading = format!("## [{ath_version}] - Unreleased");
    assert!(
        CHANGELOG.contains(&heading),
        "changelog omits current release section {heading}"
    );
    assert!(CHANGELOG.contains("## [Unreleased]"));
    assert!(CHANGELOG.contains("Semantic Versioning"));
    assert!(CHANGELOG.contains("Sigstore"));
    assert!(CHANGELOG.contains("CycloneDX SBOMs"));
}

#[test]
fn release_readiness_owners_remain_bounded() {
    for (name, source, max_lines) in [
        ("release workflow", RELEASE_WORKFLOW, 220),
        ("release guard", RELEASE_GUARD, 120),
        ("changelog", CHANGELOG, 120),
    ] {
        let lines = source.lines().count();
        assert!(lines <= max_lines, "{name} grew to {lines} lines");
    }
}
