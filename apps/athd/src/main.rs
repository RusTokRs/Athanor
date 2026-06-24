use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::Result;
use athanor_app::{
    ContextLimitOverrides, DaemonClientOptions, DaemonCommand, DaemonRequest, DaemonServeOptions,
    ProjectRegistryOptions, default_project_registry_path, request_daemon,
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
        /// Maximum concurrent daemon requests before busy responses are returned.
        #[arg(long, default_value_t = 4)]
        max_concurrent_requests: usize,
        /// Maximum in-memory daemon job records to retain.
        #[arg(long, default_value_t = 1000)]
        max_job_history: usize,
    },
    /// Query daemon status.
    Status {
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
        Command::Serve {
            project_id,
            registry,
            listen,
            max_concurrent_requests,
            max_job_history,
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
                max_concurrent_requests,
                max_job_history,
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
