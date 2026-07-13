#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::process::Stdio;
use std::time::Duration;

use athanor_core::{CoreError, CoreResult, ProcessOutput, ProcessRequest, ProcessRunner};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

use crate::CancellationToken;

/// Tokio implementation of the transport-neutral [`ProcessRunner`] port.
#[derive(Debug, Default, Clone, Copy)]
pub struct TokioProcessRunner;

impl TokioProcessRunner {
    /// Runs a process while honoring application-level cancellation.
    pub async fn run_cancellable(
        &self,
        request: ProcessRequest,
        cancellation: Option<&CancellationToken>,
    ) -> CoreResult<ProcessOutput> {
        execute_process(request, cancellation).await
    }
}

#[async_trait::async_trait]
impl ProcessRunner for TokioProcessRunner {
    async fn run(&self, request: ProcessRequest) -> CoreResult<ProcessOutput> {
        self.run_cancellable(request, None).await
    }
}

async fn execute_process(
    request: ProcessRequest,
    cancellation: Option<&CancellationToken>,
) -> CoreResult<ProcessOutput> {
    if request.stdin.len() > request.limits.max_stdin_bytes {
        return Err(CoreError::Adapter(format!(
            "input for {} exceeded {} bytes",
            request.label, request.limits.max_stdin_bytes
        )));
    }

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
    let stdout_reader = tokio::spawn(read_limited(stdout, request.limits.max_stdout_bytes));
    let stderr_reader = tokio::spawn(read_limited(stderr, request.limits.max_stderr_bytes));
    {
        let mut stdin = child.stdin.take().ok_or_else(|| {
            CoreError::Adapter(format!("failed to open stdin for {}", request.label))
        })?;
        if let Err(error) = stdin.write_all(&request.stdin).await {
            // A short-lived adapter may deliberately exit before consuming stdin. Preserve its
            // exit status and stderr instead of replacing them with a BrokenPipe write error.
            if error.kind() != std::io::ErrorKind::BrokenPipe {
                return Err(CoreError::Adapter(format!(
                    "failed to write input for {}: {error}",
                    request.label
                )));
            }
        }
    }

    let deadline = tokio::time::Instant::now() + Duration::from_millis(request.limits.timeout_ms);
    let status = tokio::select! {
        result = child.wait() => result.map_err(|error| CoreError::Adapter(format!("failed to wait for {}: {error}", request.label)))?,
        _ = tokio::time::sleep_until(deadline) => {
            terminate_external_process_tree(&mut child).await;
            let _ = stdout_reader.await;
            let _ = stderr_reader.await;
            return Err(CoreError::DeadlineExceeded(format!("{} timed out after {} ms", request.label, request.limits.timeout_ms)));
        }
        _ = wait_for_cancellation(cancellation), if cancellation.is_some() => {
            terminate_external_process_tree(&mut child).await;
            let _ = stdout_reader.await;
            let _ = stderr_reader.await;
            return Err(CoreError::Cancelled(format!("{} was cancelled", request.label)));
        }
    };
    let (stdout, stdout_truncated) = stdout_reader.await.map_err(|_| {
        CoreError::Adapter(format!("failed to read stdout for {}", request.label))
    })??;
    let (stderr, stderr_truncated) = stderr_reader.await.map_err(|_| {
        CoreError::Adapter(format!("failed to read stderr for {}", request.label))
    })??;
    Ok(ProcessOutput {
        success: status.success(),
        exit_code: status.code(),
        stdout,
        stderr,
        stdout_truncated,
        stderr_truncated,
    })
}

/// Stops an external adapter and, where the platform exposes a native tree command, its descendants.
///
/// `Child::kill` is retained as a fallback because a descendant may have already exited or the
/// platform helper may be unavailable. Unix children run in their own process group and receive a
/// group signal. Windows `taskkill /T` reaches child processes spawned by batch files and adapter
/// launchers; Job Object containment remains a future hardening step.
async fn terminate_external_process_tree(child: &mut tokio::process::Child) {
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
    if let Some(pid) = child.id() {
        let pid = pid.to_string();
        let _ = Command::new("taskkill")
            .args(["/PID", pid.as_str(), "/T", "/F"])
            .kill_on_drop(true)
            .output()
            .await;
    }

    let _ = child.kill().await;
}

async fn wait_for_cancellation(cancellation: Option<&CancellationToken>) {
    if cancellation.is_none() {
        return std::future::pending::<()>().await;
    }
    let cancellation = cancellation.expect("cancellation was checked above");
    while !cancellation.is_cancelled() {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
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
