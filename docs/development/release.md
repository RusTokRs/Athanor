---
id: doc://docs/development/release.md
kind: developer_guide
language: en
source_language: en
status: active
---
# Release Procedure

This runbook defines the repository-owned path from a verified `main` commit to a published Athanor
release. A workflow definition or a locally built archive is implementation evidence only; publication
requires one exact tag workflow whose contract, build, SBOM, verification, and publish jobs succeed.

## Supported Release Artifacts

The release workflow publishes:

- `athanor-x86_64-unknown-linux-gnu.tar.gz` and its checksum and Sigstore bundles;
- `athanor-x86_64-pc-windows-msvc.zip` and its checksum and Sigstore bundles;
- `athanor-workspace-cyclonedx.tar.gz` and its checksum and Sigstore bundles.

Binary archives contain `ath`, `athd`, `README.md`, `CHANGELOG.md`, `LICENSE`, the platform installer,
and an internal `SHA256SUMS` file for the binaries.

## Preconditions

Before creating a release tag:

1. choose one candidate commit on `main`;
2. confirm `athanor/verification-matrix`, `athanor/appsec`, and `athanor/store-conformance` succeeded on
   that exact commit;
3. confirm the candidate has no known Product Gate or installation regression;
4. set the same Semantic Version in `apps/ath/Cargo.toml` and `apps/athd/Cargo.toml`;
5. keep exactly one matching version section in `CHANGELOG.md` and freeze it with a valid ISO calendar
   date (`YYYY-MM-DD`);
6. include at least one substantive non-heading release note in that version section;
7. keep the top `## [Unreleased]` section for changes after the candidate.

The tag must be exactly `v<package.version>`. The release contract rejects missing `v` prefixes,
non-Semantic-Version tags, package-version divergence, missing or duplicate changelog sections, undated
version sections, malformed or impossible calendar dates such as `2026-02-31`, empty release notes, and
sections that contain Markdown headings but no substantive note content.

## Publish

Create the tag only after the version/changelog commit has passed the required `main` checks:

```bash
git switch main
git pull --ff-only
git tag -a v0.1.0 -m "Athanor v0.1.0"
git push origin v0.1.0
```

Replace `0.1.0` with the prepared package version. Do not move or reuse a published release tag.

The tag workflow then:

1. verifies the tag, both binary package versions, and the dated changelog section;
2. extracts that changelog section into `release-notes.md`;
3. builds and packages Linux and Windows binaries;
4. generates the workspace CycloneDX SBOM;
5. signs archives and checksums and creates provenance attestations;
6. verifies checksums, signatures, provenance, SBOM, and release notes;
7. publishes only the supported artifact set with the extracted changelog notes.

## Post-Publish Verification

After the workflow succeeds:

- confirm the GitHub Release points to the intended tag and commit;
- download both binary archives and the SBOM archive;
- verify the outer checksum files;
- verify Sigstore bundles and GitHub provenance attestations;
- inspect the archive contents and internal binary `SHA256SUMS`;
- run one clean Linux installation smoke and one clean Windows installation smoke;
- record the tag, commit, workflow run, and any follow-up issue in the implementation plan.

## Failure And Recovery

The publish job depends on successful contract and artifact verification, so a failed earlier job must
not create a release. Fix the source problem on `main`, prepare a new version when a tag was already
published, and run the complete process again. Never replace assets under an existing immutable release
claim without an explicit incident record.
