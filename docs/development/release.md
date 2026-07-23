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
6. include at least one substantive release note in that version section;
7. keep the top `## [Unreleased]` section for changes after the candidate;
8. fetch remote tags and confirm the intended tag name does not already exist locally or remotely.

The tag must be exactly `v<package.version>`. The release contract rejects missing `v` prefixes,
non-Semantic-Version tags, package-version divergence, missing or duplicate changelog sections, undated
version sections, malformed or impossible calendar dates such as `2026-02-31`, and sections without
substantive content. HTML comments, thematic breaks, empty list markers, or empty fenced code blocks do
not qualify as release notes; non-empty fenced content remains valid.

## Publish

Create the tag only after the version/changelog commit has passed the required `main` checks. Always
name the exact verified SHA explicitly because an evidence-recording or other `[skip ci]` commit may have
advanced `main` after the candidate completed its checks:

```bash
git switch main
git pull --ff-only
git fetch origin --tags
version="0.2.0"
candidate_sha="<verified exact main SHA>"
git merge-base --is-ancestor "$candidate_sha" origin/main
test ! git rev-parse -q --verify "refs/tags/v${version}" >/dev/null
git tag -a "v${version}" "$candidate_sha" -m "Athanor v${version}"
test "$(git rev-parse "v${version}^{}")" = "$candidate_sha"
git push origin "v${version}"
```

Replace `0.2.0` with the prepared package version and replace the SHA placeholder with the commit that
owns all three successful required statuses. Do not move or reuse a published release tag. If the name is
already occupied, prepare a new Semantic Version and repeat the complete candidate process.

Repository automation may dispatch the `Release` workflow after creating the same annotated tag, but the
dispatch ref must be the tag itself (`v<package.version>`), never `main` or a raw commit SHA. This keeps
the checkout, Sigstore identity, and provenance bound to `refs/tags/v*`.

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
