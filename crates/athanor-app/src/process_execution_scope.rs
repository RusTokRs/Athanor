use std::future::Future;
use std::sync::Arc;

use athanor_core::{CoreResult, OperationContext, ProcessOutput, ProcessRequest, ProcessRunner};

use crate::CancellationToken;
use crate::runtime::TokioProcessRunner;

/// Application process-runner extension that carries cancellation and operation deadlines through
/// an external adapter lifecycle.
#[async_trait::async_trait]
pub trait CancellableProcessRunner: ProcessRunner {
    async fn run_with_operation_context(
        &self,
        request: ProcessRequest,
        operation: Option<&OperationContext>,
        cancellation: Option<&CancellationToken>,
    ) -> CoreResult<ProcessOutput>;
}

pub type SharedProcessRunner = Arc<dyn CancellableProcessRunner>;

pub fn default_process_runner() -> SharedProcessRunner {
    Arc::new(TokioProcessRunner)
}

#[derive(Clone)]
pub(crate) struct ProcessExecutionContext {
    pub(crate) operation: Option<OperationContext>,
    pub(crate) cancellation: Option<CancellationToken>,
}

tokio::task_local! {
    static PROCESS_EXECUTION_CONTEXT: Option<ProcessExecutionContext>;
    static PROCESS_RUNNER_OVERRIDE: Option<SharedProcessRunner>;
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
    operation: OperationContext,
    cancellation: Option<CancellationToken>,
    future: impl Future<Output = T>,
) -> T {
    let context = ProcessExecutionContext {
        operation: Some(operation),
        cancellation,
    };
    PROCESS_EXECUTION_CONTEXT.scope(Some(context), future).await
}

pub(crate) fn current_process_execution_context() -> ProcessExecutionContext {
    PROCESS_EXECUTION_CONTEXT
        .try_with(Clone::clone)
        .ok()
        .flatten()
        .unwrap_or(ProcessExecutionContext {
            operation: None,
            cancellation: None,
        })
}

pub(crate) fn current_process_runner() -> SharedProcessRunner {
    PROCESS_RUNNER_OVERRIDE
        .try_with(Clone::clone)
        .ok()
        .flatten()
        .unwrap_or_else(default_process_runner)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use athanor_core::{ProcessLimits, ProcessOutput};

    use super::*;

    struct MarkerRunner(u8);

    #[async_trait::async_trait]
    impl ProcessRunner for MarkerRunner {
        async fn run(&self, _request: ProcessRequest) -> CoreResult<ProcessOutput> {
            Ok(ProcessOutput {
                success: true,
                exit_code: Some(0),
                stdout: vec![self.0],
                stderr: Vec::new(),
                stdout_truncated: false,
                stderr_truncated: false,
            })
        }
    }

    #[async_trait::async_trait]
    impl CancellableProcessRunner for MarkerRunner {
        async fn run_with_operation_context(
            &self,
            request: ProcessRequest,
            _operation: Option<&OperationContext>,
            _cancellation: Option<&CancellationToken>,
        ) -> CoreResult<ProcessOutput> {
            self.run(request).await
        }
    }

    #[tokio::test]
    async fn concurrent_runner_overrides_do_not_leak_between_tasks() {
        let first: SharedProcessRunner = Arc::new(MarkerRunner(1));
        let second: SharedProcessRunner = Arc::new(MarkerRunner(2));

        let first_task = tokio::spawn(with_process_runner(first, async {
            tokio::task::yield_now().await;
            current_process_runner()
                .run(request())
                .await
                .unwrap()
                .stdout
        }));
        let second_task = tokio::spawn(with_process_runner(second, async {
            tokio::task::yield_now().await;
            current_process_runner()
                .run(request())
                .await
                .unwrap()
                .stdout
        }));

        assert_eq!(first_task.await.unwrap(), vec![1]);
        assert_eq!(second_task.await.unwrap(), vec![2]);
    }

    fn request() -> ProcessRequest {
        ProcessRequest {
            label: "scoped fake process".to_string(),
            program: PathBuf::from("/not-executed"),
            args: Vec::new(),
            working_dir: PathBuf::from("/not-executed"),
            clear_environment: true,
            stdin: Vec::new(),
            limits: ProcessLimits::default(),
        }
    }
}
