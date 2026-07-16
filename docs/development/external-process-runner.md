---
id: doc://docs/development/external-process-runner.md
kind: developer_guide
language: en
source_language: en
status: verified
---
# External Process Runner Boundary

## Scope

External source, extractor, linker, and checker adapters execute through the app-layer
`CancellableProcessRunner` boundary. Adapter protocol code does not construct `tokio::process::Command`
directly and does not select a process runner through mutable global state.

The transport-neutral core contract remains `ProcessRunner`, `ProcessRequest`, `ProcessLimits`, and
`ProcessOutput`. The app-layer extension adds operation-aware execution:

```rust
CancellableProcessRunner::run_with_operation_context(
    request,
    operation,
    cancellation,
)
```

`TokioProcessRunner` is the production default. Tests and embedders can supply an immutable
`Arc<dyn CancellableProcessRunner>` through `athanor_app::with_process_runner` for the scoped future.
The override is Tokio task-local, so concurrently running application instances do not overwrite each
other and no process-global runner registry is required.

## Operation Propagation

The indexing pipeline scopes the same `OperationContext` and optional `CancellationToken` around all
external adapter phases:

- source discovery;
- extraction;
- linking;
- checking.

The process runner checks active state before spawning, while writing stdin, while waiting for the
child, and before returning success. The operation deadline therefore participates in the process
lifecycle instead of relying only on an outer future timeout.

The request-local adapter timeout remains an independent upper bound. The earliest terminal condition
wins:

- process completion;
- request timeout;
- operation deadline;
- operation cancellation;
- application cancellation token.

Cancellation or timeout terminates the process tree and drains the stdout/stderr reader tasks before
the runner returns. A simultaneous late process success cannot replace the terminal operation error.

## Bounded I/O And Errors

`ProcessRequest` carries explicit limits for:

- serialized stdin bytes;
- captured stdout bytes;
- captured stderr bytes;
- wall-clock execution time.

The runner never searches `PATH`: executable and working-directory paths are resolved before the
request reaches the runner. External adapter executable hashes are still checked immediately before
launch.

Stable classifications are preserved:

- spawn, pipe, wait, write, non-zero exit, and output-limit failures use `adapter_execution`;
- invalid successful stdout JSON uses `adapter_protocol`;
- cancellation uses `cancelled`;
- request timeout or operation deadline uses `deadline_exceeded`.

The adapter protocol continues to reject truncated stdout or stderr before decoding a successful
payload.

## Process-Tree Cleanup

Unix children start in a dedicated process group. Timeout or cancellation sends termination signals to
the group and retains `Child::kill` as a fallback. Windows uses `taskkill /T /F` plus the same child
fallback. Windows Job Object containment and stronger Linux sandbox launchers remain separate P1.1
hardening work.

## Compatibility

The existing public core `ProcessRunner::run` method and `TokioProcessRunner::run_cancellable` entry
point remain available. External adapter manifests, command arguments, JSON payloads, trust records,
allowlists, clean-environment behavior, and error messages remain compatible.

## Verification

```bash
cargo fmt --all -- --check
cargo test -p athanor-app process_runner --locked
cargo test -p athanor-app process_adapter --locked
cargo test -p athanor-app process_runner_scope --locked
cargo test -p athanor-app runtime --locked
cargo clippy -p athanor-app --all-targets --locked -- -D warnings
```

Required regressions cover:

- successful bounded output;
- spawn failure;
- pre-expired operation rejection before spawn;
- request timeout;
- operation cancellation;
- process cleanup before a delayed side effect;
- stdout truncation;
- stable non-zero-exit and protocol classifications;
- injected request, limit, operation, and cancellation propagation;
- concurrent task-local runner isolation.

Hosted Linux and Windows execution evidence remains separate and is not claimed without a workflow
run or local Rust toolchain output.
