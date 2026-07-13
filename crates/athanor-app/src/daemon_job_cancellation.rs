use anyhow::{Result, bail};

use crate::CancellationToken;
use crate::daemon::{DaemonJob, DaemonJobStatus, DaemonState};
use crate::daemon_job_registry::get as get_job;
use crate::daemon_jobs_support::{is_valid_job_id, unix_time_ms};

pub(super) fn cancel(state: &DaemonState, job_id: &str) -> Result<DaemonJob> {
    if !is_valid_job_id(job_id) {
        bail!("daemon job id must use the form job_00000001");
    }
    let current = get_job(state, job_id)?;
    match current.status {
        DaemonJobStatus::Queued => {
            if let Some(cancellation) = get(state, job_id)? {
                cancellation.cancel();
            }
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
                    job.status = DaemonJobStatus::Cancelled;
                    job.finished_at_unix_ms = Some(unix_time_ms()?);
                    job.error = Some("job cancelled before start".to_string());
                }
                DaemonJobStatus::Running => {
                    job.status = DaemonJobStatus::Cancelling;
                    job.error = Some("job cancellation requested".to_string());
                }
                _ => {}
            }
            let cancelled = job.clone();
            drop(jobs);
            if cancelled.status == DaemonJobStatus::Cancelled {
                remove(state, job_id)?;
            }
            Ok(cancelled)
        }
        DaemonJobStatus::Running => {
            let cancellation = get(state, job_id)?.ok_or_else(|| {
                anyhow::anyhow!("daemon job `{job_id}` is running and is not cancellable")
            })?;
            cancellation.cancel();
            let mut jobs = state
                .jobs
                .lock()
                .map_err(|_| anyhow::anyhow!("daemon job registry lock is poisoned"))?;
            let job = jobs
                .iter_mut()
                .find(|job| job.id == job_id)
                .ok_or_else(|| anyhow::anyhow!("daemon job `{job_id}` was not found"))?;
            if job.status == DaemonJobStatus::Running {
                job.status = DaemonJobStatus::Cancelling;
                job.error = Some("job cancellation requested".to_string());
            }
            Ok(job.clone())
        }
        DaemonJobStatus::Cancelling
        | DaemonJobStatus::Succeeded
        | DaemonJobStatus::Failed
        | DaemonJobStatus::Cancelled => Ok(current),
    }
}

pub(crate) fn get(state: &DaemonState, job_id: &str) -> Result<Option<CancellationToken>> {
    let tokens = state
        .cancellation_tokens
        .lock()
        .map_err(|_| anyhow::anyhow!("daemon cancellation registry lock is poisoned"))?;
    Ok(tokens.get(job_id).cloned())
}

pub(crate) fn remove(state: &DaemonState, job_id: &str) -> Result<()> {
    let mut tokens = state
        .cancellation_tokens
        .lock()
        .map_err(|_| anyhow::anyhow!("daemon cancellation registry lock is poisoned"))?;
    tokens.remove(job_id);
    Ok(())
}
