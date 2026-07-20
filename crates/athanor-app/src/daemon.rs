use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
#[cfg(unix)]
use std::{env, os::unix::fs::PermissionsExt};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(windows)]
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions as PipeServerOptions};
use tokio::net::{TcpListener, TcpStream};
#[cfg(unix)]
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{Semaphore, mpsc};

use athanor_core::{CanonicalSnapshot, SearchIndex};
use athanor_domain::ContextLevel;

use crate::daemon_client::request as request_daemon_transport;
use crate::daemon_connection::{
    handle as handle_connection, handle_busy as handle_busy_connection,
};
use crate::daemon_endpoint::{read as read_endpoint, write as write_endpoint};
use crate::daemon_jobs_support::unix_time_ms;
use crate::daemon_lifecycle::{cancel_active_jobs, drain_active_jobs, set as set_lifecycle};
use crate::daemon_protocol::{validate_limit, validate_request_shape};
use crate::daemon_recovery::cleanup_known_staging_artifacts;
use crate::daemon_runtime::BoundedCache;
use crate::daemon_watcher::{start_file_watcher, start_watcher_index_job};
use crate::{
    CancellationToken, ContextLimitOverrides, ContextLimits, DaemonRuntimeLock, DaemonRuntimePaths,
    RepositoryOverview, RuntimeComposition, RuntimeFileGuard, create_daemon_token,
    read_daemon_token,
};

pub const DAEMON_ENDPOINT_SCHEMA: &str = "athanor.daemon_endpoint.v3";
pub const DAEMON_REQUEST_SCHEMA: &str = "athanor.daemon_request.v3";
pub const DAEMON_RESPONSE_SCHEMA: &str = "athanor.daemon_response.v3";
pub const DAEMON_ENDPOINT_SCHEMA_V2: &str = "athanor.daemon_endpoint.v2";
pub const DAEMON_REQUEST_SCHEMA_V2: &str = "athanor.daemon_request.v2";
pub const DAEMON_RESPONSE_SCHEMA_V2: &str = "athanor.daemon_response.v2";
pub const DAEMON_REQUEST_SCHEMA_V3: &str = "athanor.daemon_request.v3";
pub const DAEMON_RESPONSE_SCHEMA_V3: &str = "athanor.daemon_response.v3";
pub const DAEMON_ENDPOINT_SCHEMA_V1: &str = "athanor.daemon_endpoint.v1";
pub const DAEMON_REQUEST_SCHEMA_V1: &str = "athanor.daemon_request.v1";
pub const DAEMON_PROTOCOL_VERSION: u32 = 3;
pub const DAEMON_PROTOCOL_VERSION_V2: u32 = 2;
pub const DAEMON_JOBS_SCHEMA: &str = "athanor.daemon_jobs.v1";
const DEFAULT_MAX_REQUEST_BYTES: u64 = 1024 * 1024;
const DEFAULT_MAX_RESPONSE_BYTES: u64 = 1024 * 1024;
pub(crate) const MIN_PROTOCOL_BYTES: u64 = 1024;
pub(crate) const MAX_PROTOCOL_BYTES: u64 = 64 * 1024 * 1024;
const DERIVED_CACHE_CAPACITY: usize = 64;

#[derive(Debug, Clone)]
pub struct DaemonServeOptions {
    pub project_id: String,
    pub root: PathBuf,
    pub registry_path: PathBuf,
    pub listen: SocketAddr,
    pub transport: DaemonTransport,
    pub max_concurrent_requests: usize,
    pub max_job_history: usize,
    pub watch: bool,
    pub watch_poll: bool,
    pub debounce_ms: u64,
    pub max_request_bytes: u64,
    pub max_response_bytes: u64,
    pub insecure_allow_v1: bool,
    pub runtime_dir: Option<PathBuf>,
    pub shutdown_timeout: Duration,
}

#[derive(Debug, Clone)]
pub struct DaemonClientOptions {
    pub root: PathBuf,
    pub runtime_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DaemonEndpoint {
    pub schema: String,
    pub protocol_version: u32,
    pub athanor_version: String,
    pub runtime_id: String,
    pub token_path: PathBuf,
    pub project_id: String,
    pub root: PathBuf,
    pub registry_path: PathBuf,
    pub address: SocketAddr,
    #[serde(default)]
    pub transport: DaemonTransport,
    #[serde(default)]
    pub local_socket_path: Option<PathBuf>,
    #[serde(default)]
    pub windows_pipe_name: Option<String>,
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

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DaemonTransport {
    #[default]
    Tcp,
    LocalSocket,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DaemonRequest {
    pub schema: String,
    pub request_id: String,
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_token: Option<String>,
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
    Index {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        deadline_unix_ms: Option<u64>,
    },
    Generate {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        deadline_unix_ms: Option<u64>,
    },
    Wiki {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        deadline_unix_ms: Option<u64>,
    },
    HtmlReport {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        deadline_unix_ms: Option<u64>,
    },
    Overview {
        top: usize,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        deadline_unix_ms: Option<u64>,
    },
    Explain {
        stable_key: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        deadline_unix_ms: Option<u64>,
    },
    Search {
        query: String,
        limit: usize,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        deadline_unix_ms: Option<u64>,
    },
    Context {
        task: String,
        #[serde(default)]
        diff: bool,
        level: ContextLevel,
        limits: ContextLimitOverrides,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        deadline_unix_ms: Option<u64>,
    },
    ChangeMap {
        #[serde(default)]
        task: Option<String>,
        #[serde(default)]
        target: Option<String>,
        #[serde(default)]
        diff: bool,
        max_entities: usize,
        max_files: usize,
        max_diagnostics: usize,
        max_depth: usize,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        deadline_unix_ms: Option<u64>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_details: Option<DaemonError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DaemonError {
    pub code: DaemonErrorCode,
    pub message: String,
    pub retryable: bool,
    #[serde(default)]
    pub details: serde_json::Map<String, Value>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DaemonErrorCode {
    InvalidInput,
    NotFound,
    Conflict,
    Busy,
    Unauthorized,
    Forbidden,
    Cancelled,
    DeadlineExceeded,
    AdapterProtocol,
    AdapterExecution,
    StorageUnavailable,
    StorageCorruption,
    SnapshotNotCommitted,
    Unsupported,
    Internal,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DaemonLifecycleState {
    Running,
    Stopping,
    Stopped,
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
    Search,
    Context,
    ChangeMap,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DaemonJobStatus {
    Queued,
    Running,
    Cancelling,
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

pub(crate) struct DaemonState {
    pub(crate) composition: RuntimeComposition,
    pub(crate) endpoint: DaemonEndpoint,
    pub(crate) auth_token: String,
    pub(crate) insecure_allow_v1: bool,
    pub(crate) lifecycle: Mutex<DaemonLifecycleState>,
    pub(crate) last_successful_index: Mutex<Option<String>>,
    pub(crate) jobs: Mutex<Vec<DaemonJob>>,
    pub(crate) next_job_sequence: Mutex<u64>,
    pub(crate) max_job_history: usize,
    pub(crate) latest_snapshot_cache: Mutex<Option<CanonicalSnapshot>>,
    pub(crate) search_index_cache: Mutex<Option<CachedSearchIndex>>,
    pub(crate) overview_cache: Mutex<BoundedCache<OverviewCacheKey, RepositoryOverview>>,
    pub(crate) context_cache: Mutex<BoundedCache<ContextCacheKey, athanor_domain::ContextPack>>,
    pub(crate) cancellation_tokens: Mutex<HashMap<String, CancellationToken>>,
}

pub(crate) struct CachedSearchIndex {
    pub(crate) snapshot_id: String,
    pub(crate) index: Arc<dyn SearchIndex>,
}

impl fmt::Debug for CachedSearchIndex {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CachedSearchIndex")
            .field("snapshot_id", &self.snapshot_id)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OverviewCacheKey {
    pub(crate) snapshot_id: String,
    pub(crate) top: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContextCacheKey {
    pub(crate) snapshot_id: String,
    pub(crate) task: String,
    pub(crate) level: String,
    pub(crate) limits: ContextLimits,
}

enum DaemonConnection {
    Tcp(TcpStream),
    #[cfg(unix)]
    Unix(UnixStream),
    #[cfg(windows)]
    Pipe(NamedPipeServer),
}

struct AcceptedDaemonConnection {
    stream: DaemonConnection,
    peer: String,
}

/// Serves a daemon with explicitly supplied application dependencies.
pub async fn serve_daemon_with_composition(
    options: DaemonServeOptions,
    composition: RuntimeComposition,
) -> Result<()> {
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
    if options.transport == DaemonTransport::Tcp && !options.listen.ip().is_loopback() {
        bail!("daemon TCP transport may only bind to a loopback address");
    }
    if options.shutdown_timeout < Duration::from_secs(1)
        || options.shutdown_timeout > Duration::from_secs(300)
    {
        bail!("daemon shutdown timeout must be between 1 and 300 seconds");
    }
    validate_limit("max_request_bytes", options.max_request_bytes)?;
    validate_limit("max_response_bytes", options.max_response_bytes)?;
    let root = options.root.canonicalize().with_context(|| {
        format!(
            "failed to canonicalize daemon root {}",
            options.root.display()
        )
    })?;
    cleanup_known_staging_artifacts(&root)?;
    let runtime =
        DaemonRuntimePaths::for_project(&options.project_id, options.runtime_dir.as_deref())?;
    runtime.prepare()?;
    let _lock = DaemonRuntimeLock::acquire(&runtime.lock, &options.project_id)?;
    let auth_token = create_daemon_token(&runtime.token)?;
    let _runtime_guard = RuntimeFileGuard::new([runtime.endpoint.clone(), runtime.token.clone()]);
    let runtime_id = format!("runtime-{}-{}", std::process::id(), unix_time_ms()?);

    let (accepted_tx, mut accepted_rx) = mpsc::channel::<AcceptedDaemonConnection>(64);
    let mut _local_socket_guard = None;
    let (address, local_socket_path, windows_pipe_name) = match options.transport {
        DaemonTransport::Tcp => {
            let listener = TcpListener::bind(options.listen)
                .await
                .with_context(|| format!("failed to bind daemon listener {}", options.listen))?;
            let address = listener.local_addr()?;
            spawn_tcp_acceptor(listener, accepted_tx);
            (address, None, None)
        }
        DaemonTransport::LocalSocket => {
            let local = local_socket_endpoint(&runtime.directory, &runtime_id)?;
            spawn_local_socket_acceptor(&local, accepted_tx).await?;
            _local_socket_guard = local.guard;
            (options.listen, local.socket_path, local.pipe_name)
        }
    };
    let endpoint = DaemonEndpoint {
        schema: DAEMON_ENDPOINT_SCHEMA.to_string(),
        protocol_version: DAEMON_PROTOCOL_VERSION,
        athanor_version: env!("CARGO_PKG_VERSION").to_string(),
        runtime_id,
        token_path: runtime.token.clone(),
        project_id: options.project_id.clone(),
        root: root.clone(),
        registry_path: options.registry_path,
        address,
        transport: options.transport,
        local_socket_path,
        windows_pipe_name,
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
    write_endpoint(&runtime.endpoint, &endpoint)?;
    let state = Arc::new(DaemonState {
        composition,
        auth_token,
        insecure_allow_v1: options.insecure_allow_v1,
        lifecycle: Mutex::new(DaemonLifecycleState::Running),
        last_successful_index: Mutex::new(None),
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
        search_index_cache: Mutex::new(None),
        overview_cache: Mutex::new(BoundedCache::new(DERIVED_CACHE_CAPACITY)),
        context_cache: Mutex::new(BoundedCache::new(DERIVED_CACHE_CAPACITY)),
        cancellation_tokens: Mutex::new(HashMap::new()),
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
    if options.insecure_allow_v1 {
        tracing::warn!(
            project_id = %state.endpoint.project_id,
            "insecure daemon v1 compatibility is enabled"
        );
    }
    tracing::info!(
        project_id = %state.endpoint.project_id,
        root = %root.display(),
        transport = ?state.endpoint.transport,
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
            accepted = accepted_rx.recv() => {
                let Some(accepted) = accepted else {
                    break;
                };
                if let Err(error) = spawn_daemon_connection(
                    accepted,
                    &state,
                    &request_slots,
                    &shutdown_tx,
                ) {
                    tracing::warn!(error = %error, "failed to schedule daemon connection");
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

    set_lifecycle(&state, DaemonLifecycleState::Stopping);
    cancel_active_jobs(&state);
    drain_active_jobs(&state, options.shutdown_timeout).await?;
    set_lifecycle(&state, DaemonLifecycleState::Stopped);
    tracing::info!(project_id = %state.endpoint.project_id, "Athanor daemon stopped");
    Ok(())
}

fn spawn_tcp_acceptor(listener: TcpListener, accepted_tx: mpsc::Sender<AcceptedDaemonConnection>) {
    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, peer)) => {
                    if accepted_tx
                        .send(AcceptedDaemonConnection {
                            stream: DaemonConnection::Tcp(stream),
                            peer: peer.to_string(),
                        })
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Err(error) => {
                    tracing::warn!(error = %error, "failed to accept daemon TCP connection");
                    break;
                }
            }
        }
    });
}

fn spawn_daemon_connection(
    accepted: AcceptedDaemonConnection,
    state: &Arc<DaemonState>,
    request_slots: &Arc<Semaphore>,
    shutdown_tx: &mpsc::Sender<()>,
) -> Result<()> {
    let peer = accepted.peer;
    match request_slots.clone().try_acquire_owned() {
        Ok(permit) => {
            let state = Arc::clone(state);
            let shutdown_tx = shutdown_tx.clone();
            tokio::spawn(async move {
                let _permit = permit;
                let result = match accepted.stream {
                    DaemonConnection::Tcp(stream) => handle_connection(stream, state).await,
                    #[cfg(unix)]
                    DaemonConnection::Unix(stream) => handle_connection(stream, state).await,
                    #[cfg(windows)]
                    DaemonConnection::Pipe(stream) => handle_connection(stream, state).await,
                };
                match result {
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
            let state = Arc::clone(state);
            tokio::spawn(async move {
                let result = match accepted.stream {
                    DaemonConnection::Tcp(stream) => handle_busy_connection(stream, &state).await,
                    #[cfg(unix)]
                    DaemonConnection::Unix(stream) => handle_busy_connection(stream, &state).await,
                    #[cfg(windows)]
                    DaemonConnection::Pipe(stream) => handle_busy_connection(stream, &state).await,
                };
                if let Err(error) = result {
                    tracing::warn!(%peer, error = %error, "failed to reject busy daemon request");
                }
            });
        }
    }
    Ok(())
}

pub async fn request_daemon(
    options: DaemonClientOptions,
    mut request: DaemonRequest,
) -> Result<DaemonResponse> {
    let runtime =
        DaemonRuntimePaths::for_project(&request.project_id, options.runtime_dir.as_deref())?;
    let endpoint = read_endpoint(&runtime.endpoint)?;
    if endpoint.project_id != request.project_id {
        bail!(
            "daemon endpoint belongs to project `{}`, not `{}`",
            endpoint.project_id,
            request.project_id
        );
    }
    if endpoint.token_path != runtime.token {
        bail!("daemon endpoint token path does not match the expected runtime path");
    }
    request.schema = if endpoint.protocol_version == DAEMON_PROTOCOL_VERSION_V2
        || endpoint.schema == DAEMON_ENDPOINT_SCHEMA_V2
    {
        DAEMON_REQUEST_SCHEMA_V2.to_string()
    } else {
        DAEMON_REQUEST_SCHEMA.to_string()
    };
    request.auth_token = Some(read_daemon_token(&endpoint.token_path)?);
    validate_request_shape(&request)?;
    request_daemon_transport(&endpoint, &request).await
}

pub(crate) fn start_index_job(state: &Arc<DaemonState>, description: String) -> Result<DaemonJob> {
    crate::daemon_write_jobs::start_index(
        state,
        description,
        athanor_core::OperationContext::new("daemon.index"),
    )
}

pub(crate) fn has_active_job(state: &DaemonState, kind: DaemonJobKind) -> Result<bool> {
    crate::daemon_job_state::has_active(state, kind)
}

fn default_max_request_bytes() -> u64 {
    DEFAULT_MAX_REQUEST_BYTES
}

fn default_max_response_bytes() -> u64 {
    DEFAULT_MAX_RESPONSE_BYTES
}

struct LocalSocketGuard {
    socket_path: PathBuf,
    directory_path: Option<PathBuf>,
}

impl Drop for LocalSocketGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.socket_path);
        if let Some(directory_path) = &self.directory_path {
            let _ = fs::remove_dir(directory_path);
        }
    }
}

struct LocalSocketEndpoint {
    socket_path: Option<PathBuf>,
    pipe_name: Option<String>,
    guard: Option<LocalSocketGuard>,
}

#[cfg(unix)]
fn local_socket_endpoint(runtime_dir: &Path, runtime_id: &str) -> Result<LocalSocketEndpoint> {
    const MAX_UNIX_SOCKET_PATH_BYTES: usize = 100;

    let runtime_socket_path = runtime_dir.join("daemon.sock");
    let (socket_path, directory_path) =
        if runtime_socket_path.as_os_str().as_encoded_bytes().len() <= MAX_UNIX_SOCKET_PATH_BYTES {
            (runtime_socket_path, None)
        } else {
            let directory_path = env::temp_dir().join(format!("athanor-{runtime_id}"));
            fs::create_dir_all(&directory_path).with_context(|| {
                format!(
                    "failed to create short daemon socket directory {}",
                    directory_path.display()
                )
            })?;
            fs::set_permissions(&directory_path, fs::Permissions::from_mode(0o700))
                .with_context(|| format!("failed to secure {}", directory_path.display()))?;
            (directory_path.join("daemon.sock"), Some(directory_path))
        };
    if socket_path.exists() {
        fs::remove_file(&socket_path)
            .with_context(|| format!("failed to remove stale socket {}", socket_path.display()))?;
    }
    Ok(LocalSocketEndpoint {
        socket_path: Some(socket_path.clone()),
        pipe_name: None,
        guard: Some(LocalSocketGuard {
            socket_path,
            directory_path,
        }),
    })
}

#[cfg(windows)]
fn local_socket_endpoint(_runtime_dir: &Path, runtime_id: &str) -> Result<LocalSocketEndpoint> {
    Ok(LocalSocketEndpoint {
        socket_path: None,
        pipe_name: Some(format!(
            r"\\.\pipe\athanor-{}",
            sanitize_local_socket_label(runtime_id)
        )),
        guard: None,
    })
}

#[cfg(not(any(unix, windows)))]
fn local_socket_endpoint(_runtime_dir: &Path, _runtime_id: &str) -> Result<LocalSocketEndpoint> {
    bail!("local socket transport is not supported on this platform")
}

#[cfg(unix)]
async fn spawn_local_socket_acceptor(
    local: &LocalSocketEndpoint,
    accepted_tx: mpsc::Sender<AcceptedDaemonConnection>,
) -> Result<()> {
    let socket_path = local
        .socket_path
        .as_ref()
        .context("local socket path is missing")?;
    let listener = UnixListener::bind(socket_path)
        .with_context(|| format!("failed to bind daemon socket {}", socket_path.display()))?;
    fs::set_permissions(socket_path, fs::Permissions::from_mode(0o600))
        .with_context(|| format!("failed to secure daemon socket {}", socket_path.display()))?;
    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    if accepted_tx
                        .send(AcceptedDaemonConnection {
                            stream: DaemonConnection::Unix(stream),
                            peer: "local-socket".to_string(),
                        })
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Err(error) => {
                    tracing::warn!(error = %error, "failed to accept daemon local socket connection");
                    break;
                }
            }
        }
    });
    Ok(())
}

#[cfg(windows)]
async fn spawn_local_socket_acceptor(
    local: &LocalSocketEndpoint,
    accepted_tx: mpsc::Sender<AcceptedDaemonConnection>,
) -> Result<()> {
    let pipe_name = local
        .pipe_name
        .clone()
        .context("Windows pipe name is missing")?;
    tokio::spawn(async move {
        loop {
            let server = match PipeServerOptions::new().create(&pipe_name) {
                Ok(server) => server,
                Err(error) => {
                    tracing::warn!(pipe = %pipe_name, error = %error, "failed to create daemon pipe");
                    break;
                }
            };
            if let Err(error) = server.connect().await {
                tracing::warn!(pipe = %pipe_name, error = %error, "failed to accept daemon pipe connection");
                break;
            }
            if accepted_tx
                .send(AcceptedDaemonConnection {
                    stream: DaemonConnection::Pipe(server),
                    peer: pipe_name.clone(),
                })
                .await
                .is_err()
            {
                break;
            }
        }
    });
    Ok(())
}

#[cfg(not(any(unix, windows)))]
async fn spawn_local_socket_acceptor(
    _local: &LocalSocketEndpoint,
    _accepted_tx: mpsc::Sender<AcceptedDaemonConnection>,
) -> Result<()> {
    bail!("local socket transport is not supported on this platform")
}

#[cfg(windows)]
fn sanitize_local_socket_label(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "project".to_string()
    } else {
        sanitized
    }
}
