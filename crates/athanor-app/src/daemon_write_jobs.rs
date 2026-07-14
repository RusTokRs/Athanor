use std::sync::Arc;

use anyhow::Result;

use crate::daemon::{DaemonJob, DaemonJobKind, DaemonJobStatus, DaemonState};
use crate::daemon_job_registry::get as get_daemon_job;
use crate::daemon_job_scheduler::start_cancellable;
use crate::daemon_job_state::{begin_or_finish_failed, finish, finish_cancellable_error};
use crate::daemon_operation::context as operation_context;
use crate::{
    GenerationOptions, HtmlReportOptions, IndexOptions, WikiOptions,
    generate_project_cancellable_with_composition_and_operation_context,
    generate_project_cancellable_with_operation_context,
    index_project_cancellable_with_composition_and_operation_context,
    index_project_cancellable_with_operation_context,
    project_html_report_cancellable_with_composition_and_operation_context,
    project_html_report_cancellable_with_operation_context,
    project_wiki_cancellable_with_composition_and_operation_context,
    project_wiki_cancellable_with_operation_context,
};

pub(crate) fn start_index(
    state: &Arc<DaemonState>,
    description: String,
    operation: athanor_core::OperationContext,
) -> Result<DaemonJob> {
    if crate::daemon_job_state::has_active(state, DaemonJobKind::Index)? {
        anyhow::bail!("index job is already queued or running");
    }
    let (job_id, cancellation) =
        start_cancellable(state, DaemonJobKind::Index, description, &operation)?;
    let mut job = get_daemon_job(state, &job_id)?;
    let job_state = Arc::clone(state);
    let job_id_for_task = job_id.clone();
    let root = state.endpoint.root.clone();
    let composition = crate::daemon_queries::composition(state);
    if let Err(error) = std::thread::Builder::new()
        .name(format!("athd-index-{job_id_for_task}"))
        .spawn(move || {
            if !begin_or_finish_failed(&job_state, &job_id_for_task) {
                return;
            }
            let result = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(anyhow::Error::from)
                .and_then(|runtime| {
                    runtime.block_on(async move {
                        let options = IndexOptions {
                            root,
                            validation_report: None,
                            validation_result: None,
                            validate_only: false,
                        };
                        match composition {
                            Some(composition) => {
                                index_project_cancellable_with_composition_and_operation_context(
                                    options,
                                    cancellation,
                                    &composition,
                                    operation,
                                )
                                .await
                            }
                            None => {
                                index_project_cancellable_with_operation_context(
                                    options,
                                    cancellation,
                                    operation,
                                )
                                .await
                            }
                        }
                    })
                });
            match result {
                Ok(report) => {
                    crate::daemon_queries::invalidate(&job_state);
                    if let Ok(mut snapshot) = job_state.last_successful_index.lock() {
                        *snapshot = Some(report.snapshot.clone());
                    }
                    tracing::info!(
                        job_id = %job_id_for_task,
                        snapshot = %report.snapshot,
                        files_indexed = report.files_indexed,
                        "daemon index job completed"
                    );
                    let _ = finish(
                        &job_state,
                        &job_id_for_task,
                        DaemonJobStatus::Succeeded,
                        Some(serde_json::json!({
                            "snapshot": report.snapshot,
                            "files_indexed": report.files_indexed,
                            "changed_files": report.changed_files,
                            "unchanged_files": report.unchanged_files,
                            "removed_files": report.removed_files,
                            "output_dir": report.output_dir,
                            "metrics": report.metrics,
                        })),
                        None,
                    );
                }
                Err(error) => {
                    let _ = finish_cancellable_error(&job_state, &job_id_for_task, error);
                }
            }
        })
    {
        let error_message = error.to_string();
        let _ = finish(
            state,
            &job_id,
            DaemonJobStatus::Failed,
            None,
            Some(error_message),
        );
        job = get_daemon_job(state, &job_id)?;
    }
    Ok(job)
}

pub(crate) fn start_generate(
    state: &Arc<DaemonState>,
    request_id: &str,
    deadline_unix_ms: Option<u64>,
) -> Result<Option<DaemonJob>> {
    let operation = operation_context("generate", request_id, deadline_unix_ms);
    let (job_id, cancellation) = start_cancellable(
        state,
        DaemonJobKind::Generate,
        "generate read models".to_string(),
        &operation,
    )?;
    let job = get_daemon_job(state, &job_id).ok();
    let job_state = Arc::clone(state);
    let job_id_for_task = job_id.clone();
    let root = state.endpoint.root.clone();
    let composition = crate::daemon_queries::composition(state);
    if let Err(error) = std::thread::Builder::new()
        .name(format!("athd-generate-{job_id_for_task}"))
        .spawn(move || {
            if !begin_or_finish_failed(&job_state, &job_id_for_task) {
                return;
            }
            let result = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(anyhow::Error::from)
                .and_then(|runtime| {
                    runtime.block_on(async move {
                        let options = GenerationOptions { root, force: false };
                        match composition {
                            Some(composition) => {
                                generate_project_cancellable_with_composition_and_operation_context(
                                    options,
                                    cancellation,
                                    &composition,
                                    operation,
                                )
                                .await
                            }
                            None => {
                                generate_project_cancellable_with_operation_context(
                                    options,
                                    cancellation,
                                    operation,
                                )
                                .await
                            }
                        }
                    })
                });
            match result {
                Ok(report) => {
                    tracing::info!(
                        job_id = %job_id_for_task,
                        generation = %report.generation,
                        snapshot = %report.snapshot,
                        "daemon generate job completed"
                    );
                    let _ = finish(
                        &job_state,
                        &job_id_for_task,
                        DaemonJobStatus::Succeeded,
                        Some(serde_json::json!({
                            "generation": report.generation,
                            "generation_dir": report.generation_dir,
                            "current_pointer": report.current_pointer,
                            "snapshot": report.snapshot,
                            "entities": report.entities,
                            "facts": report.facts,
                            "relations": report.relations,
                            "diagnostics": report.diagnostics,
                        })),
                        None,
                    );
                }
                Err(error) => {
                    let _ = finish_cancellable_error(&job_state, &job_id_for_task, error);
                }
            }
        })
    {
        let _ = finish(
            state,
            &job_id,
            DaemonJobStatus::Failed,
            None,
            Some(error.to_string()),
        );
    }
    Ok(job)
}

pub(crate) fn start_html_report(
    state: &Arc<DaemonState>,
    request_id: &str,
    deadline_unix_ms: Option<u64>,
) -> Result<Option<DaemonJob>> {
    let operation = operation_context("html_report", request_id, deadline_unix_ms);
    let (job_id, cancellation) = start_cancellable(
        state,
        DaemonJobKind::HtmlReport,
        "HTML report".to_string(),
        &operation,
    )?;
    let job = get_daemon_job(state, &job_id).ok();
    let job_state = Arc::clone(state);
    let job_id_for_task = job_id.clone();
    let root = state.endpoint.root.clone();
    let composition = crate::daemon_queries::composition(state);
    if let Err(error) = std::thread::Builder::new()
        .name(format!("athd-html-report-{job_id_for_task}"))
        .spawn(move || {
            if !begin_or_finish_failed(&job_state, &job_id_for_task) {
                return;
            }
            let result = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(anyhow::Error::from)
                .and_then(|runtime| {
                    runtime.block_on(async move {
                        let options = HtmlReportOptions { root, output: None };
                        match composition {
                            Some(composition) => {
                                project_html_report_cancellable_with_composition_and_operation_context(
                                    options,
                                    cancellation,
                                    &composition,
                                    operation,
                                )
                                .await
                            }
                            None => {
                                project_html_report_cancellable_with_operation_context(
                                    options,
                                    cancellation,
                                    operation,
                                )
                                .await
                            }
                        }
                    })
                });
            match result {
                Ok(report) => {
                    tracing::info!(
                        job_id = %job_id_for_task,
                        snapshot = %report.snapshot,
                        output = %report.output_dir.display(),
                        "daemon HTML report job completed"
                    );
                    let _ = finish(
                        &job_state,
                        &job_id_for_task,
                        DaemonJobStatus::Succeeded,
                        Some(serde_json::json!({
                            "snapshot": report.snapshot,
                            "output_dir": report.output_dir,
                            "entities": report.entities,
                            "facts": report.facts,
                            "relations": report.relations,
                            "open_diagnostics": report.open_diagnostics,
                        })),
                        None,
                    );
                }
                Err(error) => {
                    let _ = finish_cancellable_error(&job_state, &job_id_for_task, error);
                }
            }
        })
    {
        let _ = finish(
            state,
            &job_id,
            DaemonJobStatus::Failed,
            None,
            Some(error.to_string()),
        );
    }
    Ok(job)
}

pub(crate) fn start_wiki(
    state: &Arc<DaemonState>,
    request_id: &str,
    deadline_unix_ms: Option<u64>,
) -> Result<Option<DaemonJob>> {
    let operation = operation_context("wiki", request_id, deadline_unix_ms);
    let (job_id, cancellation) = start_cancellable(
        state,
        DaemonJobKind::Wiki,
        "project wiki".to_string(),
        &operation,
    )?;
    let job = get_daemon_job(state, &job_id).ok();
    let job_state = Arc::clone(state);
    let job_id_for_task = job_id.clone();
    let root = state.endpoint.root.clone();
    let composition = crate::daemon_queries::composition(state);
    if let Err(error) = std::thread::Builder::new()
        .name(format!("athd-wiki-{job_id_for_task}"))
        .spawn(move || {
            if !begin_or_finish_failed(&job_state, &job_id_for_task) {
                return;
            }
            let result = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(anyhow::Error::from)
                .and_then(|runtime| {
                    runtime.block_on(async move {
                        let options = WikiOptions { root, output: None };
                        match composition {
                            Some(composition) => {
                                project_wiki_cancellable_with_composition_and_operation_context(
                                    options,
                                    cancellation,
                                    &composition,
                                    operation,
                                )
                                .await
                            }
                            None => {
                                project_wiki_cancellable_with_operation_context(
                                    options,
                                    cancellation,
                                    operation,
                                )
                                .await
                            }
                        }
                    })
                });
            match result {
                Ok(report) => {
                    tracing::info!(
                        job_id = %job_id_for_task,
                        snapshot = %report.snapshot,
                        output = %report.output_dir.display(),
                        "daemon wiki job completed"
                    );
                    let _ = finish(
                        &job_state,
                        &job_id_for_task,
                        DaemonJobStatus::Succeeded,
                        Some(serde_json::json!({
                            "snapshot": report.snapshot,
                            "output_dir": report.output_dir,
                            "entities": report.entities,
                            "facts": report.facts,
                            "relations": report.relations,
                            "open_diagnostics": report.open_diagnostics,
                        })),
                        None,
                    );
                }
                Err(error) => {
                    let _ = finish_cancellable_error(&job_state, &job_id_for_task, error);
                }
            }
        })
    {
        let _ = finish(
            state,
            &job_id,
            DaemonJobStatus::Failed,
            None,
            Some(error.to_string()),
        );
    }
    Ok(job)
}
