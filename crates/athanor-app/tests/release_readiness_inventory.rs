const RELEASE_WORKFLOW: &str = include_str!("../../../.github/workflows/release.yml");
const RELEASE_GUARD: &str = include_str!("../../../scripts/verify_release_version.py");
const RELEASE_GUIDE: &str = include_str!("../../../docs/development/release.md");
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

fn current_release_date<'a>(changelog: &'a str, version: &str) -> &'a str {
    let prefix = format!("## [{version}] - ");
    let heading = changelog
        .lines()
        .find(|line| line.starts_with(&prefix))
        .unwrap_or_else(|| panic!("changelog omits current release heading {prefix}<date>"));
    heading
        .strip_prefix(&prefix)
        .expect("matched release heading must preserve its prefix")
}

fn is_iso_release_date(value: &str) -> bool {
    value.len() == 10
        && value.bytes().enumerate().all(|(index, byte)| match index {
            4 | 7 => byte == b'-',
            _ => byte.is_ascii_digit(),
        })
}

#[test]
fn release_workflow_gates_all_artifact_jobs_on_the_tag_contract() {
    for required in [
        "workflow_dispatch:",
        "release-contract:",
        "name: Verify release contract",
        "RELEASE_TAG: ${{ github.ref_name }}",
        "python3 scripts/verify_release_version.py",
        "--tag \"$RELEASE_TAG\"",
        "--changelog CHANGELOG.md",
        "--notes-output dist/release-notes.md",
        "apps/ath/Cargo.toml",
        "apps/athd/Cargo.toml",
        "name: release-contract",
        "path: dist/release-notes.md",
        "cp target/${{ matrix.target }}/release/ath target/${{ matrix.target }}/release/athd README.md CHANGELOG.md LICENSE install.sh",
        "README.md,CHANGELOG.md,LICENSE,install.ps1",
        "test -s release-notes.md",
        "body_path: dist/release-notes.md",
        "dist/athanor-x86_64-unknown-linux-gnu.tar.gz*",
        "dist/athanor-x86_64-pc-windows-msvc.zip*",
        "dist/athanor-workspace-cyclonedx.tar.gz*",
        "generate_release_notes: false",
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
    assert!(!RELEASE_WORKFLOW.contains("files: dist/*"));
}

#[test]
fn release_guard_fails_closed_on_invalid_or_mismatched_versions() {
    for required in [
        "from datetime import date",
        "SEMVER = re.compile(",
        "RELEASE_DATE = re.compile(",
        "section_heading = re.compile(",
        "def has_substantive_release_notes",
        "fence: tuple[str, int] | None",
        "in_comment = False",
        "remaining.find(\"<!--\")",
        "remaining.find(\"-->\")",
        "len(set(compact)) == 1",
        "has_substantive_release_notes(note_lines)",
        "release tag must start with 'v'",
        "release tag is not v<semver>",
        "does not define package.version",
        "has non-semver package.version",
        "does not match release packages",
        "release package versions disagree",
        "omits release section",
        "defines multiple release sections",
        "must be dated before release",
        "date.fromisoformat(release_date)",
        "has invalid release date",
        "has no release notes",
        "has no substantive release notes",
        "notes_output.write_text",
        "return 1",
    ] {
        assert!(
            RELEASE_GUARD.contains(required),
            "release guard omits {required}"
        );
    }

    assert!(!RELEASE_GUARD.contains("except Exception"));
    assert!(!RELEASE_GUARD.contains("return 0\n    except"));
}

#[test]
fn release_packages_and_changelog_share_a_dated_current_version() {
    let ath_version = package_version(ATH_MANIFEST);
    let athd_version = package_version(ATHD_MANIFEST);
    assert_eq!(
        ath_version, athd_version,
        "release package versions diverged"
    );

    let release_prefix = format!("## [{ath_version}]");
    assert_eq!(
        CHANGELOG
            .lines()
            .filter(|line| line.starts_with(&release_prefix))
            .count(),
        1,
        "changelog must define the current release exactly once"
    );

    let release_date = current_release_date(CHANGELOG, ath_version);
    assert_ne!(
        release_date, "Unreleased",
        "current release section must be frozen before tagging"
    );
    assert!(
        is_iso_release_date(release_date),
        "current release date is not YYYY-MM-DD: {release_date}"
    );

    let heading = format!("## [{ath_version}] - {release_date}");
    let section = CHANGELOG
        .split_once(&heading)
        .expect("current release heading must delimit its notes")
        .1
        .split("\n## [")
        .next()
        .expect("current release notes must have a bounded section");
    assert!(
        section
            .lines()
            .any(|line| !line.trim().is_empty() && !line.trim_start().starts_with('#')),
        "current release section must contain substantive notes"
    );

    assert!(CHANGELOG.contains("## [Unreleased]"));
    assert!(CHANGELOG.contains("Semantic Versioning"));
    assert!(CHANGELOG.contains("Sigstore"));
    assert!(CHANGELOG.contains("CycloneDX SBOMs"));
}

#[test]
fn release_runbook_matches_the_enforced_workflow() {
    let normalized_guide = RELEASE_GUIDE
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    for invariant in [
        "athanor/verification-matrix",
        "athanor/appsec",
        "athanor/store-conformance",
        "v<package.version>",
        "valid ISO calendar date",
        "exactly one matching version section",
        "HTML comments, thematic breaks, empty list markers, or empty fenced code blocks",
        "release-notes.md",
        "CycloneDX SBOM",
        "Do not move or reuse a published release tag",
        "dispatch ref must be the tag itself",
        "Never replace assets",
    ] {
        assert!(
            normalized_guide.contains(invariant),
            "release guide omits {invariant}"
        );
    }
}

#[test]
fn release_readiness_owners_remain_bounded() {
    for (name, source, max_lines) in [
        ("release workflow", RELEASE_WORKFLOW, 230),
        ("release guard", RELEASE_GUARD, 220),
        ("release guide", RELEASE_GUIDE, 180),
        ("changelog", CHANGELOG, 120),
    ] {
        let lines = source.lines().count();
        assert!(lines <= max_lines, "{name} grew to {lines} lines");
    }
}
