use std::path::PathBuf;
use std::sync::Arc;

use athanor_core::{CoreResult, ProcessLimits, ProcessOutput, ProcessRequest, ProcessRunner};

use super::{
    CancellableProcessRunner, SharedProcessRunner, current_process_runner, with_process_runner,
};
use crate::CancellationToken;

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
        _operation: Option<&athanor_core::OperationContext>,
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
        current_process_runner().run(request()).await.unwrap().stdout
    }));
    let second_task = tokio::spawn(with_process_runner(second, async {
        tokio::task::yield_now().await;
        current_process_runner().run(request()).await.unwrap().stdout
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
