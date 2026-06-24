use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use notify_debouncer_mini::notify::{
    Config as NotifyConfig, PollWatcher, RecommendedWatcher, RecursiveMode,
};
use notify_debouncer_mini::{
    Config as DebouncerConfig, DebounceEventResult, Debouncer, new_debouncer, new_debouncer_opt,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Semaphore, mpsc};

use athanor_core::{CanonicalSnapshot, CanonicalSnapshotStore, SearchIndex};
use athanor_domain::ContextLevel;

use crate::explain::explain_snapshot;
use crate::{
    ContextLimitOverrides, ContextLimits, ContextOptions, GenerationOptions, HtmlReportOptions,
    IndexOptions, WikiOptions, build_repository_overview, context_project, generate_context_pack,
    generate_project, index_project, project_html_report, project_wiki,
};
use crate::{config::load_config, store::init_store};

pub const DAEMON_ENDPOINT_SCHEMA: &str = "athanor.daemon_endpoint.v1";
pub const DAEMON_REQUEST_SCHEMA: &str = "athanor.daemon_request.v1";
pub const DAEMON_RESPONSE_SCHEMA: &str = "athanor.daemon_response.v1";
pub const DAEMON_JOBS_SCHEMA: &str = "athanor.daemon_jobs.v1";
const DEFAULT_MAX_REQUEST_BYTES: u64 = 1024 * 1024;
const DEFAULT_MAX_RESPONSE_BYTES: u64 = 1024 * 1024;
const MIN_PROTOCOL_BYTES: u64 = 1024;
const MAX_PROTOCOL_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct DaemonServeOptions {
    pub project_id: String,
    pub root: PathBuf,
    pub registry_path: PathBuf,
    pub listen: SocketAddr,
    pub max_concurrent_requests: usize,
    pub max_job_history: usize,
    pub watch: bool,
    pub watch_poll: bool,
    pub debounce_ms: u64,
    pub max_request_bytes: u64,
    pub max_response_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct DaemonClientOptions {
    pub root: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DaemonEndpoint {
    pub schema: String,
    pub project_id: String,
    pub root: PathBuf,
    pub registry_path: PathBuf,
    pub address: SocketAddr,
    pub pid: u32,
    pub started_at_unix_ms: u128,
    pub max_concurrent_requests: usize,
    pub max_job_history: usize,
    #[serde(default)]
    pub watch: bool,
    #[serde(default)]
    pub watch_poll: bool,
    #[serde(default)]
    pub debounce_ms: u64,
    #[serde(default = "default_max_request_bytes")]
    pub max_request_bytes: u64,
    #[serde(default = "default_max_response_bytes")]
    pub max_response_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DaemonRequest {
    pub schema: String,
    pub request_id: String,
    pub project_id: String,
    pub command: DaemonCommand,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "name", rename_all = "snake_case")]
pub enum DaemonCommand {
    Status,
    Jobs {
        limit: usize,
    },
    Job {
        job_id: String,
    },
    Cancel {
        job_id: String,
    },
    Index,
    Generate,
    Wiki,
    HtmlReport,
    Overview {
        top: usize,
    },
    Explain {
        stable_key: String,
    },
    Context {
        task: String,
        #[serde(default)]
        diff: bool,
        level: ContextLevel,
        limits: ContextLimitOverrides,
    },
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DaemonResponse {
    pub schema: String,
    pub request_id: String,
    pub project_id: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DaemonJobKind {
    DaemonLifecycle,
    Index,
    Generate,
    Wiki,
    HtmlReport,
    Overview,
    Explain,
    Context,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DaemonJobStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DaemonJob {
    pub id: String,
    pub kind: DaemonJobKind,
    pub status: DaemonJobStatus,
    pub description: String,
    pub created_at_unix_ms: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at_unix_ms: Option<u128>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at_unix_ms: Option<u128>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DaemonJobsReport {
    pub schema: String,
    pub total: usize,
    pub returned: usize,
    pub retention_limit: usize,
    pub jobs: Vec<DaemonJob>,
}

#[derive(Debug)]
struct DaemonState {
    endpoint: DaemonEndpoint,
    jobs: Mutex<Vec<DaemonJob>>,
    next_job_sequence: Mutex<u64>,
    max_job_history: usize,
    latest_snapshot_cache: Mutex<Option<CanonicalSnapshot>>,
}

pub async fn serve_daemon(options: DaemonServeOptions) -> Result<()> {
    if options.max_concurrent_requests == 0 || options.max_concurrent_requests > 128 {
        bail!("daemon max_concurrent_requests must be between 1 and 128");
    }
    if options.max_job_history == 0 || options.max_job_history > 10_000 {
        bail!("daemon max_job_history must be between 1 and 10000");
    }
    if options.watch && (options.debounce_ms < 100 || options.debounce_ms > 60_000) {
        bail!("daemon debounce_ms must be between 100 and 60000");
    }
    if options.watch_poll && !options.watch {
        bail!("daemon watch_poll requires --watch");
    }
    validate_protocol_limit("max_request_bytes", options.max_request_bytes)?;
    validate_protocol_limit("max_response_bytes", options.max_response_bytes)?;
    let root = options.root.canonicalize().with_context(|| {
        format!(
            "failed to canonicalize daemon root {}",
            options.root.display()
        )
    })?;
    let runtime_dir = root.join(".athanor/daemon");
    fs::create_dir_all(&runtime_dir)
        .with_context(|| format!("failed to create {}", runtime_dir.display()))?;
    let lock_path = runtime_dir.join("lock");
    let endpoint_path = runtime_dir.join("endpoint.json");
    let _lock = DaemonLock::acquire(&lock_path, &options.project_id)?;

    let listener = TcpListener::bind(options.listen)
        .await
        .with_context(|| format!("failed to bind daemon listener {}", options.listen))?;
    let address = listener.local_addr()?;
    let endpoint = DaemonEndpoint {
        schema: DAEMON_ENDPOINT_SCHEMA.to_string(),
        project_id: options.project_id.clone(),
        root: root.clone(),
        registry_path: options.registry_path,
        address,
        pid: std::process::id(),
        started_at_unix_ms: unix_time_ms()?,
        max_concurrent_requests: options.max_concurrent_requests,
        max_job_history: options.max_job_history,
        watch: options.watch,
        watch_poll: options.watch_poll,
        debounce_ms: options.debounce_ms,
        max_request_bytes: options.max_request_bytes,
        max_response_bytes: options.max_response_bytes,
    };
    write_endpoint(&endpoint_path, &endpoint)?;
    let _endpoint_guard = EndpointGuard(endpoint_path.clone());
    let state = Arc::new(DaemonState {
        jobs: Mutex::new(vec![DaemonJob {
            id: "job_00000001".to_string(),
            kind: DaemonJobKind::DaemonLifecycle,
            status: DaemonJobStatus::Succeeded,
            description: "daemon started".to_string(),
            created_at_unix_ms: endpoint.started_at_unix_ms,
            started_at_unix_ms: Some(endpoint.started_at_unix_ms),
            finished_at_unix_ms: Some(endpoint.started_at_unix_ms),
            result: None,
            error: None,
        }]),
        next_job_sequence: Mutex::new(2),
        max_job_history: options.max_job_history,
        latest_snapshot_cache: Mutex::new(None),
        endpoint,
    });
    let request_slots = Arc::new(Semaphore::new(options.max_concurrent_requests));
    let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
    let (watch_tx, mut watch_rx) = mpsc::unbounded_channel::<Vec<PathBuf>>();
    let _watch_tx_guard = watch_tx.clone();
    let mut _watcher = if options.watch {
        Some(start_file_watcher(
            &root,
            Duration::from_millis(options.debounce_ms),
            options.watch_poll,
            watch_tx,
        )?)
    } else {
        None
    };
    tracing::info!(
        project_id = %state.endpoint.project_id,
        root = %root.display(),
        address = %address,
        watch = options.watch,
        watch_poll = options.watch_poll,
        debounce_ms = options.debounce_ms,
        max_request_bytes = options.max_request_bytes,
        max_response_bytes = options.max_response_bytes,
        "Athanor daemon listening"
    );

    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let (stream, peer) = accepted.context("failed to accept daemon connection")?;
                match request_slots.clone().try_acquire_owned() {
                    Ok(permit) => {
                        let state = Arc::clone(&state);
                        let shutdown_tx = shutdown_tx.clone();
                        tokio::spawn(async move {
                            let _permit = permit;
                            match handle_connection(stream, state).await {
                                Ok(true) => {
                                    let _ = shutdown_tx.try_send(());
                                }
                                Ok(false) => {}
                                Err(error) => {
                                    tracing::warn!(%peer, error = %error, "failed to handle daemon request");
                                }
                            }
                        });
                    }
                    Err(_) => {
                        let state = Arc::clone(&state);
                        tokio::spawn(async move {
                            if let Err(error) = handle_busy_connection(stream, &state.endpoint).await {
                                tracing::warn!(%peer, error = %error, "failed to reject busy daemon request");
                            }
                        });
                    }
                }
            }
            shutdown = shutdown_rx.recv() => {
                if shutdown.is_some() {
                    break;
                }
            }
            events = watch_rx.recv() => {
                if let Some(paths) = events {
                    match start_watcher_index_job(&state, paths) {
                        Ok(Some(job)) => {
                            tracing::info!(
                                project_id = %state.endpoint.project_id,
                                job_id = %job.id,
                                "daemon watcher scheduled index job"
                            );
                        }
                        Ok(None) => {}
                        Err(error) => {
                            tracing::warn!(
                                project_id = %state.endpoint.project_id,
                                error = %error,
                                "daemon watcher failed to schedule index job"
                            );
                        }
                    }
                }
            }
            signal = tokio::signal::ctrl_c() => {
                signal.context("failed to listen for daemon shutdown signal")?;
                break;
            }
        }
    }

    tracing::info!(project_id = %state.endpoint.project_id, "Athanor daemon stopped");
    Ok(())
}

pub async fn request_daemon(
    options: DaemonClientOptions,
    request: DaemonRequest,
) -> Result<DaemonResponse> {
    validate_request(&request)?;
    let endpoint = read_endpoint(&options.root.join(".athanor/daemon/endpoint.json"))?;
    if endpoint.project_id != request.project_id {
        bail!(
            "daemon endpoint belongs to project `{}`, not `{}`",
            endpoint.project_id,
            request.project_id
        );
    }
    let mut stream = TcpStream::connect(endpoint.address)
        .await
        .with_context(|| format!("failed to connect to daemon at {}", endpoint.address))?;
    let request_json = serde_json::to_vec(&request)?;
    if request_json.len() as u64 > endpoint.max_request_bytes {
        bail!(
            "daemon request exceeds {} bytes",
            endpoint.max_request_bytes
        );
    }
    stream.write_all(&request_json).await?;
    stream.write_all(b"\n").await?;
    stream.shutdown().await?;

    let mut response = Vec::new();
    stream
        .take(endpoint.max_response_bytes + 1)
        .read_to_end(&mut response)
        .await
        .context("failed to read daemon response")?;
    if response.len() as u64 > endpoint.max_response_bytes {
        bail!(
            "daemon response exceeds {} bytes",
            endpoint.max_response_bytes
        );
    }
    if response.is_empty() {
        bail!("daemon returned an empty response");
    }
    serde_json::from_slice(&response).context("failed to parse daemon response")
}

async fn handle_connection(stream: TcpStream, state: Arc<DaemonState>) -> Result<bool> {
    let (read_half, mut write_half) = stream.into_split();
    let mut line = String::new();
    let bytes = BufReader::new(read_half)
        .take(state.endpoint.max_request_bytes + 1)
        .read_line(&mut line)
        .await
        .context("failed to read daemon request")?;
    let (response, shutdown) = if bytes == 0 {
        (
            error_response("", &state.endpoint.project_id, "empty daemon request"),
            false,
        )
    } else if bytes as u64 > state.endpoint.max_request_bytes {
        (
            error_response(
                "",
                &state.endpoint.project_id,
                "daemon request exceeds size limit",
            ),
            false,
        )
    } else {
        match serde_json::from_str::<DaemonRequest>(&line) {
            Ok(request) => execute_request(Arc::clone(&state), request).await,
            Err(error) => (
                error_response(
                    "",
                    &state.endpoint.project_id,
                    &format!("invalid daemon request JSON: {error}"),
                ),
                false,
            ),
        }
    };
    let response_json = serialize_daemon_response(response, state.endpoint.max_response_bytes)?;
    write_half
        .write_all(&response_json)
        .await
        .context("failed to write daemon response")?;
    write_half.shutdown().await?;
    Ok(shutdown)
}

async fn handle_busy_connection(stream: TcpStream, endpoint: &DaemonEndpoint) -> Result<()> {
    let (read_half, mut write_half) = stream.into_split();
    let mut line = String::new();
    let _ = BufReader::new(read_half)
        .take(endpoint.max_request_bytes + 1)
        .read_line(&mut line)
        .await;
    let request_id = serde_json::from_str::<DaemonRequest>(&line)
        .map(|request| request.request_id)
        .unwrap_or_default();
    let response = error_response(
        &request_id,
        &endpoint.project_id,
        "daemon is busy; maximum concurrent request limit reached",
    );
    write_half
        .write_all(&serialize_daemon_response(
            response,
            endpoint.max_response_bytes,
        )?)
        .await
        .context("failed to write daemon busy response")?;
    write_half.shutdown().await?;
    Ok(())
}

async fn execute_request(
    state: Arc<DaemonState>,
    request: DaemonRequest,
) -> (DaemonResponse, bool) {
    if let Err(error) = validate_request(&request) {
        return (
            error_response(
                &request.request_id,
                &state.endpoint.project_id,
                &error.to_string(),
            ),
            false,
        );
    }
    if request.project_id != state.endpoint.project_id {
        return (
            error_response(
                &request.request_id,
                &state.endpoint.project_id,
                &format!(
                    "request project `{}` does not match daemon project `{}`",
                    request.project_id, state.endpoint.project_id
                ),
            ),
            false,
        );
    }

    match request.command {
        DaemonCommand::Status => (
            success_response(
                &request.request_id,
                &state.endpoint.project_id,
                serde_json::json!({
                    "status": "running",
                    "endpoint": state.endpoint,
                }),
            ),
            false,
        ),
        DaemonCommand::Jobs { limit } => {
            if limit == 0 || limit > 100 {
                return (
                    error_response(
                        &request.request_id,
                        &state.endpoint.project_id,
                        "jobs limit must be between 1 and 100",
                    ),
                    false,
                );
            }
            match list_daemon_jobs(&state, limit) {
                Ok(report) => (
                    success_response(
                        &request.request_id,
                        &state.endpoint.project_id,
                        serde_json::to_value(report).unwrap_or(Value::Null),
                    ),
                    false,
                ),
                Err(error) => (
                    error_response(
                        &request.request_id,
                        &state.endpoint.project_id,
                        &error.to_string(),
                    ),
                    false,
                ),
            }
        }
        DaemonCommand::Job { job_id } => match get_daemon_job(&state, &job_id) {
            Ok(job) => (
                success_response(
                    &request.request_id,
                    &state.endpoint.project_id,
                    serde_json::to_value(job).unwrap_or(Value::Null),
                ),
                false,
            ),
            Err(error) => (
                error_response(
                    &request.request_id,
                    &state.endpoint.project_id,
                    &error.to_string(),
                ),
                false,
            ),
        },
        DaemonCommand::Cancel { job_id } => match cancel_daemon_job(&state, &job_id) {
            Ok(job) => (
                success_response(
                    &request.request_id,
                    &state.endpoint.project_id,
                    serde_json::to_value(job).unwrap_or(Value::Null),
                ),
                false,
            ),
            Err(error) => (
                error_response(
                    &request.request_id,
                    &state.endpoint.project_id,
                    &error.to_string(),
                ),
                false,
            ),
        },
        DaemonCommand::Index => match start_index_job(&state, "index project".to_string()) {
            Ok(job) => (
                success_response(
                    &request.request_id,
                    &state.endpoint.project_id,
                    serde_json::to_value(job).unwrap_or(Value::Null),
                ),
                false,
            ),
            Err(error) => (
                error_response(
                    &request.request_id,
                    &state.endpoint.project_id,
                    &error.to_string(),
                ),
                false,
            ),
        },
        DaemonCommand::Generate => {
            if has_active_job(&state, DaemonJobKind::Generate).unwrap_or(false) {
                return (
                    error_response(
                        &request.request_id,
                        &state.endpoint.project_id,
                        "generate job is already queued or running",
                    ),
                    false,
                );
            }
            match start_daemon_job(
                &state,
                DaemonJobKind::Generate,
                "generate read models".to_string(),
            ) {
                Ok(job_id) => {
                    let job = get_daemon_job(&state, &job_id).ok();
                    let job_state = Arc::clone(&state);
                    let job_id_for_task = job_id.clone();
                    let root = state.endpoint.root.clone();
                    if let Err(error) = std::thread::Builder::new()
                        .name(format!("athd-generate-{job_id_for_task}"))
                        .spawn(move || {
                            if !begin_daemon_job_or_finish_failed(&job_state, &job_id_for_task) {
                                return;
                            }
                            let result = tokio::runtime::Builder::new_current_thread()
                                .enable_all()
                                .build()
                                .map_err(anyhow::Error::from)
                                .and_then(|runtime| {
                                    runtime.block_on(generate_project(GenerationOptions { root }))
                                });
                            match result {
                                Ok(report) => {
                                    tracing::info!(
                                        job_id = %job_id_for_task,
                                        generation = %report.generation,
                                        snapshot = %report.snapshot,
                                        "daemon generate job completed"
                                    );
                                    let _ = finish_daemon_job(
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
                                    let _ = finish_daemon_job(
                                        &job_state,
                                        &job_id_for_task,
                                        DaemonJobStatus::Failed,
                                        None,
                                        Some(error.to_string()),
                                    );
                                }
                            }
                        })
                    {
                        let _ = finish_daemon_job(
                            &state,
                            &job_id,
                            DaemonJobStatus::Failed,
                            None,
                            Some(error.to_string()),
                        );
                    }
                    (
                        success_response(
                            &request.request_id,
                            &state.endpoint.project_id,
                            serde_json::to_value(job).unwrap_or(Value::Null),
                        ),
                        false,
                    )
                }
                Err(error) => (
                    error_response(
                        &request.request_id,
                        &state.endpoint.project_id,
                        &error.to_string(),
                    ),
                    false,
                ),
            }
        }
        DaemonCommand::Wiki => {
            if has_active_job(&state, DaemonJobKind::Wiki).unwrap_or(false) {
                return (
                    error_response(
                        &request.request_id,
                        &state.endpoint.project_id,
                        "wiki job is already queued or running",
                    ),
                    false,
                );
            }
            match start_daemon_job(&state, DaemonJobKind::Wiki, "project wiki".to_string()) {
                Ok(job_id) => {
                    let job = get_daemon_job(&state, &job_id).ok();
                    let job_state = Arc::clone(&state);
                    let job_id_for_task = job_id.clone();
                    let root = state.endpoint.root.clone();
                    if let Err(error) = std::thread::Builder::new()
                        .name(format!("athd-wiki-{job_id_for_task}"))
                        .spawn(move || {
                            if !begin_daemon_job_or_finish_failed(&job_state, &job_id_for_task) {
                                return;
                            }
                            let result = tokio::runtime::Builder::new_current_thread()
                                .enable_all()
                                .build()
                                .map_err(anyhow::Error::from)
                                .and_then(|runtime| {
                                    runtime
                                        .block_on(project_wiki(WikiOptions { root, output: None }))
                                });
                            match result {
                                Ok(report) => {
                                    tracing::info!(
                                        job_id = %job_id_for_task,
                                        snapshot = %report.snapshot,
                                        output = %report.output_dir.display(),
                                        "daemon wiki job completed"
                                    );
                                    let _ = finish_daemon_job(
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
                                    let _ = finish_daemon_job(
                                        &job_state,
                                        &job_id_for_task,
                                        DaemonJobStatus::Failed,
                                        None,
                                        Some(error.to_string()),
                                    );
                                }
                            }
                        })
                    {
                        let _ = finish_daemon_job(
                            &state,
                            &job_id,
                            DaemonJobStatus::Failed,
                            None,
                            Some(error.to_string()),
                        );
                    }
                    (
                        success_response(
                            &request.request_id,
                            &state.endpoint.project_id,
                            serde_json::to_value(job).unwrap_or(Value::Null),
                        ),
                        false,
                    )
                }
                Err(error) => (
                    error_response(
                        &request.request_id,
                        &state.endpoint.project_id,
                        &error.to_string(),
                    ),
                    false,
                ),
            }
        }
        DaemonCommand::HtmlReport => {
            if has_active_job(&state, DaemonJobKind::HtmlReport).unwrap_or(false) {
                return (
                    error_response(
                        &request.request_id,
                        &state.endpoint.project_id,
                        "HTML report job is already queued or running",
                    ),
                    false,
                );
            }
            match start_daemon_job(&state, DaemonJobKind::HtmlReport, "HTML report".to_string()) {
                Ok(job_id) => {
                    let job = get_daemon_job(&state, &job_id).ok();
                    let job_state = Arc::clone(&state);
                    let job_id_for_task = job_id.clone();
                    let root = state.endpoint.root.clone();
                    if let Err(error) = std::thread::Builder::new()
                        .name(format!("athd-html-report-{job_id_for_task}"))
                        .spawn(move || {
                            if !begin_daemon_job_or_finish_failed(&job_state, &job_id_for_task) {
                                return;
                            }
                            let result = tokio::runtime::Builder::new_current_thread()
                                .enable_all()
                                .build()
                                .map_err(anyhow::Error::from)
                                .and_then(|runtime| {
                                    runtime.block_on(project_html_report(HtmlReportOptions {
                                        root,
                                        output: None,
                                    }))
                                });
                            match result {
                                Ok(report) => {
                                    tracing::info!(
                                        job_id = %job_id_for_task,
                                        snapshot = %report.snapshot,
                                        output = %report.output_dir.display(),
                                        "daemon HTML report job completed"
                                    );
                                    let _ = finish_daemon_job(
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
                                    let _ = finish_daemon_job(
                                        &job_state,
                                        &job_id_for_task,
                                        DaemonJobStatus::Failed,
                                        None,
                                        Some(error.to_string()),
                                    );
                                }
                            }
                        })
                    {
                        let _ = finish_daemon_job(
                            &state,
                            &job_id,
                            DaemonJobStatus::Failed,
                            None,
                            Some(error.to_string()),
                        );
                    }
                    (
                        success_response(
                            &request.request_id,
                            &state.endpoint.project_id,
                            serde_json::to_value(job).unwrap_or(Value::Null),
                        ),
                        false,
                    )
                }
                Err(error) => (
                    error_response(
                        &request.request_id,
                        &state.endpoint.project_id,
                        &error.to_string(),
                    ),
                    false,
                ),
            }
        }
        DaemonCommand::Overview { top } => {
            if top == 0 || top > 100 {
                return (
                    error_response(
                        &request.request_id,
                        &state.endpoint.project_id,
                        "overview top must be between 1 and 100",
                    ),
                    false,
                );
            }
            let job_id = match start_daemon_job(
                &state,
                DaemonJobKind::Overview,
                format!("overview top={top}"),
            ) {
                Ok(job_id) => job_id,
                Err(error) => {
                    return (
                        error_response(
                            &request.request_id,
                            &state.endpoint.project_id,
                            &error.to_string(),
                        ),
                        false,
                    );
                }
            };
            match latest_snapshot_for_daemon(&state).await {
                Ok(snapshot) => {
                    let overview = build_repository_overview(&snapshot, top);
                    let _ =
                        finish_daemon_job(&state, &job_id, DaemonJobStatus::Succeeded, None, None);
                    (
                        success_response(
                            &request.request_id,
                            &state.endpoint.project_id,
                            serde_json::to_value(overview).unwrap_or(Value::Null),
                        ),
                        false,
                    )
                }
                Err(error) => {
                    let error_message = error.to_string();
                    let _ = finish_daemon_job(
                        &state,
                        &job_id,
                        DaemonJobStatus::Failed,
                        None,
                        Some(error_message.clone()),
                    );
                    (
                        error_response(
                            &request.request_id,
                            &state.endpoint.project_id,
                            &error_message,
                        ),
                        false,
                    )
                }
            }
        }
        DaemonCommand::Explain { stable_key } => {
            if stable_key.trim().is_empty() {
                return (
                    error_response(
                        &request.request_id,
                        &state.endpoint.project_id,
                        "entity stable key must not be empty",
                    ),
                    false,
                );
            }
            let stable_key = stable_key.trim().to_string();
            let job_id = match start_daemon_job(
                &state,
                DaemonJobKind::Explain,
                format!("explain stable_key={stable_key}"),
            ) {
                Ok(job_id) => job_id,
                Err(error) => {
                    return (
                        error_response(
                            &request.request_id,
                            &state.endpoint.project_id,
                            &error.to_string(),
                        ),
                        false,
                    );
                }
            };
            match daemon_explain_from_cache(&state, &stable_key).await {
                Ok(explanation) => {
                    let _ =
                        finish_daemon_job(&state, &job_id, DaemonJobStatus::Succeeded, None, None);
                    (
                        success_response(
                            &request.request_id,
                            &state.endpoint.project_id,
                            serde_json::to_value(explanation).unwrap_or(Value::Null),
                        ),
                        false,
                    )
                }
                Err(error) => {
                    let error_message = error.to_string();
                    let _ = finish_daemon_job(
                        &state,
                        &job_id,
                        DaemonJobStatus::Failed,
                        None,
                        Some(error_message.clone()),
                    );
                    (
                        error_response(
                            &request.request_id,
                            &state.endpoint.project_id,
                            &error_message,
                        ),
                        false,
                    )
                }
            }
        }
        DaemonCommand::Context {
            task,
            diff,
            level,
            limits,
        } => {
            if task.trim().is_empty() && !diff {
                return (
                    error_response(
                        &request.request_id,
                        &state.endpoint.project_id,
                        "context task must not be empty",
                    ),
                    false,
                );
            }
            let job_id = match start_daemon_job(
                &state,
                DaemonJobKind::Context,
                format!("context task={} diff={diff}", task.trim()),
            ) {
                Ok(job_id) => job_id,
                Err(error) => {
                    return (
                        error_response(
                            &request.request_id,
                            &state.endpoint.project_id,
                            &error.to_string(),
                        ),
                        false,
                    );
                }
            };
            let context_result = if diff {
                context_project(ContextOptions {
                    root: state.endpoint.root.clone(),
                    task,
                    diff,
                    level,
                    limits,
                })
                .await
            } else {
                daemon_context_from_cache(&state, &task, level, &limits).await
            };
            match context_result {
                Ok(pack) => {
                    let _ =
                        finish_daemon_job(&state, &job_id, DaemonJobStatus::Succeeded, None, None);
                    (
                        success_response(
                            &request.request_id,
                            &state.endpoint.project_id,
                            serde_json::to_value(pack).unwrap_or(Value::Null),
                        ),
                        false,
                    )
                }
                Err(error) => {
                    let error_message = error.to_string();
                    let _ = finish_daemon_job(
                        &state,
                        &job_id,
                        DaemonJobStatus::Failed,
                        None,
                        Some(error_message.clone()),
                    );
                    (
                        error_response(
                            &request.request_id,
                            &state.endpoint.project_id,
                            &error_message,
                        ),
                        false,
                    )
                }
            }
        }
        DaemonCommand::Shutdown => (
            success_response(
                &request.request_id,
                &state.endpoint.project_id,
                serde_json::json!({"status": "stopping"}),
            ),
            true,
        ),
    }
}

fn validate_request(request: &DaemonRequest) -> Result<()> {
    if request.schema != DAEMON_REQUEST_SCHEMA {
        bail!("unsupported daemon request schema `{}`", request.schema);
    }
    if request.request_id.is_empty() || request.request_id.len() > 128 {
        bail!("daemon request_id must contain 1-128 characters");
    }
    if request.project_id.is_empty() {
        bail!("daemon project_id must not be empty");
    }
    Ok(())
}

fn start_file_watcher(
    root: &Path,
    debounce: Duration,
    poll: bool,
    watch_tx: mpsc::UnboundedSender<Vec<PathBuf>>,
) -> Result<DaemonWatcher> {
    let root = root.to_path_buf();
    let root_for_handler = root.clone();
    let handler = move |result: DebounceEventResult| match result {
        Ok(events) => {
            let paths = events
                .into_iter()
                .map(|event| event.path)
                .filter(|path| is_project_source_event(&root_for_handler, path))
                .collect::<Vec<_>>();
            if !paths.is_empty() {
                let _ = watch_tx.send(paths);
            }
        }
        Err(error) => {
            tracing::warn!(error = %error, "daemon file watcher event error");
        }
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
enum DaemonWatcher {
    Recommended {
        _debouncer: Debouncer<RecommendedWatcher>,
    },
    Poll {
        _debouncer: Debouncer<PollWatcher>,
    },
}

fn is_project_source_event(root: &Path, path: &Path) -> bool {
    let relative = path
        .strip_prefix(root)
        .or_else(|_| path.strip_prefix("."))
        .unwrap_or(path);
    relative
        .components()
        .next()
        .is_none_or(|component| component.as_os_str() != ".athanor")
}

fn start_watcher_index_job(
    state: &Arc<DaemonState>,
    paths: Vec<PathBuf>,
) -> Result<Option<DaemonJob>> {
    if has_active_job(state, DaemonJobKind::Index)? {
        tracing::info!(
            project_id = %state.endpoint.project_id,
            changed_paths = paths.len(),
            "daemon watcher skipped index because one is already queued or running"
        );
        return Ok(None);
    }
    let description = format!("watch index after {} changed paths", paths.len());
    start_index_job(state, description).map(Some)
}

async fn latest_snapshot_for_daemon(state: &Arc<DaemonState>) -> Result<CanonicalSnapshot> {
    if let Some(snapshot) = state
        .latest_snapshot_cache
        .lock()
        .map_err(|_| anyhow::anyhow!("daemon snapshot cache lock is poisoned"))?
        .clone()
    {
        return Ok(snapshot);
    }

    let root = &state.endpoint.root;
    let config = load_config(root)?;
    let store = init_store(root, &config).await?;
    let snapshot = store
        .load_latest_snapshot()
        .await
        .context("failed to load latest canonical snapshot")?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "no canonical snapshot found; run `ath index {}` first",
                root.display()
            )
        })?;
    *state
        .latest_snapshot_cache
        .lock()
        .map_err(|_| anyhow::anyhow!("daemon snapshot cache lock is poisoned"))? =
        Some(snapshot.clone());
    Ok(snapshot)
}

async fn daemon_context_from_cache(
    state: &Arc<DaemonState>,
    task: &str,
    level: ContextLevel,
    overrides: &ContextLimitOverrides,
) -> Result<athanor_domain::ContextPack> {
    let mut limits = ContextLimits::for_level(level);
    overrides.apply(&mut limits);
    if limits.max_tokens == 0 || limits.max_files == 0 || limits.max_entities == 0 {
        bail!("context token, file, and entity limits must be greater than zero");
    }
    let snapshot = latest_snapshot_for_daemon(state).await?;
    let snapshot_id = snapshot
        .snapshot
        .as_ref()
        .map(|snapshot| snapshot.0.clone())
        .unwrap_or_else(|| "unknown".to_string());
    let index_dir = state
        .endpoint
        .root
        .join(".athanor/generated/current/search");
    let direct_matches = if let Ok(index) =
        crate::search::get_or_build_search_index(&snapshot, &snapshot_id, &index_dir).await
    {
        if let Ok(search_results) = index
            .search(athanor_core::SearchQuery {
                query: task.to_string(),
                limit: limits.max_entities,
            })
            .await
        {
            Some(
                search_results
                    .into_iter()
                    .map(|result| athanor_domain::EntityId(result.id))
                    .collect::<Vec<_>>(),
            )
        } else {
            None
        }
    } else {
        None
    };
    Ok(generate_context_pack(
        &snapshot,
        task,
        level,
        limits,
        direct_matches,
    ))
}

async fn daemon_explain_from_cache(
    state: &Arc<DaemonState>,
    stable_key: &str,
) -> Result<crate::explain::EntityExplanation> {
    let snapshot = latest_snapshot_for_daemon(state).await?;
    explain_snapshot(&snapshot, stable_key)
}

fn invalidate_latest_snapshot_cache(state: &DaemonState) {
    match state.latest_snapshot_cache.lock() {
        Ok(mut cache) => {
            *cache = None;
        }
        Err(_) => {
            tracing::warn!("daemon snapshot cache lock is poisoned");
        }
    }
}

fn start_index_job(state: &Arc<DaemonState>, description: String) -> Result<DaemonJob> {
    if has_active_job(state, DaemonJobKind::Index)? {
        bail!("index job is already queued or running");
    }
    let job_id = start_daemon_job(state, DaemonJobKind::Index, description)?;
    let mut job = get_daemon_job(state, &job_id)?;
    let job_state = Arc::clone(state);
    let job_id_for_task = job_id.clone();
    let root = state.endpoint.root.clone();
    if let Err(error) = std::thread::Builder::new()
        .name(format!("athd-index-{job_id_for_task}"))
        .spawn(move || {
            if !begin_daemon_job_or_finish_failed(&job_state, &job_id_for_task) {
                return;
            }
            let result = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(anyhow::Error::from)
                .and_then(|runtime| {
                    runtime.block_on(index_project(IndexOptions {
                        root,
                        validation_report: None,
                        validation_result: None,
                        validate_only: false,
                    }))
                });
            match result {
                Ok(report) => {
                    invalidate_latest_snapshot_cache(&job_state);
                    tracing::info!(
                        job_id = %job_id_for_task,
                        snapshot = %report.snapshot,
                        files_indexed = report.files_indexed,
                        "daemon index job completed"
                    );
                    let _ = finish_daemon_job(
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
                        })),
                        None,
                    );
                }
                Err(error) => {
                    let _ = finish_daemon_job(
                        &job_state,
                        &job_id_for_task,
                        DaemonJobStatus::Failed,
                        None,
                        Some(error.to_string()),
                    );
                }
            }
        })
    {
        let error_message = error.to_string();
        let _ = finish_daemon_job(
            state,
            &job_id,
            DaemonJobStatus::Failed,
            None,
            Some(error_message.clone()),
        );
        job = get_daemon_job(state, &job_id)?;
    }
    Ok(job)
}

fn list_daemon_jobs(state: &DaemonState, limit: usize) -> Result<DaemonJobsReport> {
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

fn get_daemon_job(state: &DaemonState, job_id: &str) -> Result<DaemonJob> {
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

fn cancel_daemon_job(state: &DaemonState, job_id: &str) -> Result<DaemonJob> {
    if !is_valid_job_id(job_id) {
        bail!("daemon job id must use the form job_00000001");
    }
    let finished_at = unix_time_ms()?;
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
            job.finished_at_unix_ms = Some(finished_at);
            job.error = Some("job cancelled before start".to_string());
            Ok(job.clone())
        }
        DaemonJobStatus::Running => {
            bail!(
                "daemon job `{job_id}` is running and is not cancellable yet; wait for it to finish"
            );
        }
        DaemonJobStatus::Succeeded | DaemonJobStatus::Failed | DaemonJobStatus::Cancelled => {
            Ok(job.clone())
        }
    }
}

fn has_active_job(state: &DaemonState, kind: DaemonJobKind) -> Result<bool> {
    let jobs = state
        .jobs
        .lock()
        .map_err(|_| anyhow::anyhow!("daemon job registry lock is poisoned"))?;
    Ok(jobs.iter().any(|job| {
        job.kind == kind
            && matches!(
                job.status,
                DaemonJobStatus::Queued | DaemonJobStatus::Running
            )
    }))
}

fn is_valid_job_id(job_id: &str) -> bool {
    job_id.len() == 12
        && job_id.starts_with("job_")
        && job_id.as_bytes().iter().skip(4).all(u8::is_ascii_digit)
}

fn start_daemon_job(
    state: &DaemonState,
    kind: DaemonJobKind,
    description: String,
) -> Result<String> {
    let mut next_job_sequence = state
        .next_job_sequence
        .lock()
        .map_err(|_| anyhow::anyhow!("daemon job sequence lock is poisoned"))?;
    let job_id = format!("job_{:08}", *next_job_sequence);
    *next_job_sequence += 1;
    drop(next_job_sequence);

    let now = unix_time_ms()?;
    let mut jobs = state
        .jobs
        .lock()
        .map_err(|_| anyhow::anyhow!("daemon job registry lock is poisoned"))?;
    jobs.push(DaemonJob {
        id: job_id.clone(),
        kind,
        status: DaemonJobStatus::Queued,
        description,
        created_at_unix_ms: now,
        started_at_unix_ms: None,
        finished_at_unix_ms: None,
        result: None,
        error: None,
    });
    prune_daemon_jobs(&mut jobs, state.max_job_history);
    Ok(job_id)
}

fn mark_daemon_job_running(state: &DaemonState, job_id: &str) -> Result<bool> {
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
        DaemonJobStatus::Running => Ok(true),
        DaemonJobStatus::Succeeded | DaemonJobStatus::Failed => Ok(false),
    }
}

fn begin_daemon_job_or_finish_failed(state: &DaemonState, job_id: &str) -> bool {
    match mark_daemon_job_running(state, job_id) {
        Ok(started) => started,
        Err(error) => {
            let _ = finish_daemon_job(
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

fn finish_daemon_job(
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
    prune_daemon_jobs(&mut jobs, state.max_job_history);
    Ok(())
}

fn prune_daemon_jobs(jobs: &mut Vec<DaemonJob>, max_job_history: usize) {
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

fn read_endpoint(path: &Path) -> Result<DaemonEndpoint> {
    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let endpoint: DaemonEndpoint = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    if endpoint.schema != DAEMON_ENDPOINT_SCHEMA {
        bail!("unsupported daemon endpoint schema `{}`", endpoint.schema);
    }
    Ok(endpoint)
}

fn write_endpoint(path: &Path, endpoint: &DaemonEndpoint) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("daemon endpoint has no parent"))?;
    let staging = parent.join(format!(".endpoint.json.tmp-{}", std::process::id()));
    let content = serde_json::to_string_pretty(endpoint)?;
    fs::write(&staging, format!("{content}\n"))
        .with_context(|| format!("failed to write {}", staging.display()))?;
    if path.exists() {
        fs::remove_file(path).with_context(|| format!("failed to replace {}", path.display()))?;
    }
    fs::rename(&staging, path).with_context(|| format!("failed to publish {}", path.display()))
}

fn success_response(request_id: &str, project_id: &str, result: Value) -> DaemonResponse {
    DaemonResponse {
        schema: DAEMON_RESPONSE_SCHEMA.to_string(),
        request_id: request_id.to_string(),
        project_id: project_id.to_string(),
        ok: true,
        result: Some(result),
        error: None,
    }
}

fn error_response(request_id: &str, project_id: &str, error: &str) -> DaemonResponse {
    DaemonResponse {
        schema: DAEMON_RESPONSE_SCHEMA.to_string(),
        request_id: request_id.to_string(),
        project_id: project_id.to_string(),
        ok: false,
        result: None,
        error: Some(error.to_string()),
    }
}

fn serialize_daemon_response(response: DaemonResponse, max_response_bytes: u64) -> Result<Vec<u8>> {
    let response_json = serde_json::to_vec(&response)?;
    if response_json.len() as u64 <= max_response_bytes {
        return Ok(response_json);
    }

    let overflow = error_response(
        &response.request_id,
        &response.project_id,
        &format!(
            "daemon response exceeds size limit of {} bytes",
            max_response_bytes
        ),
    );
    let overflow_json = serde_json::to_vec(&overflow)?;
    if overflow_json.len() as u64 > max_response_bytes {
        bail!("daemon overflow error response exceeds response size limit");
    }
    Ok(overflow_json)
}

fn validate_protocol_limit(name: &str, value: u64) -> Result<()> {
    if !(MIN_PROTOCOL_BYTES..=MAX_PROTOCOL_BYTES).contains(&value) {
        bail!("{name} must be between {MIN_PROTOCOL_BYTES} and {MAX_PROTOCOL_BYTES}");
    }
    Ok(())
}

fn default_max_request_bytes() -> u64 {
    DEFAULT_MAX_REQUEST_BYTES
}

fn default_max_response_bytes() -> u64 {
    DEFAULT_MAX_RESPONSE_BYTES
}

fn unix_time_ms() -> Result<u128> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before Unix epoch")?
        .as_millis())
}

struct DaemonLock {
    path: PathBuf,
    file: Option<File>,
}

impl DaemonLock {
    fn acquire(path: &Path, project_id: &str) -> Result<Self> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .with_context(|| {
                format!(
                    "daemon lock already exists at {}; another daemon may be running",
                    path.display()
                )
            })?;
        writeln!(
            file,
            "{}",
            serde_json::json!({
                "project_id": project_id,
                "pid": std::process::id(),
            })
        )?;
        Ok(Self {
            path: path.to_path_buf(),
            file: Some(file),
        })
    }
}

impl Drop for DaemonLock {
    fn drop(&mut self) {
        self.file.take();
        let _ = fs::remove_file(&self.path);
    }
}

struct EndpointGuard(PathBuf);

impl Drop for EndpointGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn serves_status_and_rejects_wrong_project() {
        let root = temp_root("status");
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let endpoint = DaemonEndpoint {
            schema: DAEMON_ENDPOINT_SCHEMA.to_string(),
            project_id: "alpha".to_string(),
            root: root.clone(),
            registry_path: root.join("projects.json"),
            address,
            pid: 1,
            started_at_unix_ms: 1,
            max_concurrent_requests: 4,
            max_job_history: 100,
            watch: false,
            watch_poll: false,
            debounce_ms: 1000,
            max_request_bytes: DEFAULT_MAX_REQUEST_BYTES,
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
        };
        let state = Arc::new(DaemonState {
            endpoint,
            jobs: Mutex::new(Vec::new()),
            next_job_sequence: Mutex::new(1),
            max_job_history: 100,
            latest_snapshot_cache: Mutex::new(None),
        });
        let task = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_connection(stream, state).await.unwrap()
        });

        let endpoint_path = root.join(".athanor/daemon/endpoint.json");
        fs::create_dir_all(endpoint_path.parent().unwrap()).unwrap();
        write_endpoint(
            &endpoint_path,
            &DaemonEndpoint {
                schema: DAEMON_ENDPOINT_SCHEMA.to_string(),
                project_id: "alpha".to_string(),
                root: root.clone(),
                registry_path: root.join("projects.json"),
                address,
                pid: 1,
                started_at_unix_ms: 1,
                max_concurrent_requests: 4,
                max_job_history: 100,
                watch: false,
                watch_poll: false,
                debounce_ms: 1000,
                max_request_bytes: DEFAULT_MAX_REQUEST_BYTES,
                max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
            },
        )
        .unwrap();
        let response = request_daemon(
            DaemonClientOptions { root: root.clone() },
            DaemonRequest {
                schema: DAEMON_REQUEST_SCHEMA.to_string(),
                request_id: "req-1".to_string(),
                project_id: "alpha".to_string(),
                command: DaemonCommand::Status,
            },
        )
        .await
        .unwrap();
        assert!(response.ok);
        assert_eq!(response.project_id, "alpha");
        assert!(!task.await.unwrap());

        let error = request_daemon(
            DaemonClientOptions { root: root.clone() },
            DaemonRequest {
                schema: DAEMON_REQUEST_SCHEMA.to_string(),
                request_id: "req-2".to_string(),
                project_id: "beta".to_string(),
                command: DaemonCommand::Status,
            },
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("belongs to project `alpha`"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn lists_daemon_jobs_newest_first_with_limit() {
        let endpoint = DaemonEndpoint {
            schema: DAEMON_ENDPOINT_SCHEMA.to_string(),
            project_id: "alpha".to_string(),
            root: PathBuf::from("."),
            registry_path: PathBuf::from("projects.json"),
            address: "127.0.0.1:1".parse().unwrap(),
            pid: 1,
            started_at_unix_ms: 1,
            max_concurrent_requests: 4,
            max_job_history: 100,
            watch: false,
            watch_poll: false,
            debounce_ms: 1000,
            max_request_bytes: DEFAULT_MAX_REQUEST_BYTES,
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
        };
        let state = DaemonState {
            endpoint,
            jobs: Mutex::new(vec![
                DaemonJob {
                    id: "job_00000001".to_string(),
                    kind: DaemonJobKind::DaemonLifecycle,
                    status: DaemonJobStatus::Succeeded,
                    description: "first".to_string(),
                    created_at_unix_ms: 1,
                    started_at_unix_ms: Some(1),
                    finished_at_unix_ms: Some(1),
                    result: None,
                    error: None,
                },
                DaemonJob {
                    id: "job_00000002".to_string(),
                    kind: DaemonJobKind::DaemonLifecycle,
                    status: DaemonJobStatus::Running,
                    description: "second".to_string(),
                    created_at_unix_ms: 2,
                    started_at_unix_ms: Some(2),
                    finished_at_unix_ms: None,
                    result: None,
                    error: None,
                },
            ]),
            next_job_sequence: Mutex::new(3),
            max_job_history: 100,
            latest_snapshot_cache: Mutex::new(None),
        };

        let report = list_daemon_jobs(&state, 1).unwrap();
        assert_eq!(report.schema, DAEMON_JOBS_SCHEMA);
        assert_eq!(report.total, 2);
        assert_eq!(report.returned, 1);
        assert_eq!(report.jobs[0].id, "job_00000002");
    }

    #[test]
    fn gets_daemon_job_by_id_and_rejects_invalid_ids() {
        let endpoint = DaemonEndpoint {
            schema: DAEMON_ENDPOINT_SCHEMA.to_string(),
            project_id: "alpha".to_string(),
            root: PathBuf::from("."),
            registry_path: PathBuf::from("projects.json"),
            address: "127.0.0.1:1".parse().unwrap(),
            pid: 1,
            started_at_unix_ms: 1,
            max_concurrent_requests: 4,
            max_job_history: 100,
            watch: false,
            watch_poll: false,
            debounce_ms: 1000,
            max_request_bytes: DEFAULT_MAX_REQUEST_BYTES,
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
        };
        let state = DaemonState {
            endpoint,
            jobs: Mutex::new(vec![DaemonJob {
                id: "job_00000001".to_string(),
                kind: DaemonJobKind::DaemonLifecycle,
                status: DaemonJobStatus::Succeeded,
                description: "first".to_string(),
                created_at_unix_ms: 1,
                started_at_unix_ms: Some(1),
                finished_at_unix_ms: Some(1),
                result: None,
                error: None,
            }]),
            next_job_sequence: Mutex::new(2),
            max_job_history: 100,
            latest_snapshot_cache: Mutex::new(None),
        };

        let job = get_daemon_job(&state, "job_00000001").unwrap();
        assert_eq!(job.description, "first");
        assert!(get_daemon_job(&state, "bad").is_err());
        assert!(get_daemon_job(&state, "job_99999999").is_err());
    }

    #[test]
    fn context_request_defaults_diff_to_false_for_existing_clients() {
        let request: DaemonRequest = serde_json::from_value(serde_json::json!({
            "schema": DAEMON_REQUEST_SCHEMA,
            "request_id": "req-context",
            "project_id": "alpha",
            "command": {
                "name": "context",
                "task": "auth",
                "level": "normal",
                "limits": {}
            }
        }))
        .unwrap();

        match request.command {
            DaemonCommand::Context { task, diff, .. } => {
                assert_eq!(task, "auth");
                assert!(!diff);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[tokio::test]
    async fn explains_entity_from_hot_snapshot_cache() {
        let snapshot = CanonicalSnapshot {
            snapshot: Some(athanor_domain::SnapshotId("snap_cached".to_string())),
            entities: vec![athanor_domain::Entity {
                id: athanor_domain::EntityId("ent_login".to_string()),
                stable_key: athanor_domain::StableKey("api://POST:/login".to_string()),
                kind: athanor_domain::EntityKind::ApiEndpoint,
                name: "POST /login".to_string(),
                title: None,
                source: None,
                language: None,
                aliases: Vec::new(),
                ownership: Vec::new(),
                payload: serde_json::json!({}),
            }],
            facts: Vec::new(),
            relations: Vec::new(),
            diagnostics: Vec::new(),
        };
        let state = Arc::new(DaemonState {
            endpoint: DaemonEndpoint {
                schema: DAEMON_ENDPOINT_SCHEMA.to_string(),
                project_id: "alpha".to_string(),
                root: PathBuf::from("."),
                registry_path: PathBuf::from("projects.json"),
                address: "127.0.0.1:1".parse().unwrap(),
                pid: 1,
                started_at_unix_ms: 1,
                max_concurrent_requests: 4,
                max_job_history: 100,
                watch: false,
                watch_poll: false,
                debounce_ms: 1000,
                max_request_bytes: DEFAULT_MAX_REQUEST_BYTES,
                max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
            },
            jobs: Mutex::new(Vec::new()),
            next_job_sequence: Mutex::new(1),
            max_job_history: 100,
            latest_snapshot_cache: Mutex::new(Some(snapshot)),
        });

        let explanation = daemon_explain_from_cache(&state, "api://POST:/login")
            .await
            .unwrap();

        assert_eq!(explanation.schema, "athanor.entity_explanation.v1");
        assert_eq!(explanation.snapshot, "snap_cached");
        assert_eq!(explanation.entity.name, "POST /login");
    }

    #[test]
    fn detects_active_job_by_kind() {
        let endpoint = DaemonEndpoint {
            schema: DAEMON_ENDPOINT_SCHEMA.to_string(),
            project_id: "alpha".to_string(),
            root: PathBuf::from("."),
            registry_path: PathBuf::from("projects.json"),
            address: "127.0.0.1:1".parse().unwrap(),
            pid: 1,
            started_at_unix_ms: 1,
            max_concurrent_requests: 4,
            max_job_history: 100,
            watch: false,
            watch_poll: false,
            debounce_ms: 1000,
            max_request_bytes: DEFAULT_MAX_REQUEST_BYTES,
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
        };
        let state = DaemonState {
            endpoint,
            jobs: Mutex::new(vec![DaemonJob {
                id: "job_00000001".to_string(),
                kind: DaemonJobKind::Index,
                status: DaemonJobStatus::Running,
                description: "index project".to_string(),
                created_at_unix_ms: 1,
                started_at_unix_ms: Some(1),
                finished_at_unix_ms: None,
                result: None,
                error: None,
            }]),
            next_job_sequence: Mutex::new(2),
            max_job_history: 100,
            latest_snapshot_cache: Mutex::new(None),
        };

        assert!(has_active_job(&state, DaemonJobKind::Index).unwrap());
        assert!(!has_active_job(&state, DaemonJobKind::Context).unwrap());
        assert!(!has_active_job(&state, DaemonJobKind::Generate).unwrap());
    }

    #[test]
    fn watcher_ignores_athanor_artifact_events() {
        let root = PathBuf::from("D:/project");

        assert!(is_project_source_event(&root, &root.join("src/lib.rs")));
        assert!(is_project_source_event(&root, &root.join("docs/README.md")));
        assert!(!is_project_source_event(
            &root,
            &root.join(".athanor/generated/current/manifest.json")
        ));
        assert!(!is_project_source_event(
            &root,
            &root.join(".athanor/store/canonical/jsonl/latest.json")
        ));
    }

    #[test]
    fn cancels_queued_jobs_and_rejects_running_jobs() {
        let endpoint = DaemonEndpoint {
            schema: DAEMON_ENDPOINT_SCHEMA.to_string(),
            project_id: "alpha".to_string(),
            root: PathBuf::from("."),
            registry_path: PathBuf::from("projects.json"),
            address: "127.0.0.1:1".parse().unwrap(),
            pid: 1,
            started_at_unix_ms: 1,
            max_concurrent_requests: 4,
            max_job_history: 100,
            watch: false,
            watch_poll: false,
            debounce_ms: 1000,
            max_request_bytes: DEFAULT_MAX_REQUEST_BYTES,
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
        };
        let state = DaemonState {
            endpoint,
            jobs: Mutex::new(vec![
                DaemonJob {
                    id: "job_00000001".to_string(),
                    kind: DaemonJobKind::Index,
                    status: DaemonJobStatus::Queued,
                    description: "queued index".to_string(),
                    created_at_unix_ms: 1,
                    started_at_unix_ms: None,
                    finished_at_unix_ms: None,
                    result: None,
                    error: None,
                },
                DaemonJob {
                    id: "job_00000002".to_string(),
                    kind: DaemonJobKind::Index,
                    status: DaemonJobStatus::Running,
                    description: "running index".to_string(),
                    created_at_unix_ms: 2,
                    started_at_unix_ms: Some(2),
                    finished_at_unix_ms: None,
                    result: None,
                    error: None,
                },
            ]),
            next_job_sequence: Mutex::new(3),
            max_job_history: 100,
            latest_snapshot_cache: Mutex::new(None),
        };

        let cancelled = cancel_daemon_job(&state, "job_00000001").unwrap();
        assert_eq!(cancelled.status, DaemonJobStatus::Cancelled);
        assert!(cancelled.finished_at_unix_ms.is_some());
        assert!(cancel_daemon_job(&state, "job_00000002").is_err());
    }

    #[test]
    fn tracks_daemon_job_start_and_finish() {
        let endpoint = DaemonEndpoint {
            schema: DAEMON_ENDPOINT_SCHEMA.to_string(),
            project_id: "alpha".to_string(),
            root: PathBuf::from("."),
            registry_path: PathBuf::from("projects.json"),
            address: "127.0.0.1:1".parse().unwrap(),
            pid: 1,
            started_at_unix_ms: 1,
            max_concurrent_requests: 4,
            max_job_history: 100,
            watch: false,
            watch_poll: false,
            debounce_ms: 1000,
            max_request_bytes: DEFAULT_MAX_REQUEST_BYTES,
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
        };
        let state = DaemonState {
            endpoint,
            jobs: Mutex::new(Vec::new()),
            next_job_sequence: Mutex::new(1),
            max_job_history: 100,
            latest_snapshot_cache: Mutex::new(None),
        };

        let job_id = start_daemon_job(
            &state,
            DaemonJobKind::Overview,
            "overview top=1".to_string(),
        )
        .unwrap();
        assert_eq!(job_id, "job_00000001");
        let queued = get_daemon_job(&state, &job_id).unwrap();
        assert_eq!(queued.status, DaemonJobStatus::Queued);
        assert_eq!(queued.started_at_unix_ms, None);
        assert!(mark_daemon_job_running(&state, &job_id).unwrap());
        finish_daemon_job(
            &state,
            &job_id,
            DaemonJobStatus::Succeeded,
            Some(serde_json::json!({"ok": true})),
            None,
        )
        .unwrap();

        let report = list_daemon_jobs(&state, 10).unwrap();
        assert_eq!(report.total, 1);
        assert_eq!(report.jobs[0].kind, DaemonJobKind::Overview);
        assert_eq!(report.jobs[0].status, DaemonJobStatus::Succeeded);
        assert!(report.jobs[0].started_at_unix_ms.is_some());
        assert_eq!(report.jobs[0].result, Some(serde_json::json!({"ok": true})));
        assert!(report.jobs[0].finished_at_unix_ms.is_some());
    }

    #[test]
    fn cancelled_queued_job_does_not_start() {
        let endpoint = DaemonEndpoint {
            schema: DAEMON_ENDPOINT_SCHEMA.to_string(),
            project_id: "alpha".to_string(),
            root: PathBuf::from("."),
            registry_path: PathBuf::from("projects.json"),
            address: "127.0.0.1:1".parse().unwrap(),
            pid: 1,
            started_at_unix_ms: 1,
            max_concurrent_requests: 4,
            max_job_history: 100,
            watch: false,
            watch_poll: false,
            debounce_ms: 1000,
            max_request_bytes: DEFAULT_MAX_REQUEST_BYTES,
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
        };
        let state = DaemonState {
            endpoint,
            jobs: Mutex::new(Vec::new()),
            next_job_sequence: Mutex::new(1),
            max_job_history: 100,
            latest_snapshot_cache: Mutex::new(None),
        };

        let job_id =
            start_daemon_job(&state, DaemonJobKind::Index, "queued index".to_string()).unwrap();
        let cancelled = cancel_daemon_job(&state, &job_id).unwrap();
        assert_eq!(cancelled.status, DaemonJobStatus::Cancelled);
        assert!(!mark_daemon_job_running(&state, &job_id).unwrap());
    }

    #[test]
    fn prunes_old_finished_jobs_but_keeps_running_jobs() {
        let mut jobs = vec![
            DaemonJob {
                id: "job_00000001".to_string(),
                kind: DaemonJobKind::DaemonLifecycle,
                status: DaemonJobStatus::Succeeded,
                description: "old finished".to_string(),
                created_at_unix_ms: 1,
                started_at_unix_ms: Some(1),
                finished_at_unix_ms: Some(1),
                result: None,
                error: None,
            },
            DaemonJob {
                id: "job_00000002".to_string(),
                kind: DaemonJobKind::Context,
                status: DaemonJobStatus::Running,
                description: "running".to_string(),
                created_at_unix_ms: 2,
                started_at_unix_ms: Some(2),
                finished_at_unix_ms: None,
                result: None,
                error: None,
            },
            DaemonJob {
                id: "job_00000003".to_string(),
                kind: DaemonJobKind::Overview,
                status: DaemonJobStatus::Succeeded,
                description: "new finished".to_string(),
                created_at_unix_ms: 3,
                started_at_unix_ms: Some(3),
                finished_at_unix_ms: Some(3),
                result: None,
                error: None,
            },
        ];

        prune_daemon_jobs(&mut jobs, 2);

        assert_eq!(jobs.len(), 2);
        assert!(!jobs.iter().any(|job| job.id == "job_00000001"));
        assert!(jobs.iter().any(|job| job.id == "job_00000002"));
        assert!(jobs.iter().any(|job| job.id == "job_00000003"));
    }

    #[tokio::test]
    async fn busy_response_is_structured_and_preserves_request_id() {
        let root = temp_root("busy");
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let endpoint = DaemonEndpoint {
            schema: DAEMON_ENDPOINT_SCHEMA.to_string(),
            project_id: "alpha".to_string(),
            root: root.clone(),
            registry_path: root.join("projects.json"),
            address,
            pid: 1,
            started_at_unix_ms: 1,
            max_concurrent_requests: 1,
            max_job_history: 100,
            watch: false,
            watch_poll: false,
            debounce_ms: 1000,
            max_request_bytes: DEFAULT_MAX_REQUEST_BYTES,
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
        };
        let task = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_busy_connection(stream, &endpoint).await.unwrap();
        });

        let mut stream = TcpStream::connect(address).await.unwrap();
        let request = DaemonRequest {
            schema: DAEMON_REQUEST_SCHEMA.to_string(),
            request_id: "req-busy".to_string(),
            project_id: "alpha".to_string(),
            command: DaemonCommand::Status,
        };
        stream
            .write_all(&serde_json::to_vec(&request).unwrap())
            .await
            .unwrap();
        stream.write_all(b"\n").await.unwrap();
        stream.shutdown().await.unwrap();

        let mut response = Vec::new();
        stream.read_to_end(&mut response).await.unwrap();
        let response: DaemonResponse = serde_json::from_slice(&response).unwrap();
        assert!(!response.ok);
        assert_eq!(response.request_id, "req-busy");
        assert_eq!(response.project_id, "alpha");
        assert!(
            response
                .error
                .as_deref()
                .unwrap_or_default()
                .contains("daemon is busy")
        );
        task.await.unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn daemon_lock_is_single_instance_and_cleans_up() {
        let root = temp_root("lock");
        let lock_path = root.join("lock");
        let lock = DaemonLock::acquire(&lock_path, "alpha").unwrap();
        assert!(DaemonLock::acquire(&lock_path, "alpha").is_err());
        drop(lock);
        assert!(!lock_path.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn oversized_response_is_replaced_with_structured_error() {
        let response = success_response(
            "req-large",
            "alpha",
            serde_json::json!({
                "body": "x".repeat((DEFAULT_MAX_RESPONSE_BYTES as usize) + 1),
            }),
        );

        let serialized = serialize_daemon_response(response, DEFAULT_MAX_RESPONSE_BYTES).unwrap();
        assert!(serialized.len() as u64 <= DEFAULT_MAX_RESPONSE_BYTES);
        let parsed: DaemonResponse = serde_json::from_slice(&serialized).unwrap();
        assert!(!parsed.ok);
        assert_eq!(parsed.request_id, "req-large");
        assert_eq!(parsed.project_id, "alpha");
        assert!(
            parsed
                .error
                .as_deref()
                .unwrap_or_default()
                .contains("response exceeds size limit")
        );
    }

    fn temp_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "athanor-daemon-{label}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }
}
