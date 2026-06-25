use std::fs::{self, OpenOptions};
use std::io;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::{Command as ProcessCommand, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use athanor_app::{
    ContextLimitOverrides, DaemonClientOptions, DaemonCommand, DaemonRequest, DaemonRuntimePaths,
    DaemonServeOptions, DaemonTransport, ProjectRegistryOptions, default_project_registry_path,
    request_daemon, resolve_registered_project, serve_daemon,
};
use athanor_domain::ContextLevel;
use clap::{Parser, Subcommand, ValueEnum};
use tracing_subscriber::{EnvFilter, fmt};

#[cfg(any(target_os = "linux", windows))]
use anyhow::Context;

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
        /// Temporarily allow unauthenticated daemon protocol v1 requests on loopback.
        #[arg(long)]
        insecure_allow_v1: bool,
        /// Maximum time to drain active jobs during shutdown.
        #[arg(long, default_value_t = 30)]
        shutdown_timeout_seconds: u64,
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
        /// Temporarily allow unauthenticated daemon protocol v1 requests on loopback.
        #[arg(long)]
        insecure_allow_v1: bool,
        /// Maximum time to drain active jobs during shutdown.
        #[arg(long, default_value_t = 30)]
        shutdown_timeout_seconds: u64,
        /// Write structured daemon logs to this file.
        #[arg(long, hide = true)]
        log_file: Option<PathBuf>,
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
    /// Inspect production daemon configuration and runtime health.
    Doctor {
        project_id: String,
        #[arg(long)]
        registry: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Manage per-user daemon autostart.
    Service {
        #[command(subcommand)]
        command: ServiceCommand,
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

#[derive(Debug, Subcommand)]
enum ServiceCommand {
    /// Install or update per-user daemon autostart.
    Install {
        project_id: String,
        #[arg(long)]
        registry: Option<PathBuf>,
        #[arg(long, value_enum, default_value_t = TransportArg::LocalSocket)]
        transport: TransportArg,
        #[arg(long)]
        watch: bool,
        #[arg(long)]
        json: bool,
    },
    /// Remove per-user daemon autostart.
    Uninstall {
        project_id: String,
        #[arg(long)]
        json: bool,
    },
    /// Inspect per-user daemon autostart state.
    Status {
        project_id: String,
        #[arg(long)]
        json: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    init_tracing(command_log_file(&cli.command))?;
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
            insecure_allow_v1,
            shutdown_timeout_seconds,
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
                insecure_allow_v1,
                shutdown_timeout_seconds,
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
            insecure_allow_v1,
            shutdown_timeout_seconds,
            log_file: _,
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
                insecure_allow_v1,
                runtime_dir: None,
                shutdown_timeout: Duration::from_secs(shutdown_timeout_seconds),
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
        Command::Doctor {
            project_id,
            registry,
            json,
        } => print_value(doctor(&project_id, registry).await?, json),
        Command::Service { command } => match command {
            ServiceCommand::Install {
                project_id,
                registry,
                transport,
                watch,
                json,
            } => print_value(
                install_service(&project_id, registry, transport, watch)?,
                json,
            ),
            ServiceCommand::Uninstall { project_id, json } => {
                print_value(uninstall_service(&project_id)?, json)
            }
            ServiceCommand::Status { project_id, json } => {
                print_value(service_status(&project_id)?, json)
            }
        },
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
    insecure_allow_v1: bool,
    shutdown_timeout_seconds: u64,
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

    let runtime = DaemonRuntimePaths::for_project(&project_id, None)?;
    runtime.prepare()?;
    rotate_logs(&runtime.log)?;
    let log_path = runtime.log;
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
        .arg("--shutdown-timeout-seconds")
        .arg(shutdown_timeout_seconds.to_string())
        .arg("--log-file")
        .arg(&log_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if watch {
        command.arg("--watch");
    }
    if watch_poll {
        command.arg("--watch-poll");
    }
    if insecure_allow_v1 {
        command.arg("--insecure-allow-v1");
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
            runtime_dir: None,
        },
        DaemonRequest {
            schema: athanor_app::DAEMON_REQUEST_SCHEMA.to_string(),
            request_id: format!("start-{}", std::process::id()),
            project_id: project_id.to_string(),
            auth_token: None,
            command: DaemonCommand::Status,
        },
    )
    .await
}

async fn doctor(project_id: &str, registry: Option<PathBuf>) -> Result<serde_json::Value> {
    let registry_path = registry_path(registry)?;
    let resolution = resolve_registered_project(
        ProjectRegistryOptions {
            registry_path: registry_path.clone(),
        },
        project_id,
    )?;
    let runtime = DaemonRuntimePaths::for_project(project_id, None)?;
    let endpoint_exists = runtime.endpoint.is_file();
    let token_exists = runtime.token.is_file();
    let lock_exists = runtime.lock.is_file();
    let handshake = request_status(project_id, &resolution.project.root).await;
    let (healthy, response, error) = match handshake {
        Ok(response) if response.ok => (true, serde_json::to_value(response).ok(), None::<String>),
        Ok(response) => (false, serde_json::to_value(response).ok(), None),
        Err(error) => (false, None, Some(error.to_string())),
    };
    Ok(serde_json::json!({
        "schema": "athanor.daemon_doctor.v1",
        "project_id": project_id,
        "root": resolution.project.root,
        "registry_path": registry_path,
        "runtime_directory": runtime.directory,
        "endpoint_exists": endpoint_exists,
        "token_exists": token_exists,
        "lock_metadata_exists": lock_exists,
        "healthy": healthy,
        "response": response,
        "error": error,
    }))
}

fn install_service(
    project_id: &str,
    registry: Option<PathBuf>,
    transport: TransportArg,
    watch: bool,
) -> Result<serde_json::Value> {
    let registry_path = registry_path(registry)?;
    resolve_registered_project(
        ProjectRegistryOptions {
            registry_path: registry_path.clone(),
        },
        project_id,
    )?;
    let runtime = DaemonRuntimePaths::for_project(project_id, None)?;
    runtime.prepare()?;
    rotate_logs(&runtime.log)?;
    #[cfg(any(target_os = "linux", windows))]
    let executable = std::env::current_exe()?;
    #[cfg(any(target_os = "linux", windows))]
    let transport = match transport {
        TransportArg::Tcp => "tcp",
        TransportArg::LocalSocket => "local-socket",
    };

    #[cfg(target_os = "linux")]
    {
        let unit_dir = user_home()?.join(".config/systemd/user");
        fs::create_dir_all(&unit_dir)?;
        let unit_name = service_name(project_id);
        let unit_path = unit_dir.join(format!("{unit_name}.service"));
        let watch_arg = if watch { " --watch" } else { "" };
        let unit = format!(
            "[Unit]\nDescription=Athanor daemon for {project_id}\nAfter=default.target\n\n[Service]\nType=simple\nExecStart={} serve {} --registry {} --transport {}{} --shutdown-timeout-seconds 30 --log-file {}\nRestart=on-failure\nRestartSec=2\nTimeoutStopSec=35\n\n[Install]\nWantedBy=default.target\n",
            systemd_escape(&executable),
            shell_arg(project_id),
            systemd_escape(&registry_path),
            transport,
            watch_arg,
            systemd_escape(&runtime.log),
        );
        fs::write(&unit_path, unit)?;
        run_checked("systemctl", &["--user", "daemon-reload"])?;
        run_checked(
            "systemctl",
            &["--user", "enable", "--now", &format!("{unit_name}.service")],
        )?;
        Ok(serde_json::json!({
            "schema": "athanor.daemon_service.v1",
            "platform": "linux",
            "project_id": project_id,
            "installed": true,
            "unit": unit_path,
        }))
    }

    #[cfg(windows)]
    {
        let task_name = service_name(project_id);
        let xml_path = runtime.directory.join("service-task.xml");
        let watch_arg = if watch { " --watch" } else { "" };
        let arguments = format!(
            "serve {} --registry \"{}\" --transport {}{} --shutdown-timeout-seconds 30 --log-file \"{}\"",
            project_id,
            registry_path.display(),
            transport,
            watch_arg,
            runtime.log.display(),
        );
        let user = windows_user_identity()?;
        let xml = windows_task_xml(&task_name, &user, &executable, &arguments);
        write_windows_task_xml(&xml_path, &xml)?;
        let xml_arg = xml_path.to_string_lossy().into_owned();
        run_checked(
            "schtasks",
            &["/Create", "/TN", &task_name, "/XML", &xml_arg, "/F"],
        )?;
        run_checked("schtasks", &["/Run", "/TN", &task_name])?;
        Ok(serde_json::json!({
            "schema": "athanor.daemon_service.v1",
            "platform": "windows",
            "project_id": project_id,
            "installed": true,
            "task": task_name,
            "definition": xml_path,
        }))
    }

    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let _ = (transport, watch);
        bail!("daemon service installation is supported only on Windows and Linux")
    }
}

fn uninstall_service(project_id: &str) -> Result<serde_json::Value> {
    #[cfg(target_os = "linux")]
    {
        let unit_name = service_name(project_id);
        let unit_path = user_home()?
            .join(".config/systemd/user")
            .join(format!("{unit_name}.service"));
        let _ = ProcessCommand::new("systemctl")
            .args([
                "--user",
                "disable",
                "--now",
                &format!("{unit_name}.service"),
            ])
            .status();
        if unit_path.exists() {
            fs::remove_file(&unit_path)?;
        }
        run_checked("systemctl", &["--user", "daemon-reload"])?;
        Ok(serde_json::json!({
            "schema": "athanor.daemon_service.v1",
            "platform": "linux",
            "project_id": project_id,
            "installed": false,
        }))
    }

    #[cfg(windows)]
    {
        let task_name = service_name(project_id);
        let status = ProcessCommand::new("schtasks")
            .args(["/Delete", "/TN", &task_name, "/F"])
            .status()?;
        if !status.success() {
            tracing::warn!(task = %task_name, "Task Scheduler task was not installed");
        }
        if let Ok(runtime) = DaemonRuntimePaths::for_project(project_id, None) {
            let definition = runtime.directory.join("service-task.xml");
            if definition.exists() {
                fs::remove_file(definition)?;
            }
        }
        Ok(serde_json::json!({
            "schema": "athanor.daemon_service.v1",
            "platform": "windows",
            "project_id": project_id,
            "installed": false,
        }))
    }

    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let _ = project_id;
        bail!("daemon service installation is supported only on Windows and Linux")
    }
}

fn service_status(project_id: &str) -> Result<serde_json::Value> {
    #[cfg(target_os = "linux")]
    {
        let unit_name = format!("{}.service", service_name(project_id));
        let output = ProcessCommand::new("systemctl")
            .args(["--user", "is-active", &unit_name])
            .output()?;
        let state = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(serde_json::json!({
            "schema": "athanor.daemon_service.v1",
            "platform": "linux",
            "project_id": project_id,
            "installed": user_home()?.join(".config/systemd/user").join(&unit_name).is_file(),
            "state": if state.is_empty() { "inactive" } else { &state },
        }))
    }

    #[cfg(windows)]
    {
        let task_name = service_name(project_id);
        let output = ProcessCommand::new("schtasks")
            .args(["/Query", "/TN", &task_name, "/FO", "LIST"])
            .output()?;
        Ok(serde_json::json!({
            "schema": "athanor.daemon_service.v1",
            "platform": "windows",
            "project_id": project_id,
            "installed": output.status.success(),
            "details": String::from_utf8_lossy(&output.stdout).trim(),
        }))
    }

    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let _ = project_id;
        bail!("daemon service installation is supported only on Windows and Linux")
    }
}

#[cfg(any(target_os = "linux", windows))]
fn service_name(project_id: &str) -> String {
    format!("athanor-{project_id}")
}

fn rotate_logs(path: &std::path::Path) -> Result<()> {
    const MAX_LOG_BYTES: u64 = 10 * 1024 * 1024;
    const KEEP_LOGS: usize = 5;
    if !path.is_file() || fs::metadata(path)?.len() < MAX_LOG_BYTES {
        return Ok(());
    }
    for index in (1..KEEP_LOGS).rev() {
        let source = path.with_extension(format!("log.{index}"));
        let target = path.with_extension(format!("log.{}", index + 1));
        if source.exists() {
            if target.exists() {
                fs::remove_file(&target)?;
            }
            fs::rename(source, target)?;
        }
    }
    let first = path.with_extension("log.1");
    if first.exists() {
        fs::remove_file(&first)?;
    }
    fs::rename(path, first)?;
    Ok(())
}

#[cfg(any(target_os = "linux", windows))]
fn run_checked(program: &str, args: &[&str]) -> Result<()> {
    let status = ProcessCommand::new(program)
        .args(args)
        .status()
        .with_context(|| format!("failed to run {program}"))?;
    if !status.success() {
        bail!("{program} exited with {status}");
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn user_home() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("HOME is required for systemd user service installation"))
}

#[cfg(target_os = "linux")]
fn systemd_escape(path: &std::path::Path) -> String {
    shell_arg(&path.to_string_lossy())
}

#[cfg(target_os = "linux")]
fn shell_arg(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(windows)]
fn windows_task_xml(
    task_name: &str,
    user: &str,
    executable: &std::path::Path,
    arguments: &str,
) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-16\"?>\n<Task version=\"1.4\" xmlns=\"http://schemas.microsoft.com/windows/2004/02/mit/task\"><RegistrationInfo><Author>{}</Author><Description>{}</Description></RegistrationInfo><Triggers><LogonTrigger><Enabled>true</Enabled><UserId>{}</UserId></LogonTrigger></Triggers><Principals><Principal id=\"Author\"><UserId>{}</UserId><LogonType>InteractiveToken</LogonType><RunLevel>LeastPrivilege</RunLevel></Principal></Principals><Settings><MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy><DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries><StopIfGoingOnBatteries>false</StopIfGoingOnBatteries><ExecutionTimeLimit>PT0S</ExecutionTimeLimit><RestartOnFailure><Interval>PT1M</Interval><Count>10</Count></RestartOnFailure></Settings><Actions Context=\"Author\"><Exec><Command>{}</Command><Arguments>{}</Arguments></Exec></Actions></Task>",
        xml_escape(user),
        xml_escape(task_name),
        xml_escape(user),
        xml_escape(user),
        xml_escape(&executable.to_string_lossy()),
        xml_escape(arguments),
    )
}

#[cfg(windows)]
fn windows_user_identity() -> Result<String> {
    let user = std::env::var("USERNAME").context("USERNAME is required")?;
    Ok(match std::env::var("USERDOMAIN") {
        Ok(domain) if !domain.is_empty() => format!("{domain}\\{user}"),
        _ => user,
    })
}

#[cfg(windows)]
fn write_windows_task_xml(path: &std::path::Path, xml: &str) -> Result<()> {
    use std::io::Write;

    let mut file = fs::File::create(path)?;
    file.write_all(&[0xff, 0xfe])?;
    for unit in xml.encode_utf16() {
        file.write_all(&unit.to_le_bytes())?;
    }
    file.flush()?;
    Ok(())
}

#[cfg(windows)]
fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
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
            runtime_dir: None,
        },
        DaemonRequest {
            schema: athanor_app::DAEMON_REQUEST_SCHEMA.to_string(),
            request_id: format!("cli-{}", std::process::id()),
            project_id: project_id.to_string(),
            auth_token: None,
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

fn print_value(value: serde_json::Value, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        println!("{}", serde_json::to_string(&value)?);
    }
    Ok(())
}

fn command_log_file(command: &Command) -> Option<&std::path::Path> {
    match command {
        Command::Serve {
            log_file: Some(path),
            ..
        } => Some(path),
        _ => None,
    }
}

fn init_tracing(log_file: Option<&std::path::Path>) -> Result<()> {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("athanor_app=info"));
    if let Some(path) = log_file {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        rotate_logs(path)?;
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        fmt()
            .json()
            .with_env_filter(filter)
            .with_writer(Mutex::new(file))
            .init();
    } else if std::env::var_os("ATHANOR_LOG_FORMAT").as_deref()
        == Some(std::ffi::OsStr::new("json"))
    {
        fmt()
            .json()
            .with_env_filter(filter)
            .with_writer(io::stderr)
            .init();
    } else {
        fmt().with_env_filter(filter).with_writer(io::stderr).init();
    }
    Ok(())
}
