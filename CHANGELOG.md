# Changelog

All notable user-visible changes to Athanor are recorded in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and release versions use
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). A version section remains marked
`Unreleased` until its matching `v<version>` tag is intentionally published.

## [Unreleased]

### Changed

- Release tags are being hardened with repository-owned version and artifact checks.

## [0.1.0] - Unreleased

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

### Security

- Exact commit statuses are published for the CI matrix, AppSec, and Store Conformance workflows.
- Release workflows use pinned actions, least-privilege job permissions, signed assets, and provenance.
