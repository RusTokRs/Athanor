#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::process::Stdio;
use std::time::Duration;

use athanor_core::{
    CoreError, CoreResult, OperationContext, OperationContextCancellation, ProcessOutput,
    ProcessRequest, ProcessRunner,
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

use crate::{CancellableProcessRunner, CancellationToken};

/// Tokio implementation of the transport-neutral [`ProcessRunner`] port.
#[derive(Debug, Default, Clone, Copy)]
pub struct TokioProcessRunner;

impl TokioProcessRunner {
    /// Compatibility entry point for callers that only own an application cancellation token.
    pub async fn run_cancellable(
        &self,
        request: ProcessRequest,
        cancellation: Option<&CancellationToken>,
    ) -> CoreResult<ProcessOutput> {
        execute_process(request, None, cancellation).await
    }
}

#[async_trait::async_trait]
impl ProcessRunner for TokioProcessRunner {
    async fn run(&self, request: ProcessRequest) -> CoreResult<ProcessOutput> {
        execute_process(request, None, None).await
    }
}

#[async_trait::async_trait]
impl CancellableProcessRunner for TokioProcessRunner {
    async fn run_with_operation_context(
        &self,
        request: ProcessRequest,
        operation: Option<&OperationContext>,
        cancellation: Option<&CancellationToken>,
    ) -> CoreResult<ProcessOutput> {
        execute_process(request, operation, cancellation).await
    }
}

async fn execute_process(
    request: ProcessRequest,
    operation: Option<&OperationContext>,
    cancellation: Option<&CancellationToken>,
) -> CoreResult<ProcessOutput> {
    if request.stdin.len() > request.limits.max_stdin_bytes {
        return Err(CoreError::Adapter(format!(
            "input for {} exceeded {} bytes",
            request.label, request.limits.max_stdin_bytes
        )));
    }
    check_active(operation, cancellation)?;

    let mut command = Command::new(&request.program);
    command
        .args(&request.args)
        .current_dir(&request.working_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if request.clear_environment {
        command.env_clear();
    }
    #[cfg(unix)]
    command.as_std_mut().process_group(0);

    let mut child = command.spawn().map_err(|error| {
        CoreError::Adapter(format!("failed to spawn {}: {error}", request.label))
    })?;
    let stdout = child.stdout.take().ok_or_else(|| {
        CoreError::Adapter(format!("failed to open stdout for {}", request.label))
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        CoreError::Adapter(format!("failed to open stderr for {}", request.label))
    })?;
    let mut stdout_reader = tokio::spawn(read_limited(stdout, request.limits.max_stdout_bytes));
    let mut stderr_reader = tokio::spawn(read_limited(stderr, request.limits.max_stderr_bytes));
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| CoreError::Adapter(format!("failed to open stdin for {}", request.label)))?;
    let deadline = tokio::time::Instant::now() + Duration::from_millis(request.limits.timeout_ms);

    let write_result = tokio::select! {
        result = stdin.write_all(&request.stdin) => result,
        _ = tokio::time::sleep_until(deadline) => {
            terminate_external_process_tree(&mut child).await;
            let _ = (&mut stdout_reader).await;
            let _ = (&mut stderr_reader).await;
            return Err(process_timeout(&request));
        }
        terminal = wait_for_termination(operation, cancellation), if operation.is_some() || cancellation.is_some() => {
            terminate_external_process_tree(&mut child).await;
            let _ = (&mut stdout_reader).await;
            let _ = (&mut stderr_reader).await;
            return Err(terminal);
        }
    };
    if let Err(error) = write_result
        && error.kind() != std::io::ErrorKind::BrokenPipe
    {
        terminate_external_process_tree(&mut child).await;
        let _ = (&mut stdout_reader).await;
        let _ = (&mut stderr_reader).await;
        return Err(CoreError::Adapter(format!(
            "failed to write input for {}: {error}",
            request.label
        )));
    }

    // Explicitly close the child's stdin before waiting for termination. This matters for
    // line-oriented interpreters on Windows, where relying on ChildStdin's drop alone can leave
    // the inherited pipe handle live long enough for a command waiting on EOF to stall.
    if let Err(error) = stdin.shutdown().await
        && error.kind() != std::io::ErrorKind::BrokenPipe
    {
        terminate_external_process_tree(&mut child).await;
        let _ = (&mut stdout_reader).await;
        let _ = (&mut stderr_reader).await;
        return Err(CoreError::Adapter(format!(
            "failed to close input for {}: {error}",
            request.label
        )));
    }
    drop(stdin);

    let status = tokio::select! {
        result = child.wait() => result.map_err(|error| CoreError::Adapter(format!("failed to wait for {}: {error}", request.label)))?,
        _ = tokio::time::sleep_until(deadline) => {
            terminate_external_process_tree(&mut child).await;
            let _ = (&mut stdout_reader).await;
            let _ = (&mut stderr_reader).await;
            return Err(process_timeout(&request));
        }
        terminal = wait_for_termination(operation, cancellation), if operation.is_some() || cancellation.is_some() => {
            terminate_external_process_tree(&mut child).await;
            let _ = (&mut stdout_reader).await;
            let _ = (&mut stderr_reader).await;
            return Err(terminal);
        }
    };
    let (stdout, stdout_truncated) = stdout_reader.await.map_err(|_| {
        CoreError::Adapter(format!("failed to read stdout for {}", request.label))
    })??;
    let (stderr, stderr_truncated) = stderr_reader.await.map_err(|_| {
        CoreError::Adapter(format!("failed to read stderr for {}", request.label))
    })??;
    check_active(operation, cancellation)?;

    Ok(ProcessOutput {
        success: status.success(),
        exit_code: status.code(),
        stdout,
        stderr,
        stdout_truncated,
        stderr_truncated,
    })
}

fn process_timeout(request: &ProcessRequest) -> CoreError {
    CoreError::DeadlineExceeded(format!(
        "{} timed out after {} ms",
        request.label, request.limits.timeout_ms
    ))
}

fn check_active(
    operation: Option<&OperationContext>,
    cancellation: Option<&CancellationToken>,
) -> CoreResult<()> {
    if let Some(operation) = operation {
        operation.check_active()?;
    }
    if cancellation.is_some_and(CancellationToken::is_cancelled) {
        return Err(CoreError::Cancelled(
            "external process operation was cancelled".to_string(),
        ));
    }
    Ok(())
}

async fn wait_for_termination(
    operation: Option<&OperationContext>,
    cancellation: Option<&CancellationToken>,
) -> CoreError {
    loop {
        if let Err(error) = check_active(operation, cancellation) {
            return error;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// Stops an external adapter and, where the platform exposes a native tree command, its descendants.
///
/// `Child::kill` is retained as a fallback because a descendant may have already exited or the
/// platform helper may be unavailable. Unix children run in their own process group and receive a
/// group signal. Windows starts `taskkill /T` for descendant cleanup, immediately signals the direct
/// child, then waits for both cleanup and reaping; Job Object containment remains future hardening.
async fn terminate_external_process_tree(child: &mut tokio::process::Child) {
    let _ = child.start_kill();

    #[cfg(unix)]
    if let Some(pid) = child.id() {
        let process_group = format!("-{pid}");
        let _ = Command::new("kill")
            .args(["-TERM", "--", process_group.as_str()])
            .kill_on_drop(true)
            .output()
            .await;
        let _ = Command::new("kill")
            .args(["-KILL", "--", process_group.as_str()])
            .kill_on_drop(true)
            .output()
            .await;
    }

    #[cfg(windows)]
    {
        let mut tree_kill = child.id().and_then(|pid| {
            let pid = pid.to_string();
            Command::new("taskkill")
                .args(["/PID", pid.as_str(), "/T", "/F"])
                .kill_on_drop(true)
                .spawn()
                .ok()
        });
        if let Some(tree_kill) = tree_kill.as_mut() {
            let _ = tree_kill.wait().await;
        }
    }

    let _ = child.wait().await;
}

async fn read_limited(
    mut reader: impl AsyncRead + Unpin,
    max_bytes: usize,
) -> CoreResult<(Vec<u8>, bool)> {
    let mut output = Vec::new();
    let mut buffer = [0_u8; 8192];
    let mut truncated = false;

    loop {
        let read = reader.read(&mut buffer).await.map_err(|error| {
            CoreError::Adapter(format!("failed to read external process output: {error}"))
        })?;
        if read == 0 {
            break;
        }

        let remaining = max_bytes.saturating_sub(output.len());
        if read > remaining {
            output.extend_from_slice(&buffer[..remaining]);
            truncated = true;
            break;
        }

        output.extend_from_slice(&buffer[..read]);
    }

    Ok((output, truncated))
}
