---
id: doc://docs/development/production.md
kind: development
language: en
last_verified_snapshot: snap_jsonl_00000272
source_language: en
status: verified
---
# Production Operation

Athanor production v1 is a single-user local daemon for Windows 10/11 and systemd-based Linux.
LAN access, TLS, multi-user authorization, macOS service installation, and sandboxed third-party
plugins are outside this release.

## Security Model

Daemon protocol v2 uses a fresh 256-bit authentication token for every daemon process. Clients read
the token from the per-user runtime directory and attach it automatically. Endpoint metadata contains
the token path, never the token value. Authentication failures use a generic error and do not create
job records.

Runtime locations:

```text
Linux:   $XDG_RUNTIME_DIR/athanor/<project-id>
Windows: %LOCALAPPDATA%\Athanor\runtime\<project-id>
```

Linux runtime directories use mode `0700`; token, lock, and endpoint files are protected by the
directory and secret files use mode `0600`. Windows runtime paths disable inherited ACLs and grant
full access only to the current user. TCP binds are restricted to loopback. Local socket transport is
recommended. On Unix platforms with short socket-path limits, the daemon may place the socket in a
short private `0700` temporary directory while keeping endpoint metadata and authentication material
under the protected runtime directory. Bound Unix domain sockets use mode `0600`. Windows named
pipes are created per accepted connection and the acceptor recreates the server after disconnect.

Protocol v1 is disabled by default. `--insecure-allow-v1` is a temporary migration option for
loopback TCP only and must not be used by installed services.

## Service Operation

Register the repository, install per-user autostart, and verify health:

```bash
ath projects add my-project /path/to/repository
athd service install my-project --transport local-socket --watch
athd service status my-project --json
athd ping my-project --json
athd doctor my-project --json
```

Linux uses a `systemd --user` unit with restart-on-failure and a bounded stop timeout. Windows uses a
least-privilege Task Scheduler task for the current interactive user. Installation is idempotent and
updates the existing definition. Remove autostart with:

```bash
athd service uninstall my-project
```

`athd stop` rejects new work, requests cooperative cancellation for active jobs, waits up to 30
seconds by default, and then removes endpoint and token files. The OS-held lock is released
automatically after crashes, and the next successful lock acquisition replaces stale lock metadata,
so stale lock files do not block restart or preserve old process identity.

## Fault Recovery Boundaries

Cancellation is exercised against real background index and coordinated generation jobs. Before
commit/publication, a cancelled index leaves the canonical latest pointer, index state, and JSONL
read model unchanged; a cancelled generation leaves the previous generated-current pointer selected
and removes its staging directory. Read-only daemon queries remain available during indexing and are
covered by concurrent mixed-command bursts.

An abrupt OS process kill at an exact filesystem syscall boundary cannot be scheduled
deterministically in an in-process unit test. That boundary is handled by immutable generation
directories, final pointer replacement, OS-released runtime locks, and startup cleanup restricted to
known staging paths. Linux and Windows production workflows cover full daemon process/service
lifecycle. Mid-syscall kill injection remains a platform E2E/manual release-hardening technique, not
a portable unit-test guarantee.

Structured JSONL daemon logs are written under the runtime directory. Logs rotate at 10 MiB and keep
five historical files.

Runtime environment:

- `ATHANOR_RUNTIME_DIR` overrides the protected per-user runtime root for daemon endpoint, token,
  lock, and log files. Production services should normally rely on the platform default; CI uses the
  override to isolate test runs.
- `ATHANOR_LOG_FORMAT` selects daemon log formatting. Use the default structured JSONL format for
  services and automation so logs remain machine-readable.
- `ATHANOR_PROJECT_REGISTRY` overrides the project registry file location. CI and isolated smoke
  tests pass an explicit registry path instead of using the user's default registry.
- `XDG_RUNTIME_DIR` is the preferred Linux base for protected runtime state when
  `ATHANOR_RUNTIME_DIR` is not set.
- `LOCALAPPDATA` is the preferred Windows base for protected runtime state when
  `ATHANOR_RUNTIME_DIR` is not set.
- `HOME` and `USERPROFILE` are fallback locations for per-user project registry storage when no
  explicit registry path is provided.
- `USERNAME` and `USERDOMAIN` identify the current Windows user for per-user Task Scheduler service
  registration and ACL ownership.

## External Adapters

External process adapters are disabled by default:

```toml
[adapters]
allow_external_process = false
external_process_allowlist = []
```

Set the value to `true` only for trusted manifests and executables. Athanor logs every enabled
external process adapter as a security warning. This opt-in does not provide process sandboxing.
When external process adapters are enabled, each command program must also match a canonicalized
entry in `external_process_allowlist`. Relative allowlist entries are resolved from the project root,
and an empty allowlist rejects all external process adapters.

Adapter manifests are parsed strictly and reject unknown fields. External adapter commands must use
explicit absolute paths or manifest-relative paths that stay inside the manifest directory after
canonicalization; bare command names are not resolved through `PATH`. Process execution is bounded by
stdin, stdout, stderr, and wall-clock limits. Timed-out adapter processes are terminated, and
oversized output fails with a bounded adapter-scoped error.

External adapter manifests also require user-level trust stored outside the repository. The default
trust store is `~/.athanor/adapter-trust.json` or `ATHANOR_ADAPTER_TRUST` when set. A trust record
contains the canonical manifest path and SHA-256 hash of the manifest content, so modifying a
manifest requires a new explicit trust action:

```bash
ath plugins list
ath plugins trust .athanor/plugins/example/athanor-adapter.json
ath plugins untrust .athanor/plugins/example/athanor-adapter.json
```

## Releases

Tagged releases publish signed and attested archives:

```text
athanor-x86_64-pc-windows-msvc.zip
athanor-x86_64-unknown-linux-gnu.tar.gz
```

Each archive contains `ath`, `athd`, README, license, and the platform install script. Release assets
include SHA-256 files, Sigstore bundles, and GitHub build provenance attestations.

Before deployment, verify the checksum and Sigstore bundle, install the binaries, register a project,
and run the authenticated `start -> ping -> index -> context -> stop` smoke sequence.

Release workflow:

- `.github/workflows/release.yml` runs on version tags and builds both supported release targets.
- The `build` job checks out source, installs the pinned Rust toolchain for each matrix target,
  restores the Rust build cache, builds locked release binaries, packages platform archives, writes
  SHA-256 checksums, installs Cosign, signs archives and checksums, emits build provenance
  attestations, and uploads target artifacts.
- The `publish` job downloads all build artifacts and publishes the GitHub release with generated
  release notes.

Production gate workflow:

- `.github/workflows/production.yml` runs on pull requests, manual dispatch, and the nightly schedule.
- The `daemon-e2e` job checks out source, installs Rust with `clippy` and `rustfmt`, restores cache,
  and runs authenticated daemon lifecycle checks on Linux and Windows against an isolated runtime
  directory and registry.
- On Windows, the same job also installs, checks, stops, and uninstalls the per-user service task.
- The scheduled `nightly-soak` job starts a watched daemon and repeatedly exercises `ping`,
  `overview`, and `context` for one hour before stopping the daemon.
