include!("../runtime.rs");

pub use process_runner::{CancellableProcessRunner, SharedProcessRunner, default_process_runner};

#[cfg(test)]
mod process_runner_scope_tests;

tokio::task_local! {
    static PROCESS_OPERATION: Option<athanor_core::OperationContext>;
    static PROCESS_RUNNER_OVERRIDE: Option<SharedProcessRunner>;
}

#[derive(Clone)]
pub(super) struct ProcessExecutionContext {
    pub(super) operation: Option<athanor_core::OperationContext>,
    pub(super) cancellation: Option<CancellationToken>,
}

/// Executes a future with an explicit external-process runner override.
///
/// The override is task-local and immutable for the scoped future. It does not mutate process-global
/// state and composes safely with concurrently running Athanor application instances.
pub async fn with_process_runner<T>(
    runner: SharedProcessRunner,
    future: impl Future<Output = T>,
) -> T {
    PROCESS_RUNNER_OVERRIDE.scope(Some(runner), future).await
}

pub(crate) async fn with_process_execution_context<T>(
    operation: athanor_core::OperationContext,
    cancellation: Option<CancellationToken>,
    future: impl Future<Output = T>,
) -> T {
    PROCESS_OPERATION
        .scope(
            Some(operation),
            with_process_cancellation(cancellation, future),
        )
        .await
}

pub(super) fn current_process_execution_context() -> ProcessExecutionContext {
    ProcessExecutionContext {
        operation: PROCESS_OPERATION
            .try_with(Clone::clone)
            .ok()
            .flatten(),
        cancellation: PROCESS_CANCELLATION
            .try_with(Clone::clone)
            .ok()
            .flatten(),
    }
}

pub(super) fn current_process_runner() -> SharedProcessRunner {
    PROCESS_RUNNER_OVERRIDE
        .try_with(Clone::clone)
        .ok()
        .flatten()
        .unwrap_or_else(default_process_runner)
}
