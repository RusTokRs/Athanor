use anyhow::{Result, bail};

use crate::daemon::{DAEMON_JOBS_SCHEMA, DaemonJob, DaemonJobsReport, DaemonState};
use crate::daemon_jobs_support::is_valid_job_id;

pub(super) fn list(state: &DaemonState, limit: usize) -> Result<DaemonJobsReport> {
    let jobs = state
        .jobs
        .lock()
        .map_err(|_| anyhow::anyhow!("daemon job registry lock is poisoned"))?;
    let total = jobs.len();
    let mut returned_jobs = jobs.clone();
    returned_jobs.sort_by(|left, right| {
        right
            .created_at_unix_ms
            .cmp(&left.created_at_unix_ms)
            .then_with(|| left.id.cmp(&right.id))
    });
    returned_jobs.truncate(limit);
    Ok(DaemonJobsReport {
        schema: DAEMON_JOBS_SCHEMA.to_string(),
        total,
        returned: returned_jobs.len(),
        retention_limit: state.max_job_history,
        jobs: returned_jobs,
    })
}

pub(super) fn get(state: &DaemonState, job_id: &str) -> Result<DaemonJob> {
    if !is_valid_job_id(job_id) {
        bail!("daemon job id must use the form job_00000001");
    }
    let jobs = state
        .jobs
        .lock()
        .map_err(|_| anyhow::anyhow!("daemon job registry lock is poisoned"))?;
    jobs.iter()
        .find(|job| job.id == job_id)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("daemon job `{job_id}` was not found"))
}
