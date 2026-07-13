use anyhow::Result;

use crate::daemon::{DaemonJob, DaemonJobKind, DaemonJobStatus, DaemonState};
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
