use std::sync::Arc;

use anyhow::Result;
use serde::Serialize;

use crate::daemon::{DaemonJob, DaemonJobKind, DaemonJobStatus, DaemonState};
use crate::daemon_job_registry::get as get_daemon_job;
use crate::daemon_job_scheduler::start_cancellable_with_operation;
use crate::daemon_job_state::{begin_or_finish_failed, finish, finish_cancellable_error};
use crate::daemon_operation::context as operation_context;
use crate::{
    GenerationOptions, GenerationReport, HtmlReport, HtmlReportOptions, IndexOptions, IndexReport,
    WikiOptions, WikiReport, generate_project_cancellable_with_composition_and_operation_context,
    index_project_cancellable_with_composition_and_operation_context,
    project_html_report_cancellable_with_composition_and_operation_context,
    project_wiki_cancellable_with_composition_and_operation_context,
};

pub(crate) fn start_index(
    state: &Arc<DaemonState>,
    description: String,
    operation: athanor_core::OperationContext,
) -> Result<DaemonJob> {
    if crate::daemon_job_state::has_active(state, DaemonJobKind::Index)? {
        anyhow::bail!("index job is already queued or running");
    }
    let composition = required_composition(state);
    let (job_id, cancellation) =
        start_cancellable_with_operation(state, DaemonJobKind::Index, description, &operation)?;
    let mut job = get_daemon_job(state, &job_id)?;
    let job_state = Arc::clone(state);
    let job_id_for_task = job_id.clone();
    let root = state.endpoint.root.clone();
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
                        index_project_cancellable_with_composition_and_operation_context(
                            options,
                            cancellation,
                            &composition,
                            operation,
                        )
                        .await
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
                    finish_serialization_result(
                        &job_state,
                        &job_id_for_task,
                        index_job_result(&report),
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

pub(crate) fn index_job_result(report: &IndexReport) -> Result<serde_json::Value> {
    serialize_job_result(report)
}

pub(crate) fn start_generate(
    state: &Arc<DaemonState>,
    request_id: &str,
    deadline_unix_ms: Option<u64>,
) -> Result<Option<DaemonJob>> {
    let composition = required_composition(state);
    let operation = operation_context("generate", request_id, deadline_unix_ms);
    let (job_id, cancellation) = start_cancellable_with_operation(
        state,
        DaemonJobKind::Generate,
        "generate read models".to_string(),
        &operation,
    )?;
    let job = get_daemon_job(state, &job_id).ok();
    let job_state = Arc::clone(state);
    let job_id_for_task = job_id.clone();
    let root = state.endpoint.root.clone();
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
                        generate_project_cancellable_with_composition_and_operation_context(
                            options,
                            cancellation,
                            &composition,
                            operation,
                        )
                        .await
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
                    finish_serialization_result(
                        &job_state,
                        &job_id_for_task,
                        generation_job_result(&report),
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

pub(crate) fn generation_job_result(report: &GenerationReport) -> Result<serde_json::Value> {
    serialize_job_result(report)
}

pub(crate) fn start_html_report(
    state: &Arc<DaemonState>,
    request_id: &str,
    deadline_unix_ms: Option<u64>,
) -> Result<Option<DaemonJob>> {
    let composition = required_composition(state);
    let operation = operation_context("html_report", request_id, deadline_unix_ms);
    let (job_id, cancellation) = start_cancellable_with_operation(
        state,
        DaemonJobKind::HtmlReport,
        "HTML report".to_string(),
        &operation,
    )?;
    let job = get_daemon_job(state, &job_id).ok();
    let job_state = Arc::clone(state);
    let job_id_for_task = job_id.clone();
    let root = state.endpoint.root.clone();
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
                        project_html_report_cancellable_with_composition_and_operation_context(
                            options,
                            cancellation,
                            &composition,
                            operation,
                        )
                        .await
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
                    finish_serialization_result(
                        &job_state,
                        &job_id_for_task,
                        html_report_job_result(&report),
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

pub(crate) fn html_report_job_result(report: &HtmlReport) -> Result<serde_json::Value> {
    serialize_job_result(report)
}

pub(crate) fn start_wiki(
    state: &Arc<DaemonState>,
    request_id: &str,
    deadline_unix_ms: Option<u64>,
) -> Result<Option<DaemonJob>> {
    let composition = required_composition(state);
    let operation = operation_context("wiki", request_id, deadline_unix_ms);
    let (job_id, cancellation) = start_cancellable_with_operation(
        state,
        DaemonJobKind::Wiki,
        "project wiki".to_string(),
        &operation,
    )?;
    let job = get_daemon_job(state, &job_id).ok();
    let job_state = Arc::clone(state);
    let job_id_for_task = job_id.clone();
    let root = state.endpoint.root.clone();
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
                        project_wiki_cancellable_with_composition_and_operation_context(
                            options,
                            cancellation,
                            &composition,
                            operation,
                        )
                        .await
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
                    finish_serialization_result(
                        &job_state,
                        &job_id_for_task,
                        wiki_job_result(&report),
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

pub(crate) fn wiki_job_result(report: &WikiReport) -> Result<serde_json::Value> {
    serialize_job_result(report)
}

fn required_composition(state: &DaemonState) -> crate::RuntimeComposition {
    crate::daemon_queries::composition(state)
}

fn serialize_job_result<T: Serialize>(report: &T) -> Result<serde_json::Value> {
    serde_json::to_value(report).map_err(anyhow::Error::from)
}

fn finish_serialization_result(
    state: &DaemonState,
    job_id: &str,
    result: Result<serde_json::Value>,
) {
    match result {
        Ok(result) => {
            let _ = finish(
                state,
                job_id,
                DaemonJobStatus::Succeeded,
                Some(result),
                None,
            );
        }
        Err(error) => {
            let _ = finish(
                state,
                job_id,
                DaemonJobStatus::Failed,
                None,
                Some(error.to_string()),
            );
        }
    }
}
