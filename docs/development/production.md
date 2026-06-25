---
id: doc://docs/development/production.md
kind: development
language: en
source_language: en
last_verified_snapshot: snap_jsonl_00000163
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
recommended.

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
automatically after crashes, so stale lock metadata does not block restart.

Structured JSONL daemon logs are written under the runtime directory. Logs rotate at 10 MiB and keep
five historical files.

## External Adapters

External process adapters are disabled by default:

```toml
[adapters]
allow_external_process = false
```

Set the value to `true` only for trusted manifests and executables. Athanor logs every enabled
external process adapter as a security warning. This opt-in does not provide process sandboxing.

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
