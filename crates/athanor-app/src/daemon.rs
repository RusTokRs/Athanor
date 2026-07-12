use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::fs::{self};
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
#[cfg(unix)]
use std::{env, os::unix::fs::PermissionsExt};

use anyhow::{Context, Result, bail};
use notify_debouncer_mini::notify::{
    Config as NotifyConfig, PollWatcher, RecommendedWatcher, RecursiveMode,
};
use notify_debouncer_mini::{
    Config as DebouncerConfig, DebounceEventResult, Debouncer, new_debouncer, new_debouncer_opt,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
#[cfg(windows)]
use tokio::net::windows::named_pipe::{
    ClientOptions as PipeClientOptions, NamedPipeServer, ServerOptions as PipeServerOptions,
};
use tokio::net::{TcpListener, TcpStream};
#[cfg(unix)]
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{Semaphore, mpsc};

use athanor_core::{CanonicalSnapshot, CanonicalSnapshotStore, SearchIndex};
use athanor_domain::ContextLevel;

use crate::explain::explain_snapshot;
use crate::search::{
    get_or_build_search_index_sync, get_or_build_search_index_with_factory,
    search_snapshot_with_index,
};
use crate::{
    CancellationToken, ChangeMapOptions, ContextLimitOverrides, ContextLimits, ContextOptions,
    DaemonRuntimeLock, DaemonRuntimePaths, GenerationOptions, HtmlReportOptions, IndexOptions,
    RepositoryOverview, RuntimeComposition, RuntimeFileGuard, WikiOptions,
    build_repository_overview, change_map_project, change_map_project_with_composition,
    constant_time_token_eq, context_project, context_project_with_composition, create_daemon_token,
    generate_context_pack, generate_project_cancellable,
    generate_project_cancellable_with_composition, index_project_cancellable,
    index_project_cancellable_with_composition, project_html_report_cancellable,
    project_html_report_cancellable_with_composition, project_wiki_cancellable,
    project_wiki_cancellable_with_composition, read_daemon_token,
};
use crate::{config::load_config, store::init_store};

pub const DAEMON_ENDPOINT_SCHEMA: &str = "athanor.daemon_endpoint.v2";
pub const DAEMON_REQUEST_SCHEMA: &str = "athanor.daemon_request.v2";
pub const DAEMON_RESPONSE_SCHEMA: &str = "athanor.daemon_response.v2";
pub const DAEMON_ENDPOINT_SCHEMA_V1: &str = "athanor.daemon_endpoint.v1";
pub const DAEMON_REQUEST_SCHEMA_V1: &str = "athanor.daemon_request.v1";
pub const DAEMON_PROTOCOL_VERSION: u32 = 2;
pub const DAEMON_JOBS_SCHEMA: &str = "athanor.daemon_jobs.v1";
const DEFAULT_MAX_REQUEST_BYTES: u64 = 1024 * 1024;
const DEFAULT_MAX_RESPONSE_BYTES: u64 = 1024 * 1024;
const MIN_PROTOCOL_BYTES: u64 = 1024;
const MAX_PROTOCOL_BYTES: u64 = 64 * 1024 * 1024;
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
    Search {
        query: String,
        limit: usize,
    },
    Context {
        task: String,
        #[serde(default)]
        diff: bool,
        level: ContextLevel,
        limits: ContextLimitOverrides,
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

struct DaemonState {
    composition: Option<RuntimeComposition>,
    endpoint: DaemonEndpoint,
    auth_token: String,
    insecure_allow_v1: bool,
    lifecycle: Mutex<DaemonLifecycleState>,
    last_successful_index: Mutex<Option<String>>,
    jobs: Mutex<Vec<DaemonJob>>,
    next_job_sequence: Mutex<u64>,
    max_job_history: usize,
    latest_snapshot_cache: Mutex<Option<CanonicalSnapshot>>,
    search_index_cache: Mutex<Option<CachedSearchIndex>>,
    overview_cache: Mutex<BoundedCache<OverviewCacheKey, RepositoryOverview>>,
    context_cache: Mutex<BoundedCache<ContextCacheKey, athanor_domain::ContextPack>>,
    cancellation_tokens: Mutex<HashMap<String, CancellationToken>>,
}

struct CachedSearchIndex {
    snapshot_id: String,
    index: Arc<dyn SearchIndex>,
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
struct OverviewCacheKey {
    snapshot_id: String,
    top: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ContextCacheKey {
    snapshot_id: String,
    task: String,
    level: String,
    limits: ContextLimits,
}

#[derive(Debug)]
struct BoundedCache<K, V> {
    capacity: usize,
    entries: VecDeque<(K, V)>,
}

impl<K: PartialEq, V: Clone> BoundedCache<K, V> {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            entries: VecDeque::new(),
        }
    }

    fn get(&mut self, key: &K) -> Option<V> {
        let index = self
            .entries
            .iter()
            .position(|(candidate, _)| candidate == key)?;
        let entry = self.entries.remove(index)?;
        let value = entry.1.clone();
        self.entries.push_back(entry);
        Some(value)
    }

    fn insert(&mut self, key: K, value: V) {
        if let Some(index) = self
            .entries
            .iter()
            .position(|(candidate, _)| candidate == &key)
        {
            self.entries.remove(index);
        }
        self.entries.push_back((key, value));
        while self.entries.len() > self.capacity {
            self.entries.pop_front();
        }
    }

    fn clear(&mut self) {
        self.entries.clear();
    }
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

pub async fn serve_daemon(options: DaemonServeOptions) -> Result<()> {
    serve_daemon_inner(options, None).await
}

/// Serves a daemon with explicitly supplied application dependencies.
///
/// This is the preferred entry point for new daemon hosts. The compatibility entry point keeps
/// using installed factories until every legacy daemon job has migrated.
pub async fn serve_daemon_with_composition(
    options: DaemonServeOptions,
    composition: RuntimeComposition,
) -> Result<()> {
    serve_daemon_inner(options, Some(composition)).await
}

async fn serve_daemon_inner(
    options: DaemonServeOptions,
    composition: Option<RuntimeComposition>,
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
    validate_protocol_limit("max_request_bytes", options.max_request_bytes)?;
    validate_protocol_limit("max_response_bytes", options.max_response_bytes)?;
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
    request.schema = DAEMON_REQUEST_SCHEMA.to_string();
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
    request.auth_token = Some(read_daemon_token(&endpoint.token_path)?);
    validate_request_shape(&request)?;
    match endpoint.transport {
        DaemonTransport::Tcp => {
            let stream = TcpStream::connect(endpoint.address)
                .await
                .with_context(|| format!("failed to connect to daemon at {}", endpoint.address))?;
            request_daemon_over_stream(stream, &endpoint, &request).await
        }
        DaemonTransport::LocalSocket => request_daemon_over_local_socket(&endpoint, &request).await,
    }
}

async fn request_daemon_over_stream<S>(
    mut stream: S,
    endpoint: &DaemonEndpoint,
    request: &DaemonRequest,
) -> Result<DaemonResponse>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
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

#[cfg(unix)]
async fn request_daemon_over_local_socket(
    endpoint: &DaemonEndpoint,
    request: &DaemonRequest,
) -> Result<DaemonResponse> {
    let socket_path = endpoint
        .local_socket_path
        .as_ref()
        .context("daemon endpoint does not include a local socket path")?;
    let stream = UnixStream::connect(socket_path).await.with_context(|| {
        format!(
            "failed to connect to daemon socket {}",
            socket_path.display()
        )
    })?;
    request_daemon_over_stream(stream, endpoint, request).await
}

#[cfg(windows)]
async fn request_daemon_over_local_socket(
    endpoint: &DaemonEndpoint,
    request: &DaemonRequest,
) -> Result<DaemonResponse> {
    let pipe_name = endpoint
        .windows_pipe_name
        .as_ref()
        .context("daemon endpoint does not include a Windows pipe name")?;
    let stream = PipeClientOptions::new()
        .open(pipe_name)
        .with_context(|| format!("failed to connect to daemon pipe {pipe_name}"))?;
    request_daemon_over_stream(stream, endpoint, request).await
}

#[cfg(not(any(unix, windows)))]
async fn request_daemon_over_local_socket(
    _endpoint: &DaemonEndpoint,
    _request: &DaemonRequest,
) -> Result<DaemonResponse> {
    bail!("local socket transport is not supported on this platform")
}

async fn handle_connection<S>(mut stream: S, state: Arc<DaemonState>) -> Result<bool>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut line = String::new();
    let bytes = BufReader::new(&mut stream)
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
    if let Err(error) = stream.write_all(&response_json).await {
        if is_client_disconnect(&error) {
            return Ok(false);
        }
        return Err(error).context("failed to write daemon response");
    }
    if let Err(error) = stream.shutdown().await {
        if is_client_disconnect(&error) {
            return Ok(false);
        }
        return Err(error.into());
    }
    Ok(shutdown)
}

fn is_client_disconnect(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        ErrorKind::BrokenPipe
            | ErrorKind::ConnectionAborted
            | ErrorKind::ConnectionReset
            | ErrorKind::NotConnected
    )
}

async fn handle_busy_connection<S>(mut stream: S, state: &DaemonState) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut line = String::new();
    let _ = BufReader::new(&mut stream)
        .take(state.endpoint.max_request_bytes + 1)
        .read_line(&mut line)
        .await;
    let parsed = serde_json::from_str::<DaemonRequest>(&line);
    let request_id = parsed
        .as_ref()
        .map(|request| request.request_id.clone())
        .unwrap_or_default();
    let message = match parsed.as_ref() {
        Ok(request) if validate_request(state, request).is_ok() => {
            "daemon is busy; maximum concurrent request limit reached"
        }
        _ => "daemon authentication failed",
    };
    let response = error_response(&request_id, &state.endpoint.project_id, message);
    stream
        .write_all(&serialize_daemon_response(
            response,
            state.endpoint.max_response_bytes,
        )?)
        .await
        .context("failed to write daemon busy response")?;
    stream.shutdown().await?;
    Ok(())
}

async fn execute_request(
    state: Arc<DaemonState>,
    request: DaemonRequest,
) -> (DaemonResponse, bool) {
    if let Err(error) = validate_request(&state, &request) {
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

    if lifecycle(&state) != DaemonLifecycleState::Running
        && !matches!(
            request.command,
            DaemonCommand::Status | DaemonCommand::Jobs { .. } | DaemonCommand::Job { .. }
        )
    {
        return (
            error_response(
                &request.request_id,
                &state.endpoint.project_id,
                "daemon is stopping and does not accept new work",
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
                    "status": lifecycle(&state),
                    "protocol_version": DAEMON_PROTOCOL_VERSION,
                    "athanor_version": env!("CARGO_PKG_VERSION"),
                    "uptime_ms": unix_time_ms()
                        .unwrap_or_default()
                        .saturating_sub(state.endpoint.started_at_unix_ms),
                    "active_jobs": active_job_count(&state).unwrap_or_default(),
                    "cache": cache_status(&state),
                    "last_successful_index": state.last_successful_index
                        .lock()
                        .ok()
                        .and_then(|snapshot| snapshot.clone()),
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
            match start_cancellable_daemon_job(
                &state,
                DaemonJobKind::Generate,
                "generate read models".to_string(),
            ) {
                Ok((job_id, cancellation)) => {
                    let job = get_daemon_job(&state, &job_id).ok();
                    let job_state = Arc::clone(&state);
                    let job_id_for_task = job_id.clone();
                    let root = state.endpoint.root.clone();
                    let cancellation_for_task = cancellation.clone();
                    let composition = daemon_composition(&state);
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
                                    runtime.block_on(async move {
                                        let options = GenerationOptions { root, force: false };
                                        match composition {
                                            Some(composition) => {
                                                generate_project_cancellable_with_composition(
                                                    options,
                                                    cancellation_for_task,
                                                    &composition,
                                                )
                                                .await
                                            }
                                            None => {
                                                generate_project_cancellable(
                                                    options,
                                                    cancellation_for_task,
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
                                    let _ = finish_cancellable_daemon_job_error(
                                        &job_state,
                                        &job_id_for_task,
                                        error,
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
            match start_cancellable_daemon_job(
                &state,
                DaemonJobKind::Wiki,
                "project wiki".to_string(),
            ) {
                Ok((job_id, cancellation)) => {
                    let job = get_daemon_job(&state, &job_id).ok();
                    let job_state = Arc::clone(&state);
                    let job_id_for_task = job_id.clone();
                    let root = state.endpoint.root.clone();
                    let cancellation_for_task = cancellation.clone();
                    let composition = daemon_composition(&state);
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
                                    runtime.block_on(async move {
                                        let options = WikiOptions { root, output: None };
                                        match composition {
                                            Some(composition) => {
                                                project_wiki_cancellable_with_composition(
                                                    options,
                                                    cancellation_for_task,
                                                    &composition,
                                                )
                                                .await
                                            }
                                            None => {
                                                project_wiki_cancellable(
                                                    options,
                                                    cancellation_for_task,
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
                                    let _ = finish_cancellable_daemon_job_error(
                                        &job_state,
                                        &job_id_for_task,
                                        error,
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
            match start_cancellable_daemon_job(
                &state,
                DaemonJobKind::HtmlReport,
                "HTML report".to_string(),
            ) {
                Ok((job_id, cancellation)) => {
                    let job = get_daemon_job(&state, &job_id).ok();
                    let job_state = Arc::clone(&state);
                    let job_id_for_task = job_id.clone();
                    let root = state.endpoint.root.clone();
                    let cancellation_for_task = cancellation.clone();
                    let composition = daemon_composition(&state);
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
                                    runtime.block_on(async move {
                                        let options = HtmlReportOptions { root, output: None };
                                        match composition {
                                            Some(composition) => {
                                                project_html_report_cancellable_with_composition(
                                                    options,
                                                    cancellation_for_task,
                                                    &composition,
                                                )
                                                .await
                                            }
                                            None => {
                                                project_html_report_cancellable(
                                                    options,
                                                    cancellation_for_task,
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
                                    let _ = finish_cancellable_daemon_job_error(
                                        &job_state,
                                        &job_id_for_task,
                                        error,
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
            match daemon_overview_from_cache(&state, top).await {
                Ok(overview) => {
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
        DaemonCommand::Search { query, limit } => {
            if query.trim().is_empty() {
                return (
                    error_response(
                        &request.request_id,
                        &state.endpoint.project_id,
                        "search query must not be empty",
                    ),
                    false,
                );
            }
            if limit == 0 || limit > 100 {
                return (
                    error_response(
                        &request.request_id,
                        &state.endpoint.project_id,
                        "search limit must be between 1 and 100",
                    ),
                    false,
                );
            }
            let query = query.trim().to_string();
            let job_id = match start_daemon_job(
                &state,
                DaemonJobKind::Search,
                format!("search query={query} limit={limit}"),
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
            match daemon_search_from_cache(&state, query, limit).await {
                Ok(report) => {
                    let _ =
                        finish_daemon_job(&state, &job_id, DaemonJobStatus::Succeeded, None, None);
                    (
                        success_response(
                            &request.request_id,
                            &state.endpoint.project_id,
                            serde_json::to_value(report).unwrap_or(Value::Null),
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
                let options = ContextOptions {
                    root: state.endpoint.root.clone(),
                    task,
                    diff,
                    level,
                    limits,
                };
                match daemon_composition(&state) {
                    Some(composition) => {
                        context_project_with_composition(options, &composition).await
                    }
                    None => context_project(options).await,
                }
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
        DaemonCommand::ChangeMap {
            task,
            target,
            diff,
            max_entities,
            max_files,
            max_diagnostics,
            max_depth,
        } => {
            let job_id = match start_daemon_job(
                &state,
                DaemonJobKind::ChangeMap,
                format!("change-map task={:?} target={target:?} diff={diff}", task),
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
            let options = ChangeMapOptions {
                root: state.endpoint.root.clone(),
                task,
                target,
                diff,
                max_entities,
                max_files,
                max_diagnostics,
                max_depth,
            };
            let report = match daemon_composition(&state) {
                Some(composition) => {
                    change_map_project_with_composition(options, &composition).await
                }
                None => change_map_project(options).await,
            };
            match report {
                Ok(report) => {
                    let _ =
                        finish_daemon_job(&state, &job_id, DaemonJobStatus::Succeeded, None, None);
                    (
                        success_response(
                            &request.request_id,
                            &state.endpoint.project_id,
                            serde_json::to_value(report).unwrap_or(Value::Null),
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

fn validate_request_shape(request: &DaemonRequest) -> Result<()> {
    if request.schema != DAEMON_REQUEST_SCHEMA && request.schema != DAEMON_REQUEST_SCHEMA_V1 {
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

fn validate_request(state: &DaemonState, request: &DaemonRequest) -> Result<()> {
    validate_request_shape(request)?;
    if request.schema == DAEMON_REQUEST_SCHEMA_V1 {
        if !state.insecure_allow_v1 {
            bail!("daemon protocol v1 is disabled");
        }
        if state.endpoint.transport != DaemonTransport::Tcp
            || !state.endpoint.address.ip().is_loopback()
        {
            bail!("daemon protocol v1 is allowed only over loopback TCP");
        }
        return Ok(());
    }
    let supplied = request
        .auth_token
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("daemon authentication failed"))?;
    if !constant_time_token_eq(supplied, &state.auth_token) {
        bail!("daemon authentication failed");
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
            let paths = collect_project_source_events(
                &root_for_handler,
                events.into_iter().map(|event| event.path),
            );
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

fn collect_project_source_events(
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
    let store = match daemon_composition(state) {
        Some(composition) => composition.init_store(root, &config).await?,
        None => init_store(root, &config).await?,
    };
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

fn daemon_composition(state: &DaemonState) -> Option<RuntimeComposition> {
    state.composition.clone()
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
    let cache_key = ContextCacheKey {
        snapshot_id,
        task: task.to_string(),
        level: format!("{level:?}"),
        limits,
    };
    if let Some(pack) = state
        .context_cache
        .lock()
        .map_err(|_| anyhow::anyhow!("daemon context cache lock is poisoned"))?
        .get(&cache_key)
    {
        return Ok(pack);
    }
    let direct_matches = if let Ok(index) = search_index_for_daemon(state, &snapshot) {
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
    let pack = generate_context_pack(&snapshot, task, level, limits, direct_matches);
    state
        .context_cache
        .lock()
        .map_err(|_| anyhow::anyhow!("daemon context cache lock is poisoned"))?
        .insert(cache_key, pack.clone());
    Ok(pack)
}

async fn daemon_overview_from_cache(
    state: &Arc<DaemonState>,
    top: usize,
) -> Result<RepositoryOverview> {
    let snapshot = latest_snapshot_for_daemon(state).await?;
    let snapshot_id = snapshot_id(&snapshot)?;
    let cache_key = OverviewCacheKey { snapshot_id, top };
    if let Some(overview) = state
        .overview_cache
        .lock()
        .map_err(|_| anyhow::anyhow!("daemon overview cache lock is poisoned"))?
        .get(&cache_key)
    {
        return Ok(overview);
    }
    let overview = build_repository_overview(&snapshot, top);
    state
        .overview_cache
        .lock()
        .map_err(|_| anyhow::anyhow!("daemon overview cache lock is poisoned"))?
        .insert(cache_key, overview.clone());
    Ok(overview)
}

async fn daemon_explain_from_cache(
    state: &Arc<DaemonState>,
    stable_key: &str,
) -> Result<crate::explain::EntityExplanation> {
    let snapshot = latest_snapshot_for_daemon(state).await?;
    explain_snapshot(&snapshot, stable_key)
}

async fn daemon_search_from_cache(
    state: &Arc<DaemonState>,
    query: String,
    limit: usize,
) -> Result<crate::search::SearchReport> {
    let snapshot = latest_snapshot_for_daemon(state).await?;
    let index = search_index_for_daemon(state, &snapshot)?;
    search_snapshot_with_index(
        &state.endpoint.root,
        &snapshot,
        query,
        limit,
        index.as_ref(),
    )
    .await
}

fn snapshot_id(snapshot: &CanonicalSnapshot) -> Result<String> {
    snapshot
        .snapshot
        .as_ref()
        .map(|snapshot| snapshot.0.clone())
        .ok_or_else(|| anyhow::anyhow!("latest canonical snapshot has no snapshot id"))
}

fn search_index_for_daemon(
    state: &DaemonState,
    snapshot: &CanonicalSnapshot,
) -> Result<Arc<dyn SearchIndex>> {
    let snapshot_id = snapshot_id(snapshot)?;
    let mut cache = state
        .search_index_cache
        .lock()
        .map_err(|_| anyhow::anyhow!("daemon search index cache lock is poisoned"))?;
    if let Some(cached) = cache.as_ref()
        && cached.snapshot_id == snapshot_id
    {
        return Ok(Arc::clone(&cached.index));
    }
    let index_dir = state
        .endpoint
        .root
        .join(".athanor/generated/current/search");
    let index = match daemon_composition(state) {
        Some(composition) => get_or_build_search_index_with_factory(
            snapshot,
            &snapshot_id,
            &index_dir,
            |directory, documents| composition.build_search_index(directory, documents),
        )?,
        None => get_or_build_search_index_sync(snapshot, &snapshot_id, &index_dir)?,
    };
    *cache = Some(CachedSearchIndex {
        snapshot_id,
        index: Arc::clone(&index),
    });
    Ok(index)
}

fn invalidate_daemon_caches(state: &DaemonState) {
    match state.latest_snapshot_cache.lock() {
        Ok(mut cache) => {
            *cache = None;
        }
        Err(_) => {
            tracing::warn!("daemon snapshot cache lock is poisoned");
        }
    }
    match state.search_index_cache.lock() {
        Ok(mut cache) => {
            *cache = None;
        }
        Err(_) => {
            tracing::warn!("daemon search index cache lock is poisoned");
        }
    }
    match state.overview_cache.lock() {
        Ok(mut cache) => cache.clear(),
        Err(_) => tracing::warn!("daemon overview cache lock is poisoned"),
    }
    match state.context_cache.lock() {
        Ok(mut cache) => cache.clear(),
        Err(_) => tracing::warn!("daemon context cache lock is poisoned"),
    }
}

fn lifecycle(state: &DaemonState) -> DaemonLifecycleState {
    state
        .lifecycle
        .lock()
        .map(|lifecycle| *lifecycle)
        .unwrap_or(DaemonLifecycleState::Stopping)
}

fn set_lifecycle(state: &DaemonState, lifecycle: DaemonLifecycleState) {
    match state.lifecycle.lock() {
        Ok(mut current) => *current = lifecycle,
        Err(_) => tracing::warn!("daemon lifecycle lock is poisoned"),
    }
}

fn active_job_count(state: &DaemonState) -> Result<usize> {
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

fn cache_status(state: &DaemonState) -> Value {
    serde_json::json!({
        "snapshot_loaded": state.latest_snapshot_cache.lock().is_ok_and(|cache| cache.is_some()),
        "search_index_loaded": state.search_index_cache.lock().is_ok_and(|cache| cache.is_some()),
        "overview_entries": state.overview_cache.lock().map_or(0, |cache| cache.entries.len()),
        "context_entries": state.context_cache.lock().map_or(0, |cache| cache.entries.len()),
    })
}

fn cancel_active_jobs(state: &DaemonState) {
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

async fn drain_active_jobs(state: &DaemonState, timeout: Duration) -> Result<()> {
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

fn start_index_job(state: &Arc<DaemonState>, description: String) -> Result<DaemonJob> {
    if has_active_job(state, DaemonJobKind::Index)? {
        bail!("index job is already queued or running");
    }
    let (job_id, cancellation) =
        start_cancellable_daemon_job(state, DaemonJobKind::Index, description)?;
    let mut job = get_daemon_job(state, &job_id)?;
    let job_state = Arc::clone(state);
    let job_id_for_task = job_id.clone();
    let root = state.endpoint.root.clone();
    let cancellation_for_task = cancellation.clone();
    let composition = daemon_composition(state);
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
                    runtime.block_on(async move {
                        let options = IndexOptions {
                            root,
                            validation_report: None,
                            validation_result: None,
                            validate_only: false,
                        };
                        match composition {
                            Some(composition) => {
                                index_project_cancellable_with_composition(
                                    options,
                                    cancellation_for_task,
                                    &composition,
                                )
                                .await
                            }
                            None => index_project_cancellable(options, cancellation_for_task).await,
                        }
                    })
                });
            match result {
                Ok(report) => {
                    invalidate_daemon_caches(&job_state);
                    if let Ok(mut snapshot) = job_state.last_successful_index.lock() {
                        *snapshot = Some(report.snapshot.clone());
                    }
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
                            "metrics": report.metrics,
                        })),
                        None,
                    );
                }
                Err(error) => {
                    let _ =
                        finish_cancellable_daemon_job_error(&job_state, &job_id_for_task, error);
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
    let current = get_daemon_job(state, job_id)?;
    match current.status {
        DaemonJobStatus::Queued => {
            if let Some(cancellation) = cancellation_token(state, job_id)? {
                cancellation.cancel();
            }
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
                    job.finished_at_unix_ms = Some(unix_time_ms()?);
                    job.error = Some("job cancelled before start".to_string());
                }
                DaemonJobStatus::Running => {
                    job.status = DaemonJobStatus::Cancelling;
                    job.error = Some("job cancellation requested".to_string());
                }
                _ => {}
            }
            let cancelled = job.clone();
            drop(jobs);
            if cancelled.status == DaemonJobStatus::Cancelled {
                remove_cancellation_token(state, job_id)?;
            }
            Ok(cancelled)
        }
        DaemonJobStatus::Running => {
            let cancellation = cancellation_token(state, job_id)?.ok_or_else(|| {
                anyhow::anyhow!("daemon job `{job_id}` is running and is not cancellable")
            })?;
            cancellation.cancel();
            let mut jobs = state
                .jobs
                .lock()
                .map_err(|_| anyhow::anyhow!("daemon job registry lock is poisoned"))?;
            let job = jobs
                .iter_mut()
                .find(|job| job.id == job_id)
                .ok_or_else(|| anyhow::anyhow!("daemon job `{job_id}` was not found"))?;
            if job.status == DaemonJobStatus::Running {
                job.status = DaemonJobStatus::Cancelling;
                job.error = Some("job cancellation requested".to_string());
            }
            Ok(job.clone())
        }
        DaemonJobStatus::Cancelling
        | DaemonJobStatus::Succeeded
        | DaemonJobStatus::Failed
        | DaemonJobStatus::Cancelled => Ok(current),
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
                DaemonJobStatus::Queued | DaemonJobStatus::Running | DaemonJobStatus::Cancelling
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

fn start_cancellable_daemon_job(
    state: &DaemonState,
    kind: DaemonJobKind,
    description: String,
) -> Result<(String, CancellationToken)> {
    let job_id = start_daemon_job(state, kind, description)?;
    let cancellation = CancellationToken::new();
    let mut tokens = match state.cancellation_tokens.lock() {
        Ok(tokens) => tokens,
        Err(_) => {
            let message = "daemon cancellation registry lock is poisoned".to_string();
            let _ = finish_daemon_job(
                state,
                &job_id,
                DaemonJobStatus::Failed,
                None,
                Some(message.clone()),
            );
            bail!(message);
        }
    };
    tokens.insert(job_id.clone(), cancellation.clone());
    Ok((job_id, cancellation))
}

fn cancellation_token(state: &DaemonState, job_id: &str) -> Result<Option<CancellationToken>> {
    let tokens = state
        .cancellation_tokens
        .lock()
        .map_err(|_| anyhow::anyhow!("daemon cancellation registry lock is poisoned"))?;
    Ok(tokens.get(job_id).cloned())
}

fn remove_cancellation_token(state: &DaemonState, job_id: &str) -> Result<()> {
    let mut tokens = state
        .cancellation_tokens
        .lock()
        .map_err(|_| anyhow::anyhow!("daemon cancellation registry lock is poisoned"))?;
    tokens.remove(job_id);
    Ok(())
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
        DaemonJobStatus::Running | DaemonJobStatus::Cancelling => Ok(true),
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
    drop(jobs);
    remove_cancellation_token(state, job_id)?;
    Ok(())
}

fn finish_cancellable_daemon_job_error(
    state: &DaemonState,
    job_id: &str,
    error: anyhow::Error,
) -> Result<()> {
    let cancelled = error
        .chain()
        .any(|cause| cause.to_string().contains("operation cancelled"));
    let (status, message) = if cancelled {
        (
            DaemonJobStatus::Cancelled,
            "operation cancelled".to_string(),
        )
    } else {
        (DaemonJobStatus::Failed, error.to_string())
    };
    finish_daemon_job(state, job_id, status, None, Some(message))
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
    if endpoint.protocol_version != DAEMON_PROTOCOL_VERSION {
        bail!(
            "unsupported daemon protocol version `{}`",
            endpoint.protocol_version
        );
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

fn cleanup_known_staging_artifacts(root: &Path) -> Result<()> {
    let roots = [
        root.join(".athanor/store/canonical/jsonl"),
        root.join(".athanor/generated"),
        root.join(".athanor/generated/current"),
    ];
    for directory in roots {
        cleanup_staging_directory(&directory)?;
    }
    Ok(())
}

fn cleanup_staging_directory(directory: &Path) -> Result<()> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to inspect {}", directory.display()));
        }
    };
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !(name.starts_with('.') && (name.contains(".tmp-") || name.contains(".backup-"))) {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            fs::remove_dir_all(&path)
                .with_context(|| format!("failed to remove stale staging {}", path.display()))?;
        } else {
            fs::remove_file(&path)
                .with_context(|| format!("failed to remove stale staging {}", path.display()))?;
        }
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
fn local_socket_endpoint(runtime_dir: &Path, _runtime_id: &str) -> Result<LocalSocketEndpoint> {
    const MAX_UNIX_SOCKET_PATH_BYTES: usize = 100;

    let runtime_socket_path = runtime_dir.join("daemon.sock");
    let (socket_path, directory_path) =
        if runtime_socket_path.as_os_str().as_encoded_bytes().len() <= MAX_UNIX_SOCKET_PATH_BYTES {
            (runtime_socket_path, None)
        } else {
            let directory_path = env::temp_dir().join(format!("athanor-{_runtime_id}"));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn serves_status_and_rejects_wrong_project() {
        let root = temp_root("status");
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let runtime_dir = root.join(".athanor/daemon");
        fs::create_dir_all(&runtime_dir).unwrap();
        let token_path = runtime_dir.join("token");
        let auth_token = "a".repeat(crate::DAEMON_TOKEN_BYTES * 2);
        fs::write(&token_path, format!("{auth_token}\n")).unwrap();
        let endpoint = DaemonEndpoint {
            schema: DAEMON_ENDPOINT_SCHEMA.to_string(),
            protocol_version: DAEMON_PROTOCOL_VERSION,
            athanor_version: env!("CARGO_PKG_VERSION").to_string(),
            runtime_id: "runtime-test".to_string(),
            token_path: token_path.clone(),
            project_id: "alpha".to_string(),
            root: root.clone(),
            registry_path: root.join("projects.json"),
            address,
            transport: DaemonTransport::Tcp,
            local_socket_path: None,
            windows_pipe_name: None,
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
            composition: None,
            auth_token,
            insecure_allow_v1: false,
            lifecycle: Mutex::new(DaemonLifecycleState::Running),
            last_successful_index: Mutex::new(None),
            endpoint,
            jobs: Mutex::new(Vec::new()),
            next_job_sequence: Mutex::new(1),
            max_job_history: 100,
            latest_snapshot_cache: Mutex::new(None),
            search_index_cache: Mutex::new(None),
            overview_cache: Mutex::new(BoundedCache::new(DERIVED_CACHE_CAPACITY)),
            context_cache: Mutex::new(BoundedCache::new(DERIVED_CACHE_CAPACITY)),
            cancellation_tokens: Mutex::new(HashMap::new()),
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
                protocol_version: DAEMON_PROTOCOL_VERSION,
                athanor_version: env!("CARGO_PKG_VERSION").to_string(),
                runtime_id: "runtime-test".to_string(),
                token_path,
                project_id: "alpha".to_string(),
                root: root.clone(),
                registry_path: root.join("projects.json"),
                address,
                transport: DaemonTransport::Tcp,
                local_socket_path: None,
                windows_pipe_name: None,
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
            DaemonClientOptions {
                root: root.clone(),
                runtime_dir: Some(root.join(".athanor/daemon")),
            },
            DaemonRequest {
                schema: DAEMON_REQUEST_SCHEMA.to_string(),
                request_id: "req-1".to_string(),
                project_id: "alpha".to_string(),
                auth_token: None,
                command: DaemonCommand::Status,
            },
        )
        .await
        .unwrap();
        assert!(response.ok);
        assert_eq!(response.project_id, "alpha");
        assert!(!task.await.unwrap());

        let error = request_daemon(
            DaemonClientOptions {
                root: root.clone(),
                runtime_dir: Some(root.join(".athanor/daemon")),
            },
            DaemonRequest {
                schema: DAEMON_REQUEST_SCHEMA.to_string(),
                request_id: "req-2".to_string(),
                project_id: "beta".to_string(),
                auth_token: None,
                command: DaemonCommand::Status,
            },
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("belongs to project `alpha`"));
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn serves_status_over_local_socket_transport() {
        let root = temp_root("local-socket");
        let serve_root = root.clone();
        let serve_task = tokio::spawn(async move {
            serve_daemon_with_composition(
                DaemonServeOptions {
                    project_id: "alpha".to_string(),
                    root: serve_root.clone(),
                    registry_path: serve_root.join("projects.json"),
                    listen: "127.0.0.1:0".parse().unwrap(),
                    transport: DaemonTransport::LocalSocket,
                    max_concurrent_requests: 4,
                    max_job_history: 100,
                    watch: false,
                    watch_poll: false,
                    debounce_ms: 1000,
                    max_request_bytes: DEFAULT_MAX_REQUEST_BYTES,
                    max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
                    insecure_allow_v1: false,
                    runtime_dir: Some(serve_root.join(".athanor/daemon")),
                    shutdown_timeout: Duration::from_secs(5),
                },
                crate::test_runtime::composition(),
            )
            .await
        });

        let response = request_status_with_retry(&root).await;
        assert!(response.ok);
        assert_eq!(response.project_id, "alpha");

        let shutdown = request_daemon(
            DaemonClientOptions {
                root: root.clone(),
                runtime_dir: Some(root.join(".athanor/daemon")),
            },
            DaemonRequest {
                schema: DAEMON_REQUEST_SCHEMA.to_string(),
                request_id: "req-stop".to_string(),
                project_id: "alpha".to_string(),
                auth_token: None,
                command: DaemonCommand::Shutdown,
            },
        )
        .await
        .unwrap();
        assert!(shutdown.ok);

        tokio::time::timeout(std::time::Duration::from_secs(5), serve_task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn lists_daemon_jobs_newest_first_with_limit() {
        let endpoint = DaemonEndpoint {
            schema: DAEMON_ENDPOINT_SCHEMA.to_string(),
            protocol_version: DAEMON_PROTOCOL_VERSION,
            athanor_version: env!("CARGO_PKG_VERSION").to_string(),
            runtime_id: "runtime-test".to_string(),
            token_path: PathBuf::from("token"),
            project_id: "alpha".to_string(),
            root: PathBuf::from("."),
            registry_path: PathBuf::from("projects.json"),
            address: "127.0.0.1:1".parse().unwrap(),
            transport: DaemonTransport::Tcp,
            local_socket_path: None,
            windows_pipe_name: None,
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
            composition: None,
            auth_token: "test-token".to_string(),
            insecure_allow_v1: false,
            lifecycle: Mutex::new(DaemonLifecycleState::Running),
            last_successful_index: Mutex::new(None),
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
            search_index_cache: Mutex::new(None),
            overview_cache: Mutex::new(BoundedCache::new(DERIVED_CACHE_CAPACITY)),
            context_cache: Mutex::new(BoundedCache::new(DERIVED_CACHE_CAPACITY)),
            cancellation_tokens: Mutex::new(HashMap::new()),
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
            protocol_version: DAEMON_PROTOCOL_VERSION,
            athanor_version: env!("CARGO_PKG_VERSION").to_string(),
            runtime_id: "runtime-test".to_string(),
            token_path: PathBuf::from("token"),
            project_id: "alpha".to_string(),
            root: PathBuf::from("."),
            registry_path: PathBuf::from("projects.json"),
            address: "127.0.0.1:1".parse().unwrap(),
            transport: DaemonTransport::Tcp,
            local_socket_path: None,
            windows_pipe_name: None,
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
            composition: None,
            auth_token: "test-token".to_string(),
            insecure_allow_v1: false,
            lifecycle: Mutex::new(DaemonLifecycleState::Running),
            last_successful_index: Mutex::new(None),
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
            search_index_cache: Mutex::new(None),
            overview_cache: Mutex::new(BoundedCache::new(DERIVED_CACHE_CAPACITY)),
            context_cache: Mutex::new(BoundedCache::new(DERIVED_CACHE_CAPACITY)),
            cancellation_tokens: Mutex::new(HashMap::new()),
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
    async fn rejects_missing_and_wrong_tokens_before_job_creation() {
        let root = temp_root("auth-rejection");
        let state = Arc::new(test_daemon_state(&root, false));

        for auth_token in [None, Some("wrong-token".to_string())] {
            let (response, shutdown) = execute_request(
                Arc::clone(&state),
                DaemonRequest {
                    schema: DAEMON_REQUEST_SCHEMA.to_string(),
                    request_id: "req-auth".to_string(),
                    project_id: "alpha".to_string(),
                    auth_token,
                    command: DaemonCommand::Overview { top: 1 },
                },
            )
            .await;
            assert!(!response.ok);
            assert!(!shutdown);
            assert_eq!(
                response.error.as_deref(),
                Some("daemon authentication failed")
            );
        }

        assert!(state.jobs.lock().unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn protocol_v1_requires_explicit_insecure_compatibility() {
        let root = temp_root("v1-compatibility");
        let disabled = Arc::new(test_daemon_state(&root, false));
        let request = || DaemonRequest {
            schema: DAEMON_REQUEST_SCHEMA_V1.to_string(),
            request_id: "req-v1".to_string(),
            project_id: "alpha".to_string(),
            auth_token: None,
            command: DaemonCommand::Status,
        };

        let (rejected, _) = execute_request(disabled, request()).await;
        assert!(!rejected.ok);
        assert_eq!(
            rejected.error.as_deref(),
            Some("daemon protocol v1 is disabled")
        );

        let enabled = Arc::new(test_daemon_state(&root, true));
        let (accepted, _) = execute_request(enabled, request()).await;
        assert!(accepted.ok);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn malformed_request_shape_is_rejected_without_work() {
        let root = temp_root("malformed-request-shape");
        let state = Arc::new(test_daemon_state(&root, false));

        for (request, expected_error) in [
            (
                DaemonRequest {
                    schema: "athanor.daemon_request.v999".to_string(),
                    request_id: "req-bad-schema".to_string(),
                    project_id: "alpha".to_string(),
                    auth_token: Some(state.auth_token.clone()),
                    command: DaemonCommand::Status,
                },
                "unsupported daemon request schema",
            ),
            (
                DaemonRequest {
                    schema: DAEMON_REQUEST_SCHEMA.to_string(),
                    request_id: "x".repeat(129),
                    project_id: "alpha".to_string(),
                    auth_token: Some(state.auth_token.clone()),
                    command: DaemonCommand::Status,
                },
                "request_id must contain 1-128 characters",
            ),
            (
                DaemonRequest {
                    schema: DAEMON_REQUEST_SCHEMA.to_string(),
                    request_id: "req-empty-project".to_string(),
                    project_id: String::new(),
                    auth_token: Some(state.auth_token.clone()),
                    command: DaemonCommand::Status,
                },
                "project_id must not be empty",
            ),
        ] {
            let (response, shutdown) = execute_request(Arc::clone(&state), request).await;
            assert!(!response.ok);
            assert!(!shutdown);
            assert!(
                response
                    .error
                    .as_deref()
                    .unwrap_or_default()
                    .contains(expected_error)
            );
        }

        assert!(state.jobs.lock().unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn stopping_daemon_allows_lifecycle_reads_but_rejects_new_work() {
        let root = temp_root("stopping-lifecycle");
        let state = Arc::new(test_daemon_state(&root, false));
        let job_id = start_daemon_job(
            &state,
            DaemonJobKind::Overview,
            "completed overview".to_string(),
        )
        .unwrap();
        finish_daemon_job(
            &state,
            &job_id,
            DaemonJobStatus::Succeeded,
            Some(serde_json::json!({"ok": true})),
            None,
        )
        .unwrap();
        set_lifecycle(&state, DaemonLifecycleState::Stopping);

        let token = Some(state.auth_token.clone());
        for command in [
            DaemonCommand::Status,
            DaemonCommand::Jobs { limit: 10 },
            DaemonCommand::Job {
                job_id: job_id.clone(),
            },
        ] {
            let (response, shutdown) = execute_request(
                Arc::clone(&state),
                DaemonRequest {
                    schema: DAEMON_REQUEST_SCHEMA.to_string(),
                    request_id: "req-lifecycle-read".to_string(),
                    project_id: "alpha".to_string(),
                    auth_token: token.clone(),
                    command,
                },
            )
            .await;
            assert!(response.ok);
            assert!(!shutdown);
        }

        let (response, shutdown) = execute_request(
            Arc::clone(&state),
            DaemonRequest {
                schema: DAEMON_REQUEST_SCHEMA.to_string(),
                request_id: "req-new-work".to_string(),
                project_id: "alpha".to_string(),
                auth_token: token,
                command: DaemonCommand::Index,
            },
        )
        .await;
        assert!(!response.ok);
        assert!(!shutdown);
        assert_eq!(
            response.error.as_deref(),
            Some("daemon is stopping and does not accept new work")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn invalid_query_parameters_are_rejected_without_work() {
        let root = temp_root("invalid-query-parameters");
        let state = Arc::new(test_daemon_state(&root, false));
        let token = Some(state.auth_token.clone());

        for (command, expected_error) in [
            (DaemonCommand::Jobs { limit: 0 }, "jobs limit"),
            (DaemonCommand::Overview { top: 0 }, "overview top"),
            (
                DaemonCommand::Explain {
                    stable_key: "  ".to_string(),
                },
                "stable key must not be empty",
            ),
            (
                DaemonCommand::Search {
                    query: "  ".to_string(),
                    limit: 10,
                },
                "search query must not be empty",
            ),
            (
                DaemonCommand::Search {
                    query: "login".to_string(),
                    limit: 0,
                },
                "search limit",
            ),
            (
                DaemonCommand::Context {
                    task: "  ".to_string(),
                    diff: false,
                    level: ContextLevel::Normal,
                    limits: ContextLimitOverrides::default(),
                },
                "context task must not be empty",
            ),
        ] {
            let (response, shutdown) = execute_request(
                Arc::clone(&state),
                DaemonRequest {
                    schema: DAEMON_REQUEST_SCHEMA.to_string(),
                    request_id: "req-invalid-query".to_string(),
                    project_id: "alpha".to_string(),
                    auth_token: token.clone(),
                    command,
                },
            )
            .await;
            assert!(!response.ok);
            assert!(!shutdown);
            assert!(
                response
                    .error
                    .as_deref()
                    .unwrap_or_default()
                    .contains(expected_error)
            );
        }

        assert!(state.jobs.lock().unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn cancelling_running_read_only_job_returns_non_cancellable_error() {
        let root = temp_root("cancel-read-only");
        let state = Arc::new(test_daemon_state(&root, false));
        let job_id =
            start_daemon_job(&state, DaemonJobKind::Search, "running search".to_string()).unwrap();
        assert!(mark_daemon_job_running(&state, &job_id).unwrap());

        let (response, shutdown) = execute_request(
            Arc::clone(&state),
            DaemonRequest {
                schema: DAEMON_REQUEST_SCHEMA.to_string(),
                request_id: "req-cancel-read-only".to_string(),
                project_id: "alpha".to_string(),
                auth_token: Some(state.auth_token.clone()),
                command: DaemonCommand::Cancel {
                    job_id: job_id.clone(),
                },
            },
        )
        .await;

        assert!(!response.ok);
        assert!(!shutdown);
        assert!(
            response
                .error
                .as_deref()
                .unwrap_or_default()
                .contains("is running and is not cancellable")
        );
        let job = get_daemon_job(&state, &job_id).unwrap();
        assert_eq!(job.status, DaemonJobStatus::Running);
        assert!(job.finished_at_unix_ms.is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn duplicate_writable_jobs_are_rejected_without_starting_new_work() {
        for (label, kind, command, expected_error) in [
            (
                "index",
                DaemonJobKind::Index,
                DaemonCommand::Index,
                "index job is already queued or running",
            ),
            (
                "generate",
                DaemonJobKind::Generate,
                DaemonCommand::Generate,
                "generate job is already queued or running",
            ),
            (
                "wiki",
                DaemonJobKind::Wiki,
                DaemonCommand::Wiki,
                "wiki job is already queued or running",
            ),
            (
                "html",
                DaemonJobKind::HtmlReport,
                DaemonCommand::HtmlReport,
                "HTML report job is already queued or running",
            ),
        ] {
            let root = temp_root(&format!("duplicate-{label}"));
            let state = Arc::new(test_daemon_state(&root, false));
            let existing = start_daemon_job(&state, kind, format!("running {label} job")).unwrap();
            assert!(mark_daemon_job_running(&state, &existing).unwrap());

            let (response, shutdown) = execute_request(
                Arc::clone(&state),
                DaemonRequest {
                    schema: DAEMON_REQUEST_SCHEMA.to_string(),
                    request_id: format!("req-duplicate-{label}"),
                    project_id: "alpha".to_string(),
                    auth_token: Some(state.auth_token.clone()),
                    command,
                },
            )
            .await;

            assert!(!response.ok);
            assert!(!shutdown);
            assert_eq!(response.error.as_deref(), Some(expected_error));
            assert_eq!(list_daemon_jobs(&state, 10).unwrap().total, 1);
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[tokio::test]
    async fn protocol_cancel_queued_writable_job_finishes_and_removes_token() {
        let root = temp_root("cancel-queued-writable");
        let state = Arc::new(test_daemon_state(&root, false));
        let (job_id, cancellation) =
            start_cancellable_daemon_job(&state, DaemonJobKind::Index, "queued index".to_string())
                .unwrap();

        let (response, shutdown) = execute_request(
            Arc::clone(&state),
            DaemonRequest {
                schema: DAEMON_REQUEST_SCHEMA.to_string(),
                request_id: "req-cancel-queued".to_string(),
                project_id: "alpha".to_string(),
                auth_token: Some(state.auth_token.clone()),
                command: DaemonCommand::Cancel {
                    job_id: job_id.clone(),
                },
            },
        )
        .await;

        assert!(response.ok);
        assert!(!shutdown);
        assert!(cancellation.is_cancelled());
        let job = get_daemon_job(&state, &job_id).unwrap();
        assert_eq!(job.status, DaemonJobStatus::Cancelled);
        assert_eq!(job.error.as_deref(), Some("job cancelled before start"));
        assert!(job.finished_at_unix_ms.is_some());
        assert!(cancellation_token(&state, &job_id).unwrap().is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn client_disconnect_before_request_does_not_create_work() {
        let root = temp_root("disconnect-before-request");
        let state = Arc::new(test_daemon_state(&root, false));
        let (server, client) = tokio::io::duplex(64);
        drop(client);

        let shutdown = handle_connection(server, Arc::clone(&state)).await.unwrap();

        assert!(!shutdown);
        assert!(state.jobs.lock().unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn invalid_request_json_gets_structured_error_without_work() {
        let root = temp_root("invalid-request-json");
        let state = Arc::new(test_daemon_state(&root, false));
        let (server, mut client) = tokio::io::duplex(256);
        let server_task = tokio::spawn(handle_connection(server, Arc::clone(&state)));

        client.write_all(b"{not-json\n").await.unwrap();
        client.shutdown().await.unwrap();

        let mut response = Vec::new();
        client.read_to_end(&mut response).await.unwrap();
        let response: DaemonResponse = serde_json::from_slice(&response).unwrap();
        assert!(!response.ok);
        assert_eq!(response.project_id, "alpha");
        assert!(
            response
                .error
                .as_deref()
                .unwrap_or_default()
                .contains("invalid daemon request JSON")
        );
        assert!(!server_task.await.unwrap().unwrap());
        assert!(state.jobs.lock().unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn request_project_mismatch_is_rejected_without_work() {
        let root = temp_root("project-mismatch");
        let state = Arc::new(test_daemon_state(&root, false));

        let (response, shutdown) = execute_request(
            Arc::clone(&state),
            DaemonRequest {
                schema: DAEMON_REQUEST_SCHEMA.to_string(),
                request_id: "req-project-mismatch".to_string(),
                project_id: "beta".to_string(),
                auth_token: Some(state.auth_token.clone()),
                command: DaemonCommand::Status,
            },
        )
        .await;

        assert!(!response.ok);
        assert!(!shutdown);
        assert!(
            response
                .error
                .as_deref()
                .unwrap_or_default()
                .contains("does not match daemon project")
        );
        assert!(state.jobs.lock().unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn oversized_request_gets_structured_error_without_work() {
        let root = temp_root("oversized-request");
        let mut state = test_daemon_state(&root, false);
        state.endpoint.max_request_bytes = 32;
        let state = Arc::new(state);
        let (server, mut client) = tokio::io::duplex(256);
        let server_task = tokio::spawn(handle_connection(server, Arc::clone(&state)));

        client.write_all(&[b'x'; 64]).await.unwrap();
        client.write_all(b"\n").await.unwrap();
        client.shutdown().await.unwrap();

        let mut response = Vec::new();
        client.read_to_end(&mut response).await.unwrap();
        let response: DaemonResponse = serde_json::from_slice(&response).unwrap();
        assert!(!response.ok);
        assert_eq!(response.project_id, "alpha");
        assert!(
            response
                .error
                .as_deref()
                .unwrap_or_default()
                .contains("request exceeds size limit")
        );
        assert!(!server_task.await.unwrap().unwrap());
        assert!(state.jobs.lock().unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn client_rejects_empty_and_invalid_wire_responses() {
        let root = temp_root("client-invalid-response");
        let endpoint = test_daemon_state(&root, false).endpoint;
        let request = DaemonRequest {
            schema: DAEMON_REQUEST_SCHEMA.to_string(),
            request_id: "req-client-invalid-response".to_string(),
            project_id: "alpha".to_string(),
            auth_token: Some("a".repeat(crate::DAEMON_TOKEN_BYTES * 2)),
            command: DaemonCommand::Status,
        };

        let (mut server, client) = tokio::io::duplex(4096);
        let empty_server_task = tokio::spawn(async move {
            let mut request_bytes = Vec::new();
            server.read_to_end(&mut request_bytes).await.unwrap();
            request_bytes
        });
        let error = request_daemon_over_stream(client, &endpoint, &request)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("empty response"));
        assert!(!empty_server_task.await.unwrap().is_empty());

        let (mut server, client) = tokio::io::duplex(4096);
        let server_task = tokio::spawn(async move {
            let mut request_bytes = Vec::new();
            server.read_to_end(&mut request_bytes).await.unwrap();
            server.write_all(b"not-json").await.unwrap();
            server.shutdown().await.unwrap();
            request_bytes
        });
        let error = request_daemon_over_stream(client, &endpoint, &request)
            .await
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("failed to parse daemon response")
        );
        assert!(!server_task.await.unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn client_refuses_to_send_oversized_request() {
        let root = temp_root("client-oversized-request");
        let mut endpoint = test_daemon_state(&root, false).endpoint;
        endpoint.max_request_bytes = 64;
        let (mut server, client) = tokio::io::duplex(4096);
        let request = DaemonRequest {
            schema: DAEMON_REQUEST_SCHEMA.to_string(),
            request_id: "req-client-oversized-request".repeat(8),
            project_id: "alpha".to_string(),
            auth_token: Some("a".repeat(crate::DAEMON_TOKEN_BYTES * 2)),
            command: DaemonCommand::Status,
        };

        let error = request_daemon_over_stream(client, &endpoint, &request)
            .await
            .unwrap_err();

        assert!(error.to_string().contains("daemon request exceeds"));
        let mut written = Vec::new();
        server.read_to_end(&mut written).await.unwrap();
        assert!(written.is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn client_rejects_oversized_wire_response() {
        let root = temp_root("client-oversized-response");
        let mut endpoint = test_daemon_state(&root, false).endpoint;
        endpoint.max_response_bytes = 64;
        let (mut server, client) = tokio::io::duplex(4096);
        let request = DaemonRequest {
            schema: DAEMON_REQUEST_SCHEMA.to_string(),
            request_id: "req-client-oversized-response".to_string(),
            project_id: "alpha".to_string(),
            auth_token: Some("a".repeat(crate::DAEMON_TOKEN_BYTES * 2)),
            command: DaemonCommand::Status,
        };
        let server_task = tokio::spawn(async move {
            let mut request_bytes = Vec::new();
            server.read_to_end(&mut request_bytes).await.unwrap();
            server.write_all(&[b'x'; 65]).await.unwrap();
            server.shutdown().await.unwrap();
            request_bytes
        });

        let error = request_daemon_over_stream(client, &endpoint, &request)
            .await
            .unwrap_err();

        assert!(error.to_string().contains("daemon response exceeds"));
        let request_bytes = server_task.await.unwrap();
        assert!(!request_bytes.is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn rejects_unsafe_serve_options_before_binding() {
        let root = temp_root("unsafe-serve-options");
        let options = |label: &str| DaemonServeOptions {
            project_id: format!("alpha-{label}"),
            root: root.clone(),
            registry_path: root.join("projects.json"),
            listen: "127.0.0.1:0".parse().unwrap(),
            transport: DaemonTransport::Tcp,
            max_concurrent_requests: 4,
            max_job_history: 100,
            watch: false,
            watch_poll: false,
            debounce_ms: 1000,
            max_request_bytes: DEFAULT_MAX_REQUEST_BYTES,
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
            insecure_allow_v1: false,
            runtime_dir: Some(root.join(format!(".athanor/daemon-{label}"))),
            shutdown_timeout: Duration::from_secs(5),
        };

        let mut watch_poll_without_watch = options("watch-poll");
        watch_poll_without_watch.watch_poll = true;
        let error = serve_daemon(watch_poll_without_watch).await.unwrap_err();
        assert!(error.to_string().contains("watch_poll requires --watch"));

        let mut invalid_debounce = options("debounce");
        invalid_debounce.watch = true;
        invalid_debounce.debounce_ms = 99;
        let error = serve_daemon(invalid_debounce).await.unwrap_err();
        assert!(error.to_string().contains("debounce_ms"));

        let mut non_loopback = options("non-loopback");
        non_loopback.listen = "192.0.2.1:0".parse().unwrap();
        let error = serve_daemon(non_loopback).await.unwrap_err();
        assert!(error.to_string().contains("loopback"));

        let mut oversized_protocol_limit = options("protocol-limit");
        oversized_protocol_limit.max_request_bytes = MAX_PROTOCOL_BYTES + 1;
        let error = serve_daemon(oversized_protocol_limit).await.unwrap_err();
        assert!(error.to_string().contains("max_request_bytes"));

        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn client_rejects_stale_or_corrupt_runtime_metadata_before_connecting() {
        let root = temp_root("stale-runtime-metadata");
        let runtime_dir = root.join(".athanor/daemon");
        fs::create_dir_all(&runtime_dir).unwrap();
        let token = "a".repeat(crate::DAEMON_TOKEN_BYTES * 2);
        fs::write(runtime_dir.join("token"), format!("{token}\n")).unwrap();
        let mut endpoint = test_daemon_state(&root, false).endpoint;
        endpoint.token_path = runtime_dir.join("other-token");
        write_endpoint(&runtime_dir.join("endpoint.json"), &endpoint).unwrap();

        let request = || DaemonRequest {
            schema: DAEMON_REQUEST_SCHEMA.to_string(),
            request_id: "req-stale".to_string(),
            project_id: "alpha".to_string(),
            auth_token: None,
            command: DaemonCommand::Status,
        };

        fs::write(runtime_dir.join("endpoint.json"), "{not-json\n").unwrap();
        let error = request_daemon(
            DaemonClientOptions {
                root: root.clone(),
                runtime_dir: Some(runtime_dir.clone()),
            },
            request(),
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("failed to parse"));

        write_endpoint(&runtime_dir.join("endpoint.json"), &endpoint).unwrap();
        let error = request_daemon(
            DaemonClientOptions {
                root: root.clone(),
                runtime_dir: Some(runtime_dir.clone()),
            },
            request(),
        )
        .await
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("endpoint token path does not match")
        );

        endpoint.token_path = runtime_dir.join("token");
        endpoint.schema = "athanor.daemon_endpoint.v999".to_string();
        write_endpoint(&runtime_dir.join("endpoint.json"), &endpoint).unwrap();
        let error = request_daemon(
            DaemonClientOptions {
                root: root.clone(),
                runtime_dir: Some(runtime_dir.clone()),
            },
            request(),
        )
        .await
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("unsupported daemon endpoint schema")
        );

        endpoint.schema = DAEMON_ENDPOINT_SCHEMA.to_string();
        endpoint.protocol_version = DAEMON_PROTOCOL_VERSION + 1;
        write_endpoint(&runtime_dir.join("endpoint.json"), &endpoint).unwrap();
        let error = request_daemon(
            DaemonClientOptions {
                root: root.clone(),
                runtime_dir: Some(runtime_dir.clone()),
            },
            request(),
        )
        .await
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("unsupported daemon protocol version")
        );

        endpoint.protocol_version = DAEMON_PROTOCOL_VERSION;
        write_endpoint(&runtime_dir.join("endpoint.json"), &endpoint).unwrap();
        fs::write(runtime_dir.join("token"), "not-a-token\n").unwrap();
        let error = request_daemon(
            DaemonClientOptions {
                root: root.clone(),
                runtime_dir: Some(runtime_dir),
            },
            request(),
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("daemon token is invalid"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn endpoint_defaults_to_tcp_when_optional_transport_metadata_is_absent() {
        let endpoint: DaemonEndpoint = serde_json::from_value(serde_json::json!({
            "schema": DAEMON_ENDPOINT_SCHEMA,
            "protocol_version": DAEMON_PROTOCOL_VERSION,
            "athanor_version": env!("CARGO_PKG_VERSION"),
            "runtime_id": "runtime-test",
            "token_path": "token",
            "project_id": "alpha",
            "root": ".",
            "registry_path": "projects.json",
            "address": "127.0.0.1:7",
            "pid": 1,
            "started_at_unix_ms": 1,
            "max_concurrent_requests": 4,
            "max_job_history": 100
        }))
        .unwrap();

        assert_eq!(endpoint.transport, DaemonTransport::Tcp);
        assert!(endpoint.local_socket_path.is_none());
        assert!(endpoint.windows_pipe_name.is_none());
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
            composition: None,
            auth_token: "test-token".to_string(),
            insecure_allow_v1: false,
            lifecycle: Mutex::new(DaemonLifecycleState::Running),
            last_successful_index: Mutex::new(None),
            endpoint: DaemonEndpoint {
                schema: DAEMON_ENDPOINT_SCHEMA.to_string(),
                protocol_version: DAEMON_PROTOCOL_VERSION,
                athanor_version: env!("CARGO_PKG_VERSION").to_string(),
                runtime_id: "runtime-test".to_string(),
                token_path: PathBuf::from("token"),
                project_id: "alpha".to_string(),
                root: PathBuf::from("."),
                registry_path: PathBuf::from("projects.json"),
                address: "127.0.0.1:1".parse().unwrap(),
                transport: DaemonTransport::Tcp,
                local_socket_path: None,
                windows_pipe_name: None,
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
            search_index_cache: Mutex::new(None),
            overview_cache: Mutex::new(BoundedCache::new(DERIVED_CACHE_CAPACITY)),
            context_cache: Mutex::new(BoundedCache::new(DERIVED_CACHE_CAPACITY)),
            cancellation_tokens: Mutex::new(HashMap::new()),
        });

        let explanation = daemon_explain_from_cache(&state, "api://POST:/login")
            .await
            .unwrap();

        assert_eq!(explanation.schema, "athanor.entity_explanation.v1");
        assert_eq!(explanation.snapshot, "snap_cached");
        assert_eq!(explanation.entity.name, "POST /login");
    }

    #[tokio::test]
    async fn searches_entities_from_hot_snapshot_cache() {
        let root = temp_root("search-cache");
        let snapshot = CanonicalSnapshot {
            snapshot: Some(athanor_domain::SnapshotId("snap_search".to_string())),
            entities: vec![athanor_domain::Entity {
                id: athanor_domain::EntityId("ent_login".to_string()),
                stable_key: athanor_domain::StableKey("api://POST:/login".to_string()),
                kind: athanor_domain::EntityKind::ApiEndpoint,
                name: "POST /login".to_string(),
                title: Some("Login endpoint".to_string()),
                source: None,
                language: None,
                aliases: vec!["auth login".to_string()],
                ownership: Vec::new(),
                payload: serde_json::json!({"summary": "Authenticate a user"}),
            }],
            facts: Vec::new(),
            relations: Vec::new(),
            diagnostics: Vec::new(),
        };
        let state = Arc::new(DaemonState {
            composition: None,
            auth_token: "test-token".to_string(),
            insecure_allow_v1: false,
            lifecycle: Mutex::new(DaemonLifecycleState::Running),
            last_successful_index: Mutex::new(None),
            endpoint: DaemonEndpoint {
                schema: DAEMON_ENDPOINT_SCHEMA.to_string(),
                protocol_version: DAEMON_PROTOCOL_VERSION,
                athanor_version: env!("CARGO_PKG_VERSION").to_string(),
                runtime_id: "runtime-test".to_string(),
                token_path: PathBuf::from("token"),
                project_id: "alpha".to_string(),
                root: root.clone(),
                registry_path: root.join("projects.json"),
                address: "127.0.0.1:1".parse().unwrap(),
                transport: DaemonTransport::Tcp,
                local_socket_path: None,
                windows_pipe_name: None,
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
            search_index_cache: Mutex::new(None),
            overview_cache: Mutex::new(BoundedCache::new(DERIVED_CACHE_CAPACITY)),
            context_cache: Mutex::new(BoundedCache::new(DERIVED_CACHE_CAPACITY)),
            cancellation_tokens: Mutex::new(HashMap::new()),
        });

        let report = daemon_search_from_cache(&state, "login".to_string(), 10)
            .await
            .unwrap();

        assert_eq!(report.schema, "athanor.search.v1");
        assert_eq!(report.snapshot, "snap_search");
        assert_eq!(report.returned, 1);
        assert_eq!(report.results[0].stable_key, "api://POST:/login");

        let first_index = state
            .search_index_cache
            .lock()
            .unwrap()
            .as_ref()
            .map(|cached| Arc::clone(&cached.index))
            .unwrap();
        let second_report = daemon_search_from_cache(&state, "auth".to_string(), 10)
            .await
            .unwrap();
        let second_index = state
            .search_index_cache
            .lock()
            .unwrap()
            .as_ref()
            .map(|cached| Arc::clone(&cached.index))
            .unwrap();

        assert_eq!(second_report.snapshot, "snap_search");
        assert!(Arc::ptr_eq(&first_index, &second_index));
    }

    #[tokio::test]
    async fn caches_derived_results_and_invalidates_the_full_cache_epoch() {
        let root = temp_root("derived-cache");
        let snapshot = CanonicalSnapshot {
            snapshot: Some(athanor_domain::SnapshotId("snap_derived".to_string())),
            entities: vec![athanor_domain::Entity {
                id: athanor_domain::EntityId("ent_login".to_string()),
                stable_key: athanor_domain::StableKey("api://POST:/login".to_string()),
                kind: athanor_domain::EntityKind::ApiEndpoint,
                name: "POST /login".to_string(),
                title: Some("Login endpoint".to_string()),
                source: None,
                language: None,
                aliases: vec!["auth login".to_string()],
                ownership: Vec::new(),
                payload: serde_json::json!({"summary": "Authenticate a user"}),
            }],
            facts: Vec::new(),
            relations: Vec::new(),
            diagnostics: Vec::new(),
        };
        let state = Arc::new(DaemonState {
            composition: None,
            auth_token: "test-token".to_string(),
            insecure_allow_v1: false,
            lifecycle: Mutex::new(DaemonLifecycleState::Running),
            last_successful_index: Mutex::new(None),
            endpoint: DaemonEndpoint {
                schema: DAEMON_ENDPOINT_SCHEMA.to_string(),
                protocol_version: DAEMON_PROTOCOL_VERSION,
                athanor_version: env!("CARGO_PKG_VERSION").to_string(),
                runtime_id: "runtime-test".to_string(),
                token_path: PathBuf::from("token"),
                project_id: "alpha".to_string(),
                root: root.clone(),
                registry_path: root.join("projects.json"),
                address: "127.0.0.1:1".parse().unwrap(),
                transport: DaemonTransport::Tcp,
                local_socket_path: None,
                windows_pipe_name: None,
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
            search_index_cache: Mutex::new(None),
            overview_cache: Mutex::new(BoundedCache::new(DERIVED_CACHE_CAPACITY)),
            context_cache: Mutex::new(BoundedCache::new(DERIVED_CACHE_CAPACITY)),
            cancellation_tokens: Mutex::new(HashMap::new()),
        });

        daemon_overview_from_cache(&state, 5).await.unwrap();
        daemon_overview_from_cache(&state, 5).await.unwrap();
        daemon_context_from_cache(
            &state,
            "login",
            ContextLevel::Normal,
            &ContextLimitOverrides::default(),
        )
        .await
        .unwrap();
        daemon_context_from_cache(
            &state,
            "login",
            ContextLevel::Normal,
            &ContextLimitOverrides::default(),
        )
        .await
        .unwrap();

        assert_eq!(state.overview_cache.lock().unwrap().entries.len(), 1);
        assert_eq!(state.context_cache.lock().unwrap().entries.len(), 1);
        assert!(state.search_index_cache.lock().unwrap().is_some());

        invalidate_daemon_caches(&state);

        assert!(state.latest_snapshot_cache.lock().unwrap().is_none());
        assert!(state.search_index_cache.lock().unwrap().is_none());
        assert!(state.overview_cache.lock().unwrap().entries.is_empty());
        assert!(state.context_cache.lock().unwrap().entries.is_empty());
    }

    #[test]
    fn bounded_cache_evicts_the_least_recently_used_entry() {
        let mut cache = BoundedCache::new(2);
        cache.insert("first", 1);
        cache.insert("second", 2);
        assert_eq!(cache.get(&"first"), Some(1));

        cache.insert("third", 3);

        assert_eq!(cache.get(&"second"), None);
        assert_eq!(cache.get(&"first"), Some(1));
        assert_eq!(cache.get(&"third"), Some(3));
    }

    #[test]
    fn detects_active_job_by_kind() {
        let endpoint = DaemonEndpoint {
            schema: DAEMON_ENDPOINT_SCHEMA.to_string(),
            protocol_version: DAEMON_PROTOCOL_VERSION,
            athanor_version: env!("CARGO_PKG_VERSION").to_string(),
            runtime_id: "runtime-test".to_string(),
            token_path: PathBuf::from("token"),
            project_id: "alpha".to_string(),
            root: PathBuf::from("."),
            registry_path: PathBuf::from("projects.json"),
            address: "127.0.0.1:1".parse().unwrap(),
            transport: DaemonTransport::Tcp,
            local_socket_path: None,
            windows_pipe_name: None,
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
            composition: None,
            auth_token: "test-token".to_string(),
            insecure_allow_v1: false,
            lifecycle: Mutex::new(DaemonLifecycleState::Running),
            last_successful_index: Mutex::new(None),
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
            search_index_cache: Mutex::new(None),
            overview_cache: Mutex::new(BoundedCache::new(DERIVED_CACHE_CAPACITY)),
            context_cache: Mutex::new(BoundedCache::new(DERIVED_CACHE_CAPACITY)),
            cancellation_tokens: Mutex::new(HashMap::new()),
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
    fn watcher_event_storm_is_deduplicated_and_skips_when_index_is_active() {
        let root = temp_root("watcher-storm");
        let source = root.join("src/lib.rs");
        let docs = root.join("docs/README.md");
        let generated = root.join(".athanor/generated/current/jsonl/entities.jsonl");
        let storm = (0..50)
            .flat_map(|_| [source.clone(), docs.clone(), generated.clone()])
            .collect::<Vec<_>>();

        let paths = collect_project_source_events(&root, storm);

        assert_eq!(paths, vec![docs.clone(), source.clone()]);

        let state = Arc::new(test_daemon_state(&root, false));
        let active_job =
            start_daemon_job(&state, DaemonJobKind::Index, "active index".to_string()).unwrap();
        assert!(mark_daemon_job_running(&state, &active_job).unwrap());

        let scheduled = start_watcher_index_job(&state, paths).unwrap();

        assert!(scheduled.is_none());
        assert_eq!(list_daemon_jobs(&state, 10).unwrap().total, 1);
        assert!(has_active_job(&state, DaemonJobKind::Index).unwrap());
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn polling_watcher_debounces_source_changes_and_ignores_artifacts() {
        let root = temp_root("polling-watcher");
        let source_dir = root.join("src");
        let docs_dir = root.join("docs");
        let artifact_dir = root.join(".athanor/generated/current/jsonl");
        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(&docs_dir).unwrap();
        fs::create_dir_all(&artifact_dir).unwrap();
        let source = source_dir.join("lib.rs");
        let docs = docs_dir.join("README.md");
        let artifact = artifact_dir.join("entities.jsonl");

        let (watch_tx, mut watch_rx) = mpsc::unbounded_channel();
        let _watcher = start_file_watcher(&root, Duration::from_millis(100), true, watch_tx)
            .expect("polling watcher should start");

        fs::write(&source, "pub fn changed() {}\n").unwrap();
        fs::write(&docs, "# Changed\n").unwrap();
        fs::write(&artifact, "{}\n").unwrap();

        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        let mut observed = Vec::new();
        while tokio::time::Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            match tokio::time::timeout(remaining, watch_rx.recv()).await {
                Ok(Some(paths)) => {
                    observed.extend(paths);
                    observed.sort();
                    observed.dedup();
                    if observed.contains(&source) && observed.contains(&docs) {
                        break;
                    }
                }
                Ok(None) => break,
                Err(_) => break,
            }
        }

        assert!(
            observed.contains(&source),
            "missing source event: {observed:?}"
        );
        assert!(observed.contains(&docs), "missing docs event: {observed:?}");
        assert!(
            !observed
                .iter()
                .any(|path| path.starts_with(root.join(".athanor"))),
            "artifact paths should be filtered: {observed:?}"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cancels_queued_jobs_and_requests_running_job_cancellation() {
        let endpoint = DaemonEndpoint {
            schema: DAEMON_ENDPOINT_SCHEMA.to_string(),
            protocol_version: DAEMON_PROTOCOL_VERSION,
            athanor_version: env!("CARGO_PKG_VERSION").to_string(),
            runtime_id: "runtime-test".to_string(),
            token_path: PathBuf::from("token"),
            project_id: "alpha".to_string(),
            root: PathBuf::from("."),
            registry_path: PathBuf::from("projects.json"),
            address: "127.0.0.1:1".parse().unwrap(),
            transport: DaemonTransport::Tcp,
            local_socket_path: None,
            windows_pipe_name: None,
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
        let running_cancellation = CancellationToken::new();
        let state = DaemonState {
            composition: None,
            auth_token: "test-token".to_string(),
            insecure_allow_v1: false,
            lifecycle: Mutex::new(DaemonLifecycleState::Running),
            last_successful_index: Mutex::new(None),
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
            search_index_cache: Mutex::new(None),
            overview_cache: Mutex::new(BoundedCache::new(DERIVED_CACHE_CAPACITY)),
            context_cache: Mutex::new(BoundedCache::new(DERIVED_CACHE_CAPACITY)),
            cancellation_tokens: Mutex::new(HashMap::from([(
                "job_00000002".to_string(),
                running_cancellation.clone(),
            )])),
        };

        let cancelled = cancel_daemon_job(&state, "job_00000001").unwrap();
        assert_eq!(cancelled.status, DaemonJobStatus::Cancelled);
        assert!(cancelled.finished_at_unix_ms.is_some());

        let cancelling = cancel_daemon_job(&state, "job_00000002").unwrap();
        assert_eq!(cancelling.status, DaemonJobStatus::Cancelling);
        assert!(cancelling.finished_at_unix_ms.is_none());
        assert!(running_cancellation.is_cancelled());
        assert!(has_active_job(&state, DaemonJobKind::Index).unwrap());

        finish_daemon_job(
            &state,
            "job_00000002",
            DaemonJobStatus::Cancelled,
            None,
            Some("operation cancelled".to_string()),
        )
        .unwrap();
        assert!(
            cancellation_token(&state, "job_00000002")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn cancellation_error_marks_index_and_generate_jobs_cancelled() {
        let root = temp_root("cancellable-job-finish");
        let state = test_daemon_state(&root, false);

        for kind in [DaemonJobKind::Index, DaemonJobKind::Generate] {
            let (job_id, cancellation) =
                start_cancellable_daemon_job(&state, kind.clone(), "cancellable".to_string())
                    .unwrap();
            assert!(mark_daemon_job_running(&state, &job_id).unwrap());

            let cancelling = cancel_daemon_job(&state, &job_id).unwrap();
            assert_eq!(cancelling.status, DaemonJobStatus::Cancelling);
            assert!(cancellation.is_cancelled());

            finish_cancellable_daemon_job_error(
                &state,
                &job_id,
                anyhow::anyhow!("operation cancelled"),
            )
            .unwrap();

            let finished = get_daemon_job(&state, &job_id).unwrap();
            assert_eq!(finished.kind, kind);
            assert_eq!(finished.status, DaemonJobStatus::Cancelled);
            assert_eq!(finished.error.as_deref(), Some("operation cancelled"));
            assert!(finished.finished_at_unix_ms.is_some());
            assert!(cancellation_token(&state, &job_id).unwrap().is_none());
        }

        assert!(!has_active_job(&state, DaemonJobKind::Index).unwrap());
        assert!(!has_active_job(&state, DaemonJobKind::Generate).unwrap());
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn cancelling_running_index_job_preserves_unpublished_artifacts() {
        let root = temp_root("cancel-running-index");
        let source_root = root.join("src");
        fs::create_dir_all(&source_root).unwrap();
        for index in 0..1_500 {
            fs::write(
                source_root.join(format!("module_{index:04}.rs")),
                format!("pub fn function_{index:04}() -> usize {{ {index} }}\n"),
            )
            .unwrap();
        }
        let state = Arc::new(test_daemon_state(&root, false));
        let started = start_index_job(&state, "cancellable real index".to_string()).unwrap();

        let mut running = None;
        for _ in 0..500 {
            let job = get_daemon_job(&state, &started.id).unwrap();
            if job.status == DaemonJobStatus::Running {
                running = Some(job);
                break;
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
        assert!(running.is_some(), "index job never entered running state");

        let cancelling = cancel_daemon_job(&state, &started.id).unwrap();
        assert_eq!(cancelling.status, DaemonJobStatus::Cancelling);

        let mut finished = None;
        for _ in 0..1_000 {
            let job = get_daemon_job(&state, &started.id).unwrap();
            if matches!(
                job.status,
                DaemonJobStatus::Cancelled | DaemonJobStatus::Failed | DaemonJobStatus::Succeeded
            ) {
                finished = Some(job);
                break;
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
        let finished = finished.expect("index job did not finish after cancellation");
        assert_eq!(finished.status, DaemonJobStatus::Cancelled);
        assert_eq!(finished.error.as_deref(), Some("operation cancelled"));
        assert!(cancellation_token(&state, &started.id).unwrap().is_none());
        assert!(!root.join(".athanor/state/index-state.json").exists());
        assert!(!root.join(".athanor/generated/current/jsonl").exists());
        assert!(
            !root
                .join(".athanor/store/canonical/jsonl/latest.json")
                .exists()
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn shutdown_drain_times_out_until_active_jobs_finish() {
        let root = temp_root("shutdown-drain");
        let state = test_daemon_state(&root, false);
        let (job_id, cancellation) = start_cancellable_daemon_job(
            &state,
            DaemonJobKind::Index,
            "long running index".to_string(),
        )
        .unwrap();
        assert!(mark_daemon_job_running(&state, &job_id).unwrap());

        cancel_active_jobs(&state);

        let cancelling = get_daemon_job(&state, &job_id).unwrap();
        assert_eq!(cancelling.status, DaemonJobStatus::Cancelling);
        assert!(cancellation.is_cancelled());
        let error = drain_active_jobs(&state, Duration::from_millis(1))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("timed out draining 1 active"));

        finish_daemon_job(
            &state,
            &job_id,
            DaemonJobStatus::Cancelled,
            None,
            Some("operation cancelled".to_string()),
        )
        .unwrap();

        drain_active_jobs(&state, Duration::from_millis(100))
            .await
            .unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn read_only_requests_continue_while_index_job_is_running() {
        let root = temp_root("read-only-during-index");
        let state = Arc::new(test_daemon_state(&root, false));
        {
            let mut snapshot_cache = state.latest_snapshot_cache.lock().unwrap();
            *snapshot_cache = Some(CanonicalSnapshot {
                snapshot: Some(athanor_domain::SnapshotId("snap_read_only".to_string())),
                entities: vec![athanor_domain::Entity {
                    id: athanor_domain::EntityId("ent_login".to_string()),
                    stable_key: athanor_domain::StableKey("api://POST:/login".to_string()),
                    kind: athanor_domain::EntityKind::ApiEndpoint,
                    name: "POST /login".to_string(),
                    title: Some("Login endpoint".to_string()),
                    source: None,
                    language: None,
                    aliases: vec!["auth login".to_string()],
                    ownership: Vec::new(),
                    payload: serde_json::json!({"summary": "Authenticate a user"}),
                }],
                facts: Vec::new(),
                relations: Vec::new(),
                diagnostics: Vec::new(),
            });
        }
        let index_job =
            start_daemon_job(&state, DaemonJobKind::Index, "running index".to_string()).unwrap();
        assert!(mark_daemon_job_running(&state, &index_job).unwrap());

        let token = Some(state.auth_token.clone());
        let status = execute_request(
            Arc::clone(&state),
            DaemonRequest {
                schema: DAEMON_REQUEST_SCHEMA.to_string(),
                request_id: "req-status".to_string(),
                project_id: "alpha".to_string(),
                auth_token: token.clone(),
                command: DaemonCommand::Status,
            },
        );
        let explain = execute_request(
            Arc::clone(&state),
            DaemonRequest {
                schema: DAEMON_REQUEST_SCHEMA.to_string(),
                request_id: "req-explain".to_string(),
                project_id: "alpha".to_string(),
                auth_token: token.clone(),
                command: DaemonCommand::Explain {
                    stable_key: "api://POST:/login".to_string(),
                },
            },
        );
        let search = execute_request(
            Arc::clone(&state),
            DaemonRequest {
                schema: DAEMON_REQUEST_SCHEMA.to_string(),
                request_id: "req-search".to_string(),
                project_id: "alpha".to_string(),
                auth_token: token,
                command: DaemonCommand::Search {
                    query: "login".to_string(),
                    limit: 10,
                },
            },
        );

        let ((status, _), (explain, _), (search, _)) = tokio::join!(status, explain, search);
        assert!(status.ok);
        assert!(explain.ok);
        assert!(search.ok);
        assert_eq!(
            status
                .result
                .as_ref()
                .and_then(|result| result.get("active_jobs"))
                .and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            explain
                .result
                .as_ref()
                .and_then(|result| result.get("snapshot"))
                .and_then(Value::as_str),
            Some("snap_read_only")
        );
        assert_eq!(
            search
                .result
                .as_ref()
                .and_then(|result| result.get("returned"))
                .and_then(Value::as_u64),
            Some(1)
        );
        let token = Some(state.auth_token.clone());
        let overview = execute_request(
            Arc::clone(&state),
            DaemonRequest {
                schema: DAEMON_REQUEST_SCHEMA.to_string(),
                request_id: "req-overview".to_string(),
                project_id: "alpha".to_string(),
                auth_token: token.clone(),
                command: DaemonCommand::Overview { top: 10 },
            },
        );
        let context = execute_request(
            Arc::clone(&state),
            DaemonRequest {
                schema: DAEMON_REQUEST_SCHEMA.to_string(),
                request_id: "req-context".to_string(),
                project_id: "alpha".to_string(),
                auth_token: token,
                command: DaemonCommand::Context {
                    task: "login".to_string(),
                    diff: false,
                    level: ContextLevel::Normal,
                    limits: ContextLimitOverrides::default(),
                },
            },
        );

        let ((overview, _), (context, _)) = tokio::join!(overview, context);
        assert!(overview.ok);
        assert!(context.ok);
        assert_eq!(
            overview
                .result
                .as_ref()
                .and_then(|result| result.get("schema"))
                .and_then(Value::as_str),
            Some("athanor.overview.v1")
        );
        assert_eq!(
            overview
                .result
                .as_ref()
                .and_then(|result| result.get("snapshot"))
                .and_then(Value::as_str),
            Some("snap_read_only")
        );
        assert_eq!(
            context
                .result
                .as_ref()
                .and_then(|result| result.get("payload"))
                .and_then(|payload| payload.get("schema"))
                .and_then(Value::as_str),
            Some("athanor.context_pack.v1")
        );
        assert_eq!(
            context
                .result
                .as_ref()
                .and_then(|result| result.get("payload"))
                .and_then(|payload| payload.get("snapshot"))
                .and_then(Value::as_str),
            Some("snap_read_only")
        );
        assert!(has_active_job(&state, DaemonJobKind::Index).unwrap());

        let burst = (0..48).map(|index| {
            let command = match index % 5 {
                0 => DaemonCommand::Status,
                1 => DaemonCommand::Explain {
                    stable_key: "api://POST:/login".to_string(),
                },
                2 => DaemonCommand::Search {
                    query: "login".to_string(),
                    limit: 10,
                },
                3 => DaemonCommand::Overview { top: 10 },
                _ => DaemonCommand::Context {
                    task: "login".to_string(),
                    diff: false,
                    level: ContextLevel::Normal,
                    limits: ContextLimitOverrides::default(),
                },
            };
            execute_request(
                Arc::clone(&state),
                DaemonRequest {
                    schema: DAEMON_REQUEST_SCHEMA.to_string(),
                    request_id: format!("req-burst-{index:02}"),
                    project_id: "alpha".to_string(),
                    auth_token: Some(state.auth_token.clone()),
                    command,
                },
            )
        });
        let burst = futures::future::join_all(burst).await;
        assert_eq!(burst.len(), 48);
        assert!(
            burst
                .iter()
                .all(|(response, shutdown)| response.ok && !shutdown)
        );
        assert!(has_active_job(&state, DaemonJobKind::Index).unwrap());

        invalidate_daemon_caches(&state);
        drop(state);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn tracks_daemon_job_start_and_finish() {
        let endpoint = DaemonEndpoint {
            schema: DAEMON_ENDPOINT_SCHEMA.to_string(),
            protocol_version: DAEMON_PROTOCOL_VERSION,
            athanor_version: env!("CARGO_PKG_VERSION").to_string(),
            runtime_id: "runtime-test".to_string(),
            token_path: PathBuf::from("token"),
            project_id: "alpha".to_string(),
            root: PathBuf::from("."),
            registry_path: PathBuf::from("projects.json"),
            address: "127.0.0.1:1".parse().unwrap(),
            transport: DaemonTransport::Tcp,
            local_socket_path: None,
            windows_pipe_name: None,
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
            composition: None,
            auth_token: "test-token".to_string(),
            insecure_allow_v1: false,
            lifecycle: Mutex::new(DaemonLifecycleState::Running),
            last_successful_index: Mutex::new(None),
            endpoint,
            jobs: Mutex::new(Vec::new()),
            next_job_sequence: Mutex::new(1),
            max_job_history: 100,
            latest_snapshot_cache: Mutex::new(None),
            search_index_cache: Mutex::new(None),
            overview_cache: Mutex::new(BoundedCache::new(DERIVED_CACHE_CAPACITY)),
            context_cache: Mutex::new(BoundedCache::new(DERIVED_CACHE_CAPACITY)),
            cancellation_tokens: Mutex::new(HashMap::new()),
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
            protocol_version: DAEMON_PROTOCOL_VERSION,
            athanor_version: env!("CARGO_PKG_VERSION").to_string(),
            runtime_id: "runtime-test".to_string(),
            token_path: PathBuf::from("token"),
            project_id: "alpha".to_string(),
            root: PathBuf::from("."),
            registry_path: PathBuf::from("projects.json"),
            address: "127.0.0.1:1".parse().unwrap(),
            transport: DaemonTransport::Tcp,
            local_socket_path: None,
            windows_pipe_name: None,
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
            composition: None,
            auth_token: "test-token".to_string(),
            insecure_allow_v1: false,
            lifecycle: Mutex::new(DaemonLifecycleState::Running),
            last_successful_index: Mutex::new(None),
            endpoint,
            jobs: Mutex::new(Vec::new()),
            next_job_sequence: Mutex::new(1),
            max_job_history: 100,
            latest_snapshot_cache: Mutex::new(None),
            search_index_cache: Mutex::new(None),
            overview_cache: Mutex::new(BoundedCache::new(DERIVED_CACHE_CAPACITY)),
            context_cache: Mutex::new(BoundedCache::new(DERIVED_CACHE_CAPACITY)),
            cancellation_tokens: Mutex::new(HashMap::new()),
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
            protocol_version: DAEMON_PROTOCOL_VERSION,
            athanor_version: env!("CARGO_PKG_VERSION").to_string(),
            runtime_id: "runtime-test".to_string(),
            token_path: PathBuf::from("token"),
            project_id: "alpha".to_string(),
            root: root.clone(),
            registry_path: root.join("projects.json"),
            address,
            transport: DaemonTransport::Tcp,
            local_socket_path: None,
            windows_pipe_name: None,
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
        let mut state = test_daemon_state(&root, false);
        state.endpoint = endpoint;
        let task = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_busy_connection(stream, &state).await.unwrap();
        });

        let mut stream = TcpStream::connect(address).await.unwrap();
        let request = DaemonRequest {
            schema: DAEMON_REQUEST_SCHEMA.to_string(),
            request_id: "req-busy".to_string(),
            project_id: "alpha".to_string(),
            auth_token: Some("a".repeat(crate::DAEMON_TOKEN_BYTES * 2)),
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

    #[tokio::test]
    async fn busy_response_masks_invalid_authentication() {
        let root = temp_root("busy-auth");
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let mut state = test_daemon_state(&root, false);
        state.endpoint.address = address;
        let task = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_busy_connection(stream, &state).await.unwrap();
        });

        let mut stream = TcpStream::connect(address).await.unwrap();
        let request = DaemonRequest {
            schema: DAEMON_REQUEST_SCHEMA.to_string(),
            request_id: "req-busy-auth".to_string(),
            project_id: "alpha".to_string(),
            auth_token: Some("bad-token".to_string()),
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
        assert_eq!(response.request_id, "req-busy-auth");
        assert_eq!(
            response.error.as_deref(),
            Some("daemon authentication failed")
        );
        task.await.unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn daemon_lock_is_single_instance_and_cleans_up() {
        let root = temp_root("lock");
        let lock_path = root.join("lock");
        let lock = DaemonRuntimeLock::acquire(&lock_path, "alpha").unwrap();
        assert!(DaemonRuntimeLock::acquire(&lock_path, "alpha").is_err());
        drop(lock);
        assert!(DaemonRuntimeLock::acquire(&lock_path, "alpha").is_ok());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn startup_cleanup_removes_only_known_staging_artifacts() {
        let root = temp_root("staging-cleanup");
        let generated = root.join(".athanor/generated");
        fs::create_dir_all(&generated).unwrap();
        fs::create_dir_all(generated.join(".wiki.tmp-1")).unwrap();
        fs::create_dir_all(generated.join(".html.backup-1")).unwrap();
        fs::create_dir_all(generated.join("published-generation")).unwrap();
        fs::write(generated.join("ordinary.tmp-file"), "keep").unwrap();

        cleanup_known_staging_artifacts(&root).unwrap();

        assert!(!generated.join(".wiki.tmp-1").exists());
        assert!(!generated.join(".html.backup-1").exists());
        assert!(generated.join("published-generation").exists());
        assert!(generated.join("ordinary.tmp-file").exists());
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

    #[cfg(unix)]
    #[tokio::test]
    async fn unix_local_socket_endpoint_removes_stale_file_and_uses_owner_only_permissions() {
        let root = short_unix_temp_root("unix-local-socket");
        let socket_path = root.join("daemon.sock");
        fs::write(&socket_path, "stale").unwrap();

        let endpoint = local_socket_endpoint(&root, "runtime-test").unwrap();

        assert_eq!(endpoint.socket_path.as_deref(), Some(socket_path.as_path()));
        assert!(endpoint.pipe_name.is_none());
        assert!(endpoint.guard.is_some());
        assert!(!socket_path.exists());
        let (accepted_tx, _accepted_rx) = mpsc::channel(1);
        spawn_local_socket_acceptor(&endpoint, accepted_tx)
            .await
            .unwrap();
        assert_eq!(
            fs::metadata(&socket_path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    fn short_unix_temp_root(label: &str) -> PathBuf {
        let root = PathBuf::from("/tmp").join(format!(
            "ath-{label}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[cfg(windows)]
    #[test]
    fn windows_local_socket_endpoint_sanitizes_pipe_name() {
        let root = temp_root("windows-local-socket");

        let endpoint = local_socket_endpoint(&root, "runtime:bad/value").unwrap();

        assert!(endpoint.socket_path.is_none());
        assert_eq!(
            endpoint.pipe_name.as_deref(),
            Some(r"\\.\pipe\athanor-runtime_bad_value")
        );
        assert!(endpoint.guard.is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn windows_named_pipe_acceptor_recreates_server_after_disconnect() {
        let root = temp_root("windows-pipe-lifecycle");
        let endpoint =
            local_socket_endpoint(&root, &format!("runtime-lifecycle-{}", std::process::id()))
                .unwrap();
        let pipe_name = endpoint.pipe_name.clone().unwrap();
        let (accepted_tx, mut accepted_rx) = mpsc::channel(2);
        spawn_local_socket_acceptor(&endpoint, accepted_tx)
            .await
            .unwrap();

        for _ in 0..2 {
            let mut client = None;
            let mut last_error = None;
            for _ in 0..200 {
                match PipeClientOptions::new().open(&pipe_name) {
                    Ok(opened) => {
                        client = Some(opened);
                        break;
                    }
                    Err(error) => {
                        last_error = Some(error);
                        tokio::time::sleep(Duration::from_millis(5)).await;
                    }
                }
            }
            let client = client.unwrap_or_else(|| {
                panic!(
                    "failed to open test daemon pipe: {}",
                    last_error
                        .map(|error| error.to_string())
                        .unwrap_or_else(|| "no connection attempt completed".to_string())
                )
            });
            let accepted = tokio::time::timeout(Duration::from_secs(2), accepted_rx.recv())
                .await
                .unwrap()
                .expect("named pipe acceptor stopped");
            assert_eq!(accepted.peer, pipe_name);
            drop(accepted);
            drop(client);
        }

        fs::remove_dir_all(root).unwrap();
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

    async fn request_status_with_retry(root: &Path) -> DaemonResponse {
        let mut last_error = None;
        for _ in 0..50 {
            if root.join(".athanor/daemon/endpoint.json").is_file() {
                match request_daemon(
                    DaemonClientOptions {
                        root: root.to_path_buf(),
                        runtime_dir: Some(root.join(".athanor/daemon")),
                    },
                    DaemonRequest {
                        schema: DAEMON_REQUEST_SCHEMA.to_string(),
                        request_id: "req-status".to_string(),
                        project_id: "alpha".to_string(),
                        auth_token: None,
                        command: DaemonCommand::Status,
                    },
                )
                .await
                {
                    Ok(response) => return response,
                    Err(error) => last_error = Some(error.to_string()),
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        panic!(
            "daemon status request did not succeed: {}",
            last_error.unwrap_or_else(|| "endpoint was not written".to_string())
        );
    }

    fn test_daemon_state(root: &Path, insecure_allow_v1: bool) -> DaemonState {
        DaemonState {
            composition: None,
            auth_token: "a".repeat(crate::DAEMON_TOKEN_BYTES * 2),
            insecure_allow_v1,
            lifecycle: Mutex::new(DaemonLifecycleState::Running),
            last_successful_index: Mutex::new(None),
            endpoint: DaemonEndpoint {
                schema: DAEMON_ENDPOINT_SCHEMA.to_string(),
                protocol_version: DAEMON_PROTOCOL_VERSION,
                athanor_version: env!("CARGO_PKG_VERSION").to_string(),
                runtime_id: "runtime-test".to_string(),
                token_path: root.join("token"),
                project_id: "alpha".to_string(),
                root: root.to_path_buf(),
                registry_path: root.join("projects.json"),
                address: "127.0.0.1:1".parse().unwrap(),
                transport: DaemonTransport::Tcp,
                local_socket_path: None,
                windows_pipe_name: None,
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
            latest_snapshot_cache: Mutex::new(None),
            search_index_cache: Mutex::new(None),
            overview_cache: Mutex::new(BoundedCache::new(DERIVED_CACHE_CAPACITY)),
            context_cache: Mutex::new(BoundedCache::new(DERIVED_CACHE_CAPACITY)),
            cancellation_tokens: Mutex::new(HashMap::new()),
        }
    }
}
