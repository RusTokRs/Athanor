use anyhow::Result;

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

pub(crate) fn start_cancellable(
    state: &DaemonState,
    kind: DaemonJobKind,
    description: String,
) -> Result<(String, CancellationToken)> {
    let job_id = start(state, kind, description)?;
    let cancellation = CancellationToken::new();
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
