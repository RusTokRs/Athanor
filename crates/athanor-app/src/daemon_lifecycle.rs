use anyhow::Result;

use crate::daemon::{DaemonJobStatus, DaemonLifecycleState, DaemonState};

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
