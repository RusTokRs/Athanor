use athanor_core::{CoreError, CoreResult, ProcessLimits as CoreProcessLimits, ProcessRequest};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use super::process_adapter_support::{ProcessCommand, ProcessLimits, process_output_excerpt};
use super::{CancellationToken, TokioProcessRunner};

pub(super) async fn run_with_limits<I, O>(
    adapter_kind: &str,
    adapter_id: &str,
    command: &ProcessCommand,
    input: &I,
    limits: ProcessLimits,
    cancellation: Option<&CancellationToken>,
) -> CoreResult<O>
where
    I: Serialize,
    O: for<'de> Deserialize<'de>,
{
    command.verify_unchanged().map_err(|error| {
        CoreError::Adapter(format!(
            "refusing external {adapter_kind} {adapter_id}: {error}"
        ))
    })?;
    let mut input = serde_json::to_vec(input).map_err(|error| {
        CoreError::Adapter(format!(
            "failed to serialize input for external {adapter_kind} {adapter_id}: {error}"
        ))
    })?;
    input.push(b'\n');
    let output = TokioProcessRunner
        .run_cancellable(
            ProcessRequest {
                label: format!("external {adapter_kind} {adapter_id}"),
                program: command.program.clone(),
                args: command.args.clone(),
                working_dir: command.working_dir.clone(),
                clear_environment: command.clear_environment,
                stdin: input,
                limits: CoreProcessLimits {
                    timeout_ms: limits.timeout.as_millis() as u64,
                    max_stdin_bytes: limits.max_stdin_bytes,
                    max_stdout_bytes: limits.max_stdout_bytes,
                    max_stderr_bytes: limits.max_stderr_bytes,
                },
            },
            cancellation,
        )
        .await?;
    let stdout = process_output_excerpt(&output.stdout);
    let stderr = process_output_excerpt(&output.stderr);
    if !stdout.is_empty() {
        debug!(adapter_kind, adapter_id, stdout = %stdout, "external process adapter stdout");
    }
    if !stderr.is_empty() {
        warn!(adapter_kind, adapter_id, stderr = %stderr, "external process adapter stderr");
    }
    if !output.success {
        return Err(CoreError::Adapter(format!(
            "external {adapter_kind} {adapter_id} exited with {}; stderr: {}",
            output.exit_code.map_or_else(
                || "unknown status".to_string(),
                |code| format!("exit code: {code}")
            ),
            stderr
        )));
    }
    if output.stdout_truncated {
        return Err(CoreError::Adapter(format!(
            "external {adapter_kind} {adapter_id} stdout exceeded {} bytes",
            limits.max_stdout_bytes
        )));
    }
    if output.stderr_truncated {
        return Err(CoreError::Adapter(format!(
            "external {adapter_kind} {adapter_id} stderr exceeded {} bytes",
            limits.max_stderr_bytes
        )));
    }
    serde_json::from_slice(&output.stdout).map_err(|error| {
        CoreError::AdapterProtocol(format!(
            "failed to parse output from external {adapter_kind} {adapter_id}: {error}"
        ))
    })
}
