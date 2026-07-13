use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

use crate::daemon::{DaemonJob, DaemonJobStatus};

pub(super) fn is_valid_job_id(job_id: &str) -> bool {
    job_id.len() == 12
        && job_id.starts_with("job_")
        && job_id.as_bytes().iter().skip(4).all(u8::is_ascii_digit)
}

pub(super) fn unix_time_ms() -> Result<u128> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before Unix epoch")?
        .as_millis())
}

pub(super) fn prune(jobs: &mut Vec<DaemonJob>, max_job_history: usize) {
    while jobs.len() > max_job_history {
        let Some((index, _)) = jobs
            .iter()
            .enumerate()
            .filter(|(_, job)| {
                matches!(
                    job.status,
                    DaemonJobStatus::Succeeded
                        | DaemonJobStatus::Failed
                        | DaemonJobStatus::Cancelled
                )
            })
            .min_by(|(_, left), (_, right)| {
                left.created_at_unix_ms
                    .cmp(&right.created_at_unix_ms)
                    .then_with(|| left.id.cmp(&right.id))
            })
        else {
            break;
        };
        jobs.remove(index);
    }
}
