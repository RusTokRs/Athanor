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
    let process_id = child.id();
    let stdout = child.stdout.take().ok_or_else(|| {
        CoreError::Adapter(format!("failed to open stdout for {}", request.label))
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        CoreError::Adapter(format!("failed to open stderr for {}", request.label))
    })?;
    let (truncate_tx, mut truncate_rx) = tokio::sync::mpsc::channel::<()>(2);
    // Keep the channel open after both readers finish normally. Otherwise `recv()` reports
    // channel closure and the select arms below mistake ordinary EOF for oversized output.
    let _truncate_guard = truncate_tx.clone();
    let mut stdout_reader = tokio::spawn(read_limited(
        stdout,
        request.limits.max_stdout_bytes,
        Some(truncate_tx.clone()),
    ));
    let mut stderr_reader = tokio::spawn(read_limited(
        stderr,
        request.limits.max_stderr_bytes,
        Some(truncate_tx),
    ));
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| CoreError::Adapter(format!("failed to open stdin for {}", request.label)))?;
    let deadline = tokio::time::Instant::now() + Duration::from_millis(request.limits.timeout_ms);

    let write_result = tokio::select! {
        result = stdin.write_all(&request.stdin) => Some(result),
        _ = tokio::time::sleep_until(deadline) => {
            terminate_and_join(&mut child, process_id, &mut stdout_reader, &mut stderr_reader).await;
            return Err(process_timeout(&request));
        }
        _ = truncate_rx.recv() => None,
        terminal = wait_for_termination(operation, cancellation), if operation.is_some() || cancellation.is_some() => {
            terminate_and_join(&mut child, process_id, &mut stdout_reader, &mut stderr_reader).await;
            return Err(terminal);
        }
    };
    let Some(write_result) = write_result else {
        terminate_external_process_tree(&mut child, process_id).await;
        return collect_truncated_output(
            &request,
            deadline,
            operation,
            cancellation,
            &mut stdout_reader,
            &mut stderr_reader,
        )
        .await;
    };
    if let Err(error) = write_result
        && error.kind() != std::io::ErrorKind::BrokenPipe
    {
        terminate_and_join(
            &mut child,
            process_id,
            &mut stdout_reader,
            &mut stderr_reader,
        )
        .await;
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
        terminate_and_join(
            &mut child,
            process_id,
            &mut stdout_reader,
            &mut stderr_reader,
        )
        .await;
        return Err(CoreError::Adapter(format!(
            "failed to close input for {}: {error}",
            request.label
        )));
    }
    drop(stdin);

    let status = tokio::select! {
        result = child.wait() => Some(result.map_err(|error| CoreError::Adapter(format!("failed to wait for {}: {error}", request.label)))?),
        _ = tokio::time::sleep_until(deadline) => {
            terminate_and_join(&mut child, process_id, &mut stdout_reader, &mut stderr_reader).await;
            return Err(process_timeout(&request));
        }
        _ = truncate_rx.recv() => None,
        terminal = wait_for_termination(operation, cancellation), if operation.is_some() || cancellation.is_some() => {
            terminate_and_join(&mut child, process_id, &mut stdout_reader, &mut stderr_reader).await;
            return Err(terminal);
        }
    };
    let Some(status) = status else {
        terminate_external_process_tree(&mut child, process_id).await;
        return collect_truncated_output(
            &request,
            deadline,
            operation,
            cancellation,
            &mut stdout_reader,
            &mut stderr_reader,
        )
        .await;
    };
    let output = tokio::select! {
        result = async {
            let (stdout, stderr) = tokio::join!(&mut stdout_reader, &mut stderr_reader);
            let (stdout, stdout_truncated) = stdout.map_err(|_| {
                CoreError::Adapter(format!("failed to read stdout for {}", request.label))
            })??;
            let (stderr, stderr_truncated) = stderr.map_err(|_| {
                CoreError::Adapter(format!("failed to read stderr for {}", request.label))
            })??;
            Ok((stdout, stdout_truncated, stderr, stderr_truncated))
        } => Ok(result),
        _ = tokio::time::sleep_until(deadline) => Err(process_timeout(&request)),
        terminal = wait_for_termination(operation, cancellation), if operation.is_some() || cancellation.is_some() => Err(terminal),
    };
    let (stdout, stdout_truncated, stderr, stderr_truncated) = match output {
        Ok(result) => result?,
        Err(error) => {
            terminate_and_join(
                &mut child,
                process_id,
                &mut stdout_reader,
                &mut stderr_reader,
            )
            .await;
            return Err(error);
        }
    };
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
        if let Some(cancellation) = cancellation {
            tokio::select! {
                _ = cancellation.cancelled() => {
                    return CoreError::Cancelled(
                        "external process operation was cancelled".to_string(),
                    );
                }
                _ = tokio::time::sleep(Duration::from_millis(10)) => {}
            }
        } else {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
}

async fn terminate_and_join(
    child: &mut tokio::process::Child,
    process_id: Option<u32>,
    stdout_reader: &mut tokio::task::JoinHandle<CoreResult<(Vec<u8>, bool)>>,
    stderr_reader: &mut tokio::task::JoinHandle<CoreResult<(Vec<u8>, bool)>>,
) {
    terminate_external_process_tree(child, process_id).await;
    abort_and_join(stdout_reader, stderr_reader).await;
}

async fn abort_and_join(
    stdout_reader: &mut tokio::task::JoinHandle<CoreResult<(Vec<u8>, bool)>>,
    stderr_reader: &mut tokio::task::JoinHandle<CoreResult<(Vec<u8>, bool)>>,
) {
    // A descendant outside the native tree-kill mechanism may retain an inherited pipe. Never
    // let joining readers reintroduce the deadline that killing the child was meant to enforce.
    stdout_reader.abort();
    stderr_reader.abort();
    let _ = stdout_reader.await;
    let _ = stderr_reader.await;
}

/// Stops an external adapter and, where the platform exposes a native tree command, its descendants.
///
/// `Child::kill` is retained as a fallback because a descendant may have already exited or the
/// platform helper may be unavailable. Unix children run in their own process group and receive a
/// group signal. Windows asks `taskkill /T` to remove descendants only while the direct child is
/// still alive, then falls back to Tokio's direct-child kill and always reaps the child.
async fn terminate_external_process_tree(
    child: &mut tokio::process::Child,
    process_id: Option<u32>,
) {
    #[cfg(unix)]
    if let Some(pid) = process_id {
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
    if child.try_wait().ok().flatten().is_none()
        && let Some(pid) = process_id
    {
        let pid = pid.to_string();
        if let Ok(mut tree_kill) = Command::new("taskkill")
            .args(["/PID", pid.as_str(), "/T", "/F"])
            .kill_on_drop(true)
            .spawn()
        {
            let _ = tree_kill.wait().await;
        }
    }

    // The platform-specific tree operation may race with a process that exited between the
    // liveness check and the kill request. Tokio's direct-child kill is the final fallback, and
    // wait() reaps the child even when the native helper already removed it.
    let _ = child.start_kill();
    let _ = child.wait().await;
}

async fn collect_truncated_output(
    request: &ProcessRequest,
    deadline: tokio::time::Instant,
    operation: Option<&OperationContext>,
    cancellation: Option<&CancellationToken>,
    stdout_reader: &mut tokio::task::JoinHandle<CoreResult<(Vec<u8>, bool)>>,
    stderr_reader: &mut tokio::task::JoinHandle<CoreResult<(Vec<u8>, bool)>>,
) -> CoreResult<ProcessOutput> {
    let output = tokio::select! {
        result = async {
            let (stdout, stderr) = tokio::join!(&mut *stdout_reader, &mut *stderr_reader);
            let (stdout, stdout_truncated) = stdout.map_err(|_| {
                CoreError::Adapter(format!("failed to read stdout for {}", request.label))
            })??;
            let (stderr, stderr_truncated) = stderr.map_err(|_| {
                CoreError::Adapter(format!("failed to read stderr for {}", request.label))
            })??;
            Ok((stdout, stdout_truncated, stderr, stderr_truncated))
        } => Ok(result),
        _ = tokio::time::sleep_until(deadline) => Err(process_timeout(request)),
        terminal = wait_for_termination(operation, cancellation), if operation.is_some() || cancellation.is_some() => Err(terminal),
    };
    let (stdout, stdout_truncated, stderr, stderr_truncated) = match output {
        Ok(result) => result?,
        Err(error) => {
            abort_and_join(stdout_reader, stderr_reader).await;
            return Err(error);
        }
    };
    Ok(ProcessOutput {
        success: false,
        exit_code: None,
        stdout,
        stderr,
        stdout_truncated,
        stderr_truncated,
    })
}

async fn read_limited(
    mut reader: impl AsyncRead + Unpin,
    max_bytes: usize,
    truncate_notify: Option<tokio::sync::mpsc::Sender<()>>,
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
            if let Some(notify) = truncate_notify {
                let _ = notify.try_send(());
            }
            break;
        }

        output.extend_from_slice(&buffer[..read]);
    }

    Ok((output, truncated))
}
