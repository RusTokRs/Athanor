use std::sync::Arc;

use anyhow::Result;

use crate::daemon::{DaemonJob, DaemonJobKind, DaemonJobStatus, DaemonState};
use crate::daemon_job_registry::get as get_daemon_job;
use crate::daemon_job_scheduler::start_cancellable_with_operation;
use crate::daemon_job_state::{begin_or_finish_failed, finish, finish_cancellable_error};
use crate::daemon_operation::context as operation_context;
use crate::{
    GenerationOptions, GenerationReport, HtmlReportOptions, IndexOptions, IndexReport, WikiOptions,
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
        start_cancellable_with_operation(state, DaemonJobKind::Index, description, &operation)?;
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
                    match index_job_result(&report) {
                        Ok(result) => {
                            let _ = finish(
                                &job_state,
                                &job_id_for_task,
                                DaemonJobStatus::Succeeded,
                                Some(result),
                                None,
                            );
                        }
                        Err(error) => {
                            let _ = finish(
                                &job_state,
                                &job_id_for_task,
                                DaemonJobStatus::Failed,
                                None,
                                Some(error.to_string()),
                            );
                        }
                    }
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

fn index_job_result(report: &IndexReport) -> Result<serde_json::Value> {
    serde_json::to_value(report).map_err(anyhow::Error::from)
}

pub(crate) fn start_generate(
    state: &Arc<DaemonState>,
    request_id: &str,
    deadline_unix_ms: Option<u64>,
) -> Result<Option<DaemonJob>> {
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
                    match generation_job_result(&report) {
                        Ok(result) => {
                            let _ = finish(
                                &job_state,
                                &job_id_for_task,
                                DaemonJobStatus::Succeeded,
                                Some(result),
                                None,
                            );
                        }
                        Err(error) => {
                            let _ = finish(
                                &job_state,
                                &job_id_for_task,
                                DaemonJobStatus::Failed,
                                None,
                                Some(error.to_string()),
                            );
                        }
                    }
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

fn generation_job_result(report: &GenerationReport) -> Result<serde_json::Value> {
    serde_json::to_value(report).map_err(anyhow::Error::from)
}

pub(crate) fn start_html_report(
    state: &Arc<DaemonState>,
    request_id: &str,
    deadline_unix_ms: Option<u64>,
) -> Result<Option<DaemonJob>> {
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::{
        GENERATION_SCHEMA_V1, INDEX_METRICS_SCHEMA, INDEX_REPORT_METRICS_SCHEMA,
        INDEX_REPORT_SCHEMA, GenerationMetrics, GenerationStatus, IndexPipelineMetrics,
        IndexReportMetrics,
    };

    use super::*;

    #[test]
    fn daemon_index_result_matches_public_index_report_shape() {
        let report = IndexReport {
            root: PathBuf::from("project"),
            snapshot: "snap-index".to_string(),
            files_indexed: 3,
            output_dir: PathBuf::from("jsonl"),
            changed_files: 2,
            unchanged_files: 1,
            removed_files: 0,
            validation_report: PathBuf::from("validation-report.json"),
            validation_result: None,
            validate_only: false,
            metrics: IndexReportMetrics {
                schema: INDEX_REPORT_METRICS_SCHEMA,
                total_ms: 10,
                pipeline: IndexPipelineMetrics {
                    schema: INDEX_METRICS_SCHEMA,
                    ..IndexPipelineMetrics::default()
                },
                read_model_write_ms: 2,
                validation_result_write_ms: 0,
                index_state_write_ms: 1,
            },
        };

        let direct = serde_json::to_value(&report).expect("serialize direct IndexReport");
        let daemon = index_job_result(&report).expect("serialize daemon IndexReport result");

        assert_eq!(daemon, direct);
        assert_eq!(daemon["schema"], INDEX_REPORT_SCHEMA);
        assert_eq!(daemon["validation_report"], "validation-report.json");
        assert_eq!(daemon["validate_only"], false);
        assert!(daemon.get("metrics").is_some());
    }

    #[test]
    fn daemon_generation_result_matches_public_generation_report_shape() {
        let report = GenerationReport {
            schema: GENERATION_SCHEMA_V1,
            status: GenerationStatus::Published,
            root: PathBuf::from("project"),
            generation: "00000007".to_string(),
            generation_dir: PathBuf::from(".athanor/generated/generations/00000007"),
            current_pointer: PathBuf::from(".athanor/generated/current.json"),
            snapshot: "snap-generation".to_string(),
            entities: 12,
            facts: 18,
            relations: 7,
            diagnostics: 2,
            metrics: GenerationMetrics {
                schema: "athanor.generation_metrics.v1",
                total_ms: 50,
                snapshot_load_ms: 5,
                jsonl_ms: 10,
                wiki_ms: 11,
                html_ms: 12,
                publish_ms: 12,
            },
        };

        let direct = serde_json::to_value(&report).expect("serialize direct GenerationReport");
        let daemon =
            generation_job_result(&report).expect("serialize daemon GenerationReport result");

        assert_eq!(daemon, direct);
        assert_eq!(daemon["schema"], GENERATION_SCHEMA_V1);
        assert_eq!(daemon["status"], "published");
        assert_eq!(daemon["root"], "project");
        assert!(daemon.get("metrics").is_some());
    }
}
