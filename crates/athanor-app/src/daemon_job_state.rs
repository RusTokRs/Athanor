use anyhow::Result;
use serde_json::Value;

use crate::daemon::{DaemonJobStatus, DaemonState};
use crate::daemon_jobs_support::{prune, unix_time_ms};

pub(super) fn mark_running(state: &DaemonState, job_id: &str) -> Result<bool> {
    let started_at = unix_time_ms()?;
    let mut jobs = state
        .jobs
        .lock()
        .map_err(|_| anyhow::anyhow!("daemon job registry lock is poisoned"))?;
    let job = jobs
        .iter_mut()
        .find(|job| job.id == job_id)
        .ok_or_else(|| anyhow::anyhow!("daemon job `{job_id}` was not found"))?;
    match job.status {
        DaemonJobStatus::Queued => {
            job.status = DaemonJobStatus::Running;
            job.started_at_unix_ms = Some(started_at);
            Ok(true)
        }
        DaemonJobStatus::Cancelled => Ok(false),
        DaemonJobStatus::Running | DaemonJobStatus::Cancelling => Ok(true),
        DaemonJobStatus::Succeeded | DaemonJobStatus::Failed => Ok(false),
    }
}

pub(super) fn finish(
    state: &DaemonState,
    job_id: &str,
    status: DaemonJobStatus,
    result: Option<Value>,
    error: Option<String>,
) -> Result<()> {
    let finished_at = unix_time_ms()?;
    let mut jobs = state
        .jobs
        .lock()
        .map_err(|_| anyhow::anyhow!("daemon job registry lock is poisoned"))?;
    let job = jobs
        .iter_mut()
        .find(|job| job.id == job_id)
        .ok_or_else(|| anyhow::anyhow!("daemon job `{job_id}` was not found"))?;
    job.status = status;
    job.finished_at_unix_ms = Some(finished_at);
    job.result = result;
    job.error = error;
    prune(&mut jobs, state.max_job_history);
    drop(jobs);
    crate::daemon::remove_cancellation_token(state, job_id)?;
    Ok(())
}
