use anyhow::Result;
use athanor_core::OperationContext;

use crate::CancellationToken;
use crate::daemon::{DaemonJob, DaemonJobKind, DaemonJobStatus, DaemonState};
use crate::daemon_job_state::finish;
use crate::daemon_jobs_support::{prune, unix_time_ms};

pub(super) fn start(
    state: &DaemonState,
    kind: DaemonJobKind,
    description: String,
) -> Result<String> {
    let mut next_job_sequence = state
        .next_job_sequence
        .lock()
        .map_err(|_| anyhow::anyhow!("daemon job sequence lock is poisoned"))?;
    let job_id = format!("job_{:08}", *next_job_sequence);
    *next_job_sequence += 1;
    drop(next_job_sequence);

    let now = unix_time_ms()?;
    let mut jobs = state
        .jobs
        .lock()
        .map_err(|_| anyhow::anyhow!("daemon job registry lock is poisoned"))?;
    jobs.push(DaemonJob {
        id: job_id.clone(),
        kind,
        status: DaemonJobStatus::Queued,
        description,
        created_at_unix_ms: now,
        started_at_unix_ms: None,
        finished_at_unix_ms: None,
        result: None,
        error: None,
    });
    prune(&mut jobs, state.max_job_history);
    Ok(job_id)
}

/// Compatibility helper for tests and jobs that do not yet carry an `OperationContext`.
#[allow(dead_code)]
pub(crate) fn start_cancellable(
    state: &DaemonState,
    kind: DaemonJobKind,
    description: String,
) -> Result<(String, CancellationToken)> {
    start_cancellable_inner(state, kind, description, None)
}

/// Starts a cancellable job whose registry token also controls the supplied core operation.
pub(crate) fn start_cancellable_with_operation(
    state: &DaemonState,
    kind: DaemonJobKind,
    description: String,
    operation: &OperationContext,
) -> Result<(String, CancellationToken)> {
    start_cancellable_inner(state, kind, description, Some(operation))
}

fn start_cancellable_inner(
    state: &DaemonState,
    kind: DaemonJobKind,
    description: String,
    operation: Option<&OperationContext>,
) -> Result<(String, CancellationToken)> {
    let job_id = start(state, kind, description)?;
    let cancellation = CancellationToken::new();

    if let Some(operation) = operation
        && let Err(error) = cancellation.bind_operation(operation)
    {
        let message = format!("failed to bind daemon cancellation: {error}");
        let _ = finish(
            state,
            &job_id,
            DaemonJobStatus::Failed,
            None,
            Some(message.clone()),
        );
        anyhow::bail!(message);
    }

    let mut tokens = match state.cancellation_tokens.lock() {
        Ok(tokens) => tokens,
        Err(_) => {
            let message = "daemon cancellation registry lock is poisoned".to_string();
            let _ = finish(
                state,
                &job_id,
                DaemonJobStatus::Failed,
                None,
                Some(message.clone()),
            );
            anyhow::bail!(message);
        }
    };
    tokens.insert(job_id.clone(), cancellation.clone());
    Ok((job_id, cancellation))
}
