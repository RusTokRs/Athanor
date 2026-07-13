use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use notify_debouncer_mini::notify::{
    Config as NotifyConfig, PollWatcher, RecommendedWatcher, RecursiveMode,
};
use notify_debouncer_mini::{
    Config as DebouncerConfig, DebounceEventResult, Debouncer, new_debouncer, new_debouncer_opt,
};
use tokio::sync::mpsc;

use crate::daemon::{DaemonJob, DaemonJobKind, DaemonState};

pub(super) fn start_file_watcher(
    root: &Path,
    debounce: Duration,
    poll: bool,
    watch_tx: mpsc::UnboundedSender<Vec<PathBuf>>,
) -> Result<DaemonWatcher> {
    let root = root.to_path_buf();
    let root_for_handler = root.clone();
    let handler = move |result: DebounceEventResult| match result {
        Ok(events) => {
            let paths = collect_project_source_events(
                &root_for_handler,
                events.into_iter().map(|event| event.path),
            );
            if !paths.is_empty() {
                let _ = watch_tx.send(paths);
            }
        }
        Err(error) => tracing::warn!(error = %error, "daemon file watcher event error"),
    };
    if poll {
        let config = DebouncerConfig::default()
            .with_timeout(debounce)
            .with_notify_config(NotifyConfig::default().with_poll_interval(debounce));
        let mut debouncer = new_debouncer_opt::<_, PollWatcher>(config, handler)
            .context("failed to create polling daemon file watcher")?;
        debouncer
            .watcher()
            .watch(&root, RecursiveMode::Recursive)
            .with_context(|| format!("failed to watch {}", root.display()))?;
        Ok(DaemonWatcher::Poll {
            _debouncer: debouncer,
        })
    } else {
        let mut debouncer =
            new_debouncer(debounce, handler).context("failed to create daemon file watcher")?;
        debouncer
            .watcher()
            .watch(&root, RecursiveMode::Recursive)
            .with_context(|| format!("failed to watch {}", root.display()))?;
        Ok(DaemonWatcher::Recommended {
            _debouncer: debouncer,
        })
    }
}

#[derive(Debug)]
pub(super) enum DaemonWatcher {
    Recommended {
        _debouncer: Debouncer<RecommendedWatcher>,
    },
    Poll {
        _debouncer: Debouncer<PollWatcher>,
    },
}

pub(super) fn is_project_source_event(root: &Path, path: &Path) -> bool {
    let relative = path
        .strip_prefix(root)
        .or_else(|_| path.strip_prefix("."))
        .unwrap_or(path);
    relative
        .components()
        .next()
        .is_none_or(|component| component.as_os_str() != ".athanor")
}

pub(super) fn collect_project_source_events(
    root: &Path,
    paths: impl IntoIterator<Item = PathBuf>,
) -> Vec<PathBuf> {
    let mut paths = paths
        .into_iter()
        .filter(|path| is_project_source_event(root, path))
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    paths
}

pub(super) fn start_watcher_index_job(
    state: &Arc<DaemonState>,
    paths: Vec<PathBuf>,
) -> Result<Option<DaemonJob>> {
    if crate::daemon::has_active_job(state, DaemonJobKind::Index)? {
        tracing::info!(project_id = %state.endpoint.project_id, changed_paths = paths.len(), "daemon watcher skipped index because one is already queued or running");
        return Ok(None);
    }
    crate::daemon::start_index_job(
        state,
        format!("watch index after {} changed paths", paths.len()),
    )
    .map(Some)
}
