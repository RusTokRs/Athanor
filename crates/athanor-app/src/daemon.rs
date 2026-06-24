use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Semaphore, mpsc};

use athanor_domain::ContextLevel;

use crate::{
    ContextLimitOverrides, ContextOptions, OverviewOptions, context_project, overview_project,
};

pub const DAEMON_ENDPOINT_SCHEMA: &str = "athanor.daemon_endpoint.v1";
pub const DAEMON_REQUEST_SCHEMA: &str = "athanor.daemon_request.v1";
pub const DAEMON_RESPONSE_SCHEMA: &str = "athanor.daemon_response.v1";
const MAX_REQUEST_BYTES: u64 = 1024 * 1024;
const MAX_RESPONSE_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone)]
pub struct DaemonServeOptions {
    pub project_id: String,
    pub root: PathBuf,
    pub registry_path: PathBuf,
    pub listen: SocketAddr,
    pub max_concurrent_requests: usize,
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
    Overview {
        top: usize,
    },
    Context {
        task: String,
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

pub async fn serve_daemon(options: DaemonServeOptions) -> Result<()> {
    if options.max_concurrent_requests == 0 || options.max_concurrent_requests > 128 {
        bail!("daemon max_concurrent_requests must be between 1 and 128");
    }
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
    };
    write_endpoint(&endpoint_path, &endpoint)?;
    let _endpoint_guard = EndpointGuard(endpoint_path.clone());
    let endpoint = Arc::new(endpoint);
    let request_slots = Arc::new(Semaphore::new(options.max_concurrent_requests));
    let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
    tracing::info!(
        project_id = %endpoint.project_id,
        root = %root.display(),
        address = %address,
        "Athanor daemon listening"
    );

    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let (stream, peer) = accepted.context("failed to accept daemon connection")?;
                match request_slots.clone().try_acquire_owned() {
                    Ok(permit) => {
                        let endpoint = Arc::clone(&endpoint);
                        let shutdown_tx = shutdown_tx.clone();
                        tokio::spawn(async move {
                            let _permit = permit;
                            match handle_connection(stream, &endpoint).await {
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
                        let endpoint = Arc::clone(&endpoint);
                        tokio::spawn(async move {
                            if let Err(error) = handle_busy_connection(stream, &endpoint).await {
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
            signal = tokio::signal::ctrl_c() => {
                signal.context("failed to listen for daemon shutdown signal")?;
                break;
            }
        }
    }

    tracing::info!(project_id = %endpoint.project_id, "Athanor daemon stopped");
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
    if request_json.len() as u64 > MAX_REQUEST_BYTES {
        bail!("daemon request exceeds {} bytes", MAX_REQUEST_BYTES);
    }
    stream.write_all(&request_json).await?;
    stream.write_all(b"\n").await?;
    stream.shutdown().await?;

    let mut response = Vec::new();
    stream
        .take(MAX_RESPONSE_BYTES + 1)
        .read_to_end(&mut response)
        .await
        .context("failed to read daemon response")?;
    if response.len() as u64 > MAX_RESPONSE_BYTES {
        bail!("daemon response exceeds {} bytes", MAX_RESPONSE_BYTES);
    }
    if response.is_empty() {
        bail!("daemon returned an empty response");
    }
    serde_json::from_slice(&response).context("failed to parse daemon response")
}

async fn handle_connection(stream: TcpStream, endpoint: &DaemonEndpoint) -> Result<bool> {
    let (read_half, mut write_half) = stream.into_split();
    let mut line = String::new();
    let bytes = BufReader::new(read_half)
        .take(MAX_REQUEST_BYTES + 1)
        .read_line(&mut line)
        .await
        .context("failed to read daemon request")?;
    let (response, shutdown) = if bytes == 0 {
        (
            error_response("", &endpoint.project_id, "empty daemon request"),
            false,
        )
    } else if bytes as u64 > MAX_REQUEST_BYTES {
        (
            error_response(
                "",
                &endpoint.project_id,
                "daemon request exceeds size limit",
            ),
            false,
        )
    } else {
        match serde_json::from_str::<DaemonRequest>(&line) {
            Ok(request) => execute_request(endpoint, request).await,
            Err(error) => (
                error_response(
                    "",
                    &endpoint.project_id,
                    &format!("invalid daemon request JSON: {error}"),
                ),
                false,
            ),
        }
    };
    let response_json = serialize_daemon_response(response)?;
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
        .take(MAX_REQUEST_BYTES + 1)
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
        .write_all(&serialize_daemon_response(response)?)
        .await
        .context("failed to write daemon busy response")?;
    write_half.shutdown().await?;
    Ok(())
}

async fn execute_request(
    endpoint: &DaemonEndpoint,
    request: DaemonRequest,
) -> (DaemonResponse, bool) {
    if let Err(error) = validate_request(&request) {
        return (
            error_response(
                &request.request_id,
                &endpoint.project_id,
                &error.to_string(),
            ),
            false,
        );
    }
    if request.project_id != endpoint.project_id {
        return (
            error_response(
                &request.request_id,
                &endpoint.project_id,
                &format!(
                    "request project `{}` does not match daemon project `{}`",
                    request.project_id, endpoint.project_id
                ),
            ),
            false,
        );
    }

    match request.command {
        DaemonCommand::Status => (
            success_response(
                &request.request_id,
                &endpoint.project_id,
                serde_json::json!({
                    "status": "running",
                    "endpoint": endpoint,
                }),
            ),
            false,
        ),
        DaemonCommand::Overview { top } => {
            if top == 0 || top > 100 {
                return (
                    error_response(
                        &request.request_id,
                        &endpoint.project_id,
                        "overview top must be between 1 and 100",
                    ),
                    false,
                );
            }
            match overview_project(OverviewOptions {
                root: endpoint.root.clone(),
                top,
            })
            .await
            {
                Ok(overview) => (
                    success_response(
                        &request.request_id,
                        &endpoint.project_id,
                        serde_json::to_value(overview).unwrap_or(Value::Null),
                    ),
                    false,
                ),
                Err(error) => (
                    error_response(
                        &request.request_id,
                        &endpoint.project_id,
                        &error.to_string(),
                    ),
                    false,
                ),
            }
        }
        DaemonCommand::Context {
            task,
            level,
            limits,
        } => {
            if task.trim().is_empty() {
                return (
                    error_response(
                        &request.request_id,
                        &endpoint.project_id,
                        "context task must not be empty",
                    ),
                    false,
                );
            }
            match context_project(ContextOptions {
                root: endpoint.root.clone(),
                task,
                diff: false,
                level,
                limits,
            })
            .await
            {
                Ok(pack) => (
                    success_response(
                        &request.request_id,
                        &endpoint.project_id,
                        serde_json::to_value(pack).unwrap_or(Value::Null),
                    ),
                    false,
                ),
                Err(error) => (
                    error_response(
                        &request.request_id,
                        &endpoint.project_id,
                        &error.to_string(),
                    ),
                    false,
                ),
            }
        }
        DaemonCommand::Shutdown => (
            success_response(
                &request.request_id,
                &endpoint.project_id,
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

fn serialize_daemon_response(response: DaemonResponse) -> Result<Vec<u8>> {
    let response_json = serde_json::to_vec(&response)?;
    if response_json.len() as u64 <= MAX_RESPONSE_BYTES {
        return Ok(response_json);
    }

    let overflow = error_response(
        &response.request_id,
        &response.project_id,
        &format!(
            "daemon response exceeds size limit of {} bytes",
            MAX_RESPONSE_BYTES
        ),
    );
    let overflow_json = serde_json::to_vec(&overflow)?;
    if overflow_json.len() as u64 > MAX_RESPONSE_BYTES {
        bail!("daemon overflow error response exceeds response size limit");
    }
    Ok(overflow_json)
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
        };
        let task = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_connection(stream, &endpoint).await.unwrap()
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
                "body": "x".repeat((MAX_RESPONSE_BYTES as usize) + 1),
            }),
        );

        let serialized = serialize_daemon_response(response).unwrap();
        assert!(serialized.len() as u64 <= MAX_RESPONSE_BYTES);
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
