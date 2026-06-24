use std::fs::{self, OpenOptions};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::{Command as ProcessCommand, Stdio};
use std::time::{Duration, Instant};

use anyhow::Result;
use athanor_app::{
    ContextLimitOverrides, DaemonClientOptions, DaemonCommand, DaemonRequest, DaemonServeOptions,
    DaemonTransport, ProjectRegistryOptions, default_project_registry_path, request_daemon,
    resolve_registered_project, serve_daemon,
};
use athanor_domain::ContextLevel;
use clap::{Parser, Subcommand, ValueEnum};
use tracing_subscriber::{EnvFilter, fmt};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ContextLevelArg {
    Summary,
    Normal,
    Deep,
    Full,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum TransportArg {
    Tcp,
    LocalSocket,
}

impl From<TransportArg> for DaemonTransport {
    fn from(value: TransportArg) -> Self {
        match value {
            TransportArg::Tcp => Self::Tcp,
            TransportArg::LocalSocket => Self::LocalSocket,
        }
    }
}

impl From<ContextLevelArg> for ContextLevel {
    fn from(value: ContextLevelArg) -> Self {
        match value {
            ContextLevelArg::Summary => Self::Summary,
            ContextLevelArg::Normal => Self::Normal,
            ContextLevelArg::Deep => Self::Deep,
            ContextLevelArg::Full => Self::Full,
        }
    }
}

#[derive(Debug, Parser)]
#[command(name = "athd", version, about = "Athanor local daemon")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Start one explicitly registered project daemon in the background.
    Start {
        /// Exact registered project id.
        project_id: String,
        /// Override the user-level project registry path.
        #[arg(long)]
        registry: Option<PathBuf>,
        /// Local TCP address. Port 0 selects an available port.
        #[arg(long, default_value = "127.0.0.1:0")]
        listen: SocketAddr,
        /// Daemon transport. local-socket uses a Unix domain socket on Unix and a named pipe on Windows.
        #[arg(long, value_enum, default_value_t = TransportArg::Tcp)]
        transport: TransportArg,
        /// Maximum concurrent daemon requests before busy responses are returned.
        #[arg(long, default_value_t = 4)]
        max_concurrent_requests: usize,
        /// Maximum in-memory daemon job records to retain.
        #[arg(long, default_value_t = 1000)]
        max_job_history: usize,
        /// Maximum daemon request size in bytes.
        #[arg(long, default_value_t = 1024 * 1024)]
        max_request_bytes: u64,
        /// Maximum daemon response size in bytes.
        #[arg(long, default_value_t = 1024 * 1024)]
        max_response_bytes: u64,
        /// Watch project files and schedule debounced background index jobs.
        #[arg(long)]
        watch: bool,
        /// Use polling watcher backend instead of the platform-recommended backend.
        #[arg(long)]
        watch_poll: bool,
        /// Debounce window for --watch, in milliseconds.
        #[arg(long, default_value_t = 1000)]
        debounce_ms: u64,
        /// Maximum time to wait for the background daemon to answer status.
        #[arg(long, default_value_t = 10_000)]
        startup_timeout_ms: u64,
        #[arg(long)]
        json: bool,
    },
    /// Serve one explicitly registered project in the foreground.
    Serve {
        /// Exact registered project id.
        project_id: String,
        /// Override the user-level project registry path.
        #[arg(long)]
        registry: Option<PathBuf>,
        /// Local TCP address. Port 0 selects an available port.
        #[arg(long, default_value = "127.0.0.1:0")]
        listen: SocketAddr,
        /// Daemon transport. local-socket uses a Unix domain socket on Unix and a named pipe on Windows.
        #[arg(long, value_enum, default_value_t = TransportArg::Tcp)]
        transport: TransportArg,
        /// Maximum concurrent daemon requests before busy responses are returned.
        #[arg(long, default_value_t = 4)]
        max_concurrent_requests: usize,
        /// Maximum in-memory daemon job records to retain.
        #[arg(long, default_value_t = 1000)]
        max_job_history: usize,
        /// Maximum daemon request size in bytes.
        #[arg(long, default_value_t = 1024 * 1024)]
        max_request_bytes: u64,
        /// Maximum daemon response size in bytes.
        #[arg(long, default_value_t = 1024 * 1024)]
        max_response_bytes: u64,
        /// Watch project files and schedule debounced background index jobs.
        #[arg(long)]
        watch: bool,
        /// Use polling watcher backend instead of the platform-recommended backend.
        #[arg(long)]
        watch_poll: bool,
        /// Debounce window for --watch, in milliseconds.
        #[arg(long, default_value_t = 1000)]
        debounce_ms: u64,
    },
    /// Query daemon status.
    Status {
        project_id: String,
        #[arg(long)]
        registry: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Check daemon protocol health.
    Ping {
        project_id: String,
        #[arg(long)]
        registry: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// List daemon jobs.
    Jobs {
        project_id: String,
        #[arg(long)]
        registry: Option<PathBuf>,
        #[arg(long, default_value_t = 20)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    /// Inspect one daemon job by id.
    Job {
        project_id: String,
        job_id: String,
        #[arg(long)]
        registry: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Request cancellation for a daemon job.
    Cancel {
        project_id: String,
        job_id: String,
        #[arg(long)]
        registry: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Start a background index job for the project.
    Index {
        project_id: String,
        #[arg(long)]
        registry: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Start a background coordinated read-model generation job for the project.
    Generate {
        project_id: String,
        #[arg(long)]
        registry: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Start a background Markdown wiki projection job for the project.
    Wiki {
        project_id: String,
        #[arg(long)]
        registry: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Start a background HTML report projection job for the project.
    ReportHtml {
        project_id: String,
        #[arg(long)]
        registry: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Request a bounded repository overview from the daemon.
    Overview {
        project_id: String,
        #[arg(long)]
        registry: Option<PathBuf>,
        #[arg(long, default_value_t = 10)]
        top: usize,
        #[arg(long)]
        json: bool,
    },
    /// Explain one canonical entity through the daemon's hot snapshot cache.
    Explain {
        project_id: String,
        /// Exact canonical stable key, for example api://POST:/login.
        stable_key: String,
        #[arg(long)]
        registry: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Search canonical entities through the daemon's hot snapshot cache.
    Search {
        project_id: String,
        /// Search query terms.
        query: String,
        #[arg(long)]
        registry: Option<PathBuf>,
        /// Maximum number of search results to return.
        #[arg(long, default_value_t = 10)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    /// Request a bounded task-focused context pack from the daemon.
    Context {
        project_id: String,
        /// Task or question used to select relevant canonical context. Optional with --diff.
        task: Option<String>,
        #[arg(long)]
        registry: Option<PathBuf>,
        /// Select context from changed or removed files without committing a new index.
        #[arg(long)]
        diff: bool,
        /// Context detail level and its default limits.
        #[arg(long, value_enum, default_value_t = ContextLevelArg::Normal)]
        level: ContextLevelArg,
        /// Approximate maximum serialized tokens.
        #[arg(long = "budget")]
        max_tokens: Option<usize>,
        /// Maximum number of source files.
        #[arg(long)]
        max_files: Option<usize>,
        /// Maximum number of canonical entities.
        #[arg(long)]
        max_entities: Option<usize>,
        /// Maximum number of diagnostics.
        #[arg(long)]
        max_diagnostics: Option<usize>,
        /// Maximum relation traversal depth.
        #[arg(long)]
        max_depth: Option<usize>,
        #[arg(long)]
        json: bool,
    },
    /// Ask the daemon to stop cleanly.
    Stop {
        project_id: String,
        #[arg(long)]
        registry: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    let cli = Cli::parse();
    match cli.command {
        Command::Start {
            project_id,
            registry,
            listen,
            transport,
            max_concurrent_requests,
            max_job_history,
            max_request_bytes,
            max_response_bytes,
            watch,
            watch_poll,
            debounce_ms,
            startup_timeout_ms,
            json,
        } => print_response(
            start_background_daemon(
                project_id,
                registry,
                listen,
                transport,
                max_concurrent_requests,
                max_job_history,
                max_request_bytes,
                max_response_bytes,
                watch,
                watch_poll,
                debounce_ms,
                startup_timeout_ms,
            )
            .await?,
            json,
        ),
        Command::Serve {
            project_id,
            registry,
            listen,
            transport,
            max_concurrent_requests,
            max_job_history,
            max_request_bytes,
            max_response_bytes,
            watch,
            watch_poll,
            debounce_ms,
        } => {
            let registry_path = registry_path(registry)?;
            let resolution = resolve_registered_project(
                ProjectRegistryOptions {
                    registry_path: registry_path.clone(),
                },
                &project_id,
            )?;
            serve_daemon(DaemonServeOptions {
                project_id,
                root: resolution.project.root,
                registry_path,
                listen,
                transport: transport.into(),
                max_concurrent_requests,
                max_job_history,
                max_request_bytes,
                max_response_bytes,
                watch,
                watch_poll,
                debounce_ms,
            })
            .await
        }
        Command::Status {
            project_id,
            registry,
            json,
        } => print_response(
            request(&project_id, registry, DaemonCommand::Status).await?,
            json,
        ),
        Command::Ping {
            project_id,
            registry,
            json,
        } => print_response(
            request(&project_id, registry, DaemonCommand::Status).await?,
            json,
        ),
        Command::Jobs {
            project_id,
            registry,
            limit,
            json,
        } => print_response(
            request(&project_id, registry, DaemonCommand::Jobs { limit }).await?,
            json,
        ),
        Command::Job {
            project_id,
            job_id,
            registry,
            json,
        } => print_response(
            request(&project_id, registry, DaemonCommand::Job { job_id }).await?,
            json,
        ),
        Command::Cancel {
            project_id,
            job_id,
            registry,
            json,
        } => print_response(
            request(&project_id, registry, DaemonCommand::Cancel { job_id }).await?,
            json,
        ),
        Command::Index {
            project_id,
            registry,
            json,
        } => print_response(
            request(&project_id, registry, DaemonCommand::Index).await?,
            json,
        ),
        Command::Generate {
            project_id,
            registry,
            json,
        } => print_response(
            request(&project_id, registry, DaemonCommand::Generate).await?,
            json,
        ),
        Command::Wiki {
            project_id,
            registry,
            json,
        } => print_response(
            request(&project_id, registry, DaemonCommand::Wiki).await?,
            json,
        ),
        Command::ReportHtml {
            project_id,
            registry,
            json,
        } => print_response(
            request(&project_id, registry, DaemonCommand::HtmlReport).await?,
            json,
        ),
        Command::Overview {
            project_id,
            registry,
            top,
            json,
        } => print_response(
            request(&project_id, registry, DaemonCommand::Overview { top }).await?,
            json,
        ),
        Command::Explain {
            project_id,
            stable_key,
            registry,
            json,
        } => print_response(
            request(&project_id, registry, DaemonCommand::Explain { stable_key }).await?,
            json,
        ),
        Command::Search {
            project_id,
            query,
            registry,
            limit,
            json,
        } => print_response(
            request(
                &project_id,
                registry,
                DaemonCommand::Search { query, limit },
            )
            .await?,
            json,
        ),
        Command::Context {
            project_id,
            task,
            registry,
            diff,
            level,
            max_tokens,
            max_files,
            max_entities,
            max_diagnostics,
            max_depth,
            json,
        } => print_response(
            request(
                &project_id,
                registry,
                DaemonCommand::Context {
                    task: task.unwrap_or_default(),
                    diff,
                    level: level.into(),
                    limits: ContextLimitOverrides {
                        max_tokens,
                        max_files,
                        max_entities,
                        max_diagnostics,
                        max_depth,
                    },
                },
            )
            .await?,
            json,
        ),
        Command::Stop {
            project_id,
            registry,
            json,
        } => print_response(
            request(&project_id, registry, DaemonCommand::Shutdown).await?,
            json,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
async fn start_background_daemon(
    project_id: String,
    registry: Option<PathBuf>,
    listen: SocketAddr,
    transport: TransportArg,
    max_concurrent_requests: usize,
    max_job_history: usize,
    max_request_bytes: u64,
    max_response_bytes: u64,
    watch: bool,
    watch_poll: bool,
    debounce_ms: u64,
    startup_timeout_ms: u64,
) -> Result<athanor_app::DaemonResponse> {
    if !(100..=60_000).contains(&startup_timeout_ms) {
        anyhow::bail!("startup_timeout_ms must be between 100 and 60000");
    }
    let registry_path = registry_path(registry)?;
    let resolution = resolve_registered_project(
        ProjectRegistryOptions {
            registry_path: registry_path.clone(),
        },
        &project_id,
    )?;
    let root = resolution.project.root;
    if let Ok(response) = request_status(&project_id, &root).await
        && response.ok
    {
        return Ok(response);
    }

    let runtime_dir = root.join(".athanor/daemon");
    fs::create_dir_all(&runtime_dir)?;
    let log_path = runtime_dir.join("daemon.log");
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    let mut command = ProcessCommand::new(std::env::current_exe()?);
    command
        .arg("serve")
        .arg(&project_id)
        .arg("--registry")
        .arg(&registry_path)
        .arg("--listen")
        .arg(listen.to_string())
        .arg("--transport")
        .arg(match transport {
            TransportArg::Tcp => "tcp",
            TransportArg::LocalSocket => "local-socket",
        })
        .arg("--max-concurrent-requests")
        .arg(max_concurrent_requests.to_string())
        .arg("--max-job-history")
        .arg(max_job_history.to_string())
        .arg("--max-request-bytes")
        .arg(max_request_bytes.to_string())
        .arg("--max-response-bytes")
        .arg(max_response_bytes.to_string())
        .arg("--debounce-ms")
        .arg(debounce_ms.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(log));
    if watch {
        command.arg("--watch");
    }
    if watch_poll {
        command.arg("--watch-poll");
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let mut child = command.spawn()?;
    let deadline = Instant::now() + Duration::from_millis(startup_timeout_ms);
    let mut last_error = None;
    while Instant::now() < deadline {
        if let Some(status) = child.try_wait()? {
            anyhow::bail!(
                "background daemon exited with {status}; inspect {}",
                log_path.display()
            );
        }
        match request_status(&project_id, &root).await {
            Ok(response) if response.ok => return Ok(response),
            Ok(response) => last_error = response.error,
            Err(error) => last_error = Some(error.to_string()),
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let _ = child.kill();
    let _ = child.wait();
    anyhow::bail!(
        "background daemon did not become ready within {startup_timeout_ms} ms: {}; inspect {}",
        last_error.unwrap_or_else(|| "no status response".to_string()),
        log_path.display()
    )
}

async fn request_status(
    project_id: &str,
    root: &std::path::Path,
) -> Result<athanor_app::DaemonResponse> {
    request_daemon(
        DaemonClientOptions {
            root: root.to_path_buf(),
        },
        DaemonRequest {
            schema: athanor_app::DAEMON_REQUEST_SCHEMA.to_string(),
            request_id: format!("start-{}", std::process::id()),
            project_id: project_id.to_string(),
            command: DaemonCommand::Status,
        },
    )
    .await
}

async fn request(
    project_id: &str,
    registry: Option<PathBuf>,
    command: DaemonCommand,
) -> Result<athanor_app::DaemonResponse> {
    let registry_path = registry_path(registry)?;
    let resolution =
        resolve_registered_project(ProjectRegistryOptions { registry_path }, project_id)?;
    request_daemon(
        DaemonClientOptions {
            root: resolution.project.root,
        },
        DaemonRequest {
            schema: athanor_app::DAEMON_REQUEST_SCHEMA.to_string(),
            request_id: format!("cli-{}", std::process::id()),
            project_id: project_id.to_string(),
            command,
        },
    )
    .await
}

fn registry_path(path: Option<PathBuf>) -> Result<PathBuf> {
    path.map_or_else(default_project_registry_path, Ok)
}

fn print_response(response: athanor_app::DaemonResponse, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else if response.ok {
        println!(
            "{}: {}",
            response.project_id,
            response
                .result
                .as_ref()
                .map_or_else(|| "ok".to_string(), serde_json::Value::to_string)
        );
    } else {
        println!(
            "{}: error: {}",
            response.project_id,
            response.error.as_deref().unwrap_or("unknown daemon error")
        );
    }
    Ok(())
}

fn init_tracing() {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("athanor_app=info"));
    fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
}
