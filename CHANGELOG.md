# Changelog

All notable user-visible changes to Athanor are recorded in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and release versions use
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). The top `[Unreleased]` section collects
changes after the latest frozen candidate. A release version must have exactly one section with an ISO
`YYYY-MM-DD` date and at least one substantive note before its matching `v<version>` tag may be
published. HTML comments, thematic breaks, empty list markers, and empty fenced code blocks are not
substantive release notes.

## [Unreleased]

## [0.2.1] - 2026-07-23

### Fixed

- Package the actual `*.cdx.json` files emitted by `cargo-cyclonedx 0.5.9` instead of searching only for
  `bom.json`.
- Validate that the workspace SBOM inventory is non-empty and that every packaged document is a
  non-empty CycloneDX JSON object before signing or attesting the archive.

### Release

- Preserve `v0.2.0` as an immutable failed publication attempt: its release contract and Linux artifacts
  passed, but the SBOM inventory gate stopped publication before a GitHub Release was created.

## [0.2.0] - 2026-07-22

### Added

- Local-first `ath` CLI and `athd` daemon for indexing and querying repository knowledge.
- Explicit runtime composition for stores, search, projectors, extractors, and transports.
- Transactional index publication with cancellation-safe rollback and durable post-commit success.
- Memory, JSONL, and SurrealDB storage backends with shared conformance coverage.
- Rust, JavaScript/TypeScript, Markdown, OpenAPI, GraphQL, and Rustok-oriented extraction and linking.
- MCP transport with bounded request admission, responsive cancellation, and typed index reports.
- Cross-protocol OpenAPI/GraphQL consistency checks for requests, responses, status policy,
  authentication families, and permission scopes.
- Evidence-backed documentation lifecycle, JSON contract inventory, and repository indexing smoke tests.
- Cross-platform CI on Linux, macOS, and Windows with security, feature, coverage, installer, AppSec,
  Store Conformance, and Production Gate workflows.
- Release archives for Linux and Windows with checksums, Sigstore bundles, provenance attestations,
  and CycloneDX SBOMs.

### Changed

- Release tags are gated by repository-owned package-version, changelog, artifact, signature, provenance,
  SBOM, and release-note checks.
- Release preparation rejects occupied tag names and pins an annotated tag to the exact verified candidate
  SHA instead of assuming the current `main` HEAD is still the candidate.

### Security

- Exact commit statuses are published for the CI matrix, AppSec, and Store Conformance workflows.
- Release workflows use pinned actions, least-privilege job permissions, signed assets, and provenance.
- Release publication rejects malformed or impossible calendar dates, duplicate version sections,
  decorative-only notes, and empty fenced blocks before artifact jobs run.

## [0.1.0] - 2026-06-25

### Added

- Historical development snapshot establishing the initial `ath` and `athd` package versions.
