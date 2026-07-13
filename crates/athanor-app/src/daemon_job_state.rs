use anyhow::Result;
use serde_json::Value;

use crate::daemon::{DaemonJobKind, DaemonJobStatus, DaemonState};
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

pub(crate) fn begin_or_finish_failed(state: &DaemonState, job_id: &str) -> bool {
    match mark_running(state, job_id) {
        Ok(started) => started,
        Err(error) => {
            let _ = finish(
                state,
                job_id,
                DaemonJobStatus::Failed,
                None,
                Some(error.to_string()),
            );
            false
        }
    }
}

pub(crate) fn finish_cancellable_error(
    state: &DaemonState,
    job_id: &str,
    error: anyhow::Error,
) -> Result<()> {
    let cancelled = error
        .chain()
        .any(|cause| cause.to_string().contains("operation cancelled"));
    let (status, message) = if cancelled {
        (
            DaemonJobStatus::Cancelled,
            "operation cancelled".to_string(),
        )
    } else {
        (DaemonJobStatus::Failed, error.to_string())
    };
    finish(state, job_id, status, None, Some(message))
}

pub(crate) fn has_active(state: &DaemonState, kind: DaemonJobKind) -> Result<bool> {
    let jobs = state
        .jobs
        .lock()
        .map_err(|_| anyhow::anyhow!("daemon job registry lock is poisoned"))?;
    Ok(jobs.iter().any(|job| {
        job.kind == kind
            && matches!(
                job.status,
                DaemonJobStatus::Queued | DaemonJobStatus::Running | DaemonJobStatus::Cancelling
            )
    }))
}

pub(crate) fn finish(
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
    crate::daemon_job_cancellation::remove(state, job_id)?;
    Ok(())
}
