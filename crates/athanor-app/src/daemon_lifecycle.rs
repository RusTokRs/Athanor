use std::time::Duration;

use anyhow::{Result, bail};

use crate::daemon::{DaemonJobStatus, DaemonLifecycleState, DaemonState};
use crate::daemon_job_cancellation::cancel as cancel_daemon_job;

pub(super) fn current(state: &DaemonState) -> DaemonLifecycleState {
    state
        .lifecycle
        .lock()
        .map(|lifecycle| *lifecycle)
        .unwrap_or(DaemonLifecycleState::Stopping)
}

pub(super) fn set(state: &DaemonState, lifecycle: DaemonLifecycleState) {
    match state.lifecycle.lock() {
        Ok(mut current) => *current = lifecycle,
        Err(_) => tracing::warn!("daemon lifecycle lock is poisoned"),
    }
}

pub(super) fn active_job_count(state: &DaemonState) -> Result<usize> {
    let jobs = state
        .jobs
        .lock()
        .map_err(|_| anyhow::anyhow!("daemon job registry lock is poisoned"))?;
    Ok(jobs
        .iter()
        .filter(|job| {
            matches!(
                job.status,
                DaemonJobStatus::Queued | DaemonJobStatus::Running | DaemonJobStatus::Cancelling
            )
        })
        .count())
}

pub(super) fn cancel_active_jobs(state: &DaemonState) {
    let job_ids = match state.jobs.lock() {
        Ok(jobs) => jobs
            .iter()
            .filter(|job| {
                matches!(
                    job.status,
                    DaemonJobStatus::Queued
                        | DaemonJobStatus::Running
                        | DaemonJobStatus::Cancelling
                )
            })
            .map(|job| job.id.clone())
            .collect::<Vec<_>>(),
        Err(_) => {
            tracing::warn!("daemon job registry lock is poisoned during shutdown");
            return;
        }
    };
    for job_id in job_ids {
        if let Err(error) = cancel_daemon_job(state, &job_id) {
            tracing::warn!(job_id, error = %error, "failed to cancel daemon job during shutdown");
        }
    }
}

pub(super) async fn drain_active_jobs(state: &DaemonState, timeout: Duration) -> Result<()> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let active = active_job_count(state)?;
        if active == 0 {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            bail!("timed out draining {active} active daemon jobs");
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
