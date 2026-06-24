use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::Result;
use athanor_app::{
    DaemonClientOptions, DaemonCommand, DaemonRequest, DaemonServeOptions, ProjectRegistryOptions,
    default_project_registry_path, request_daemon, resolve_registered_project, serve_daemon,
};
use clap::{Parser, Subcommand};
use tracing_subscriber::{EnvFilter, fmt};

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
    },
    /// Query daemon status.
    Status {
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
            })
            .await
        }
        Command::Status {
            project_id,
            registry,
            json,
        } => {
            print_response(
                request(&project_id, registry, DaemonCommand::Status).await?,
                json,
            )
        }
        Command::Overview {
            project_id,
            registry,
            top,
            json,
        } => {
            print_response(
                request(&project_id, registry, DaemonCommand::Overview { top }).await?,
                json,
            )
        }
        Command::Stop {
            project_id,
            registry,
            json,
        } => {
            print_response(
                request(&project_id, registry, DaemonCommand::Shutdown).await?,
                json,
            )
        }
    }
}

async fn request(
    project_id: &str,
    registry: Option<PathBuf>,
    command: DaemonCommand,
) -> Result<athanor_app::DaemonResponse> {
    let registry_path = registry_path(registry)?;
    let resolution = resolve_registered_project(
        ProjectRegistryOptions { registry_path },
        project_id,
    )?;
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
    fmt().with_env_filter(filter).with_writer(std::io::stderr).init();
}
