use anyhow::{Context, Result, anyhow};
use tracing_subscriber::{EnvFilter, fmt};

mod analysis_cli;
mod api_cli;
mod direct_check_cli;
mod direct_config_cli;
mod direct_context_cli;
mod direct_generation_cli;
mod direct_graph_cli;
mod direct_operation;
mod direct_plugin_cli;
mod direct_read;
mod direct_rustok_help;
mod direct_search_cli;
mod direct_validate_changed_cli;
mod docs_cli;
mod index_cli;
mod mcp_cli;
mod projects_cli;
mod render;
mod repair;
mod root_command;
mod rustok_cli;

fn main() -> Result<()> {
    std::thread::Builder::new()
        .name("athanor-cli".to_string())
        .stack_size(8 * 1024 * 1024)
        .spawn(run)
        .context("failed to start Athanor CLI thread")?
        .join()
        .map_err(|_| anyhow!("Athanor CLI thread panicked"))?
}

fn run() -> Result<()> {
    init_tracing();
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    match root_command::parse(&args)? {
        root_command::Command::Handled => Ok(()),
        root_command::Command::Plugin(command) => direct_plugin_cli::run(command),
        root_command::Command::ValidateChanged(command) => {
            runtime("Athanor changed validation runtime")?
                .block_on(direct_validate_changed_cli::run(command))
        }
        root_command::Command::Repair(command) => {
            runtime("Athanor repair runtime")?.block_on(repair::run(command))
        }
        root_command::Command::Generation(command) => {
            runtime("Athanor generation runtime")?.block_on(direct_generation_cli::run(command))
        }
        root_command::Command::Config(command) => direct_config_cli::run(command),
        root_command::Command::Check(command) => {
            runtime("Athanor check runtime")?.block_on(direct_check_cli::run(command))
        }
        root_command::Command::Rustok(command) => {
            runtime("Athanor Rustok runtime")?.block_on(rustok_cli::run(command))
        }
        root_command::Command::Graph(command) => {
            runtime("Athanor graph runtime")?.block_on(direct_graph_cli::run(command))
        }
        root_command::Command::Context(command) => {
            runtime("Athanor context runtime")?.block_on(direct_context_cli::run(command))
        }
        root_command::Command::Search(command) => {
            runtime("Athanor search runtime")?.block_on(direct_search_cli::run(command))
        }
        root_command::Command::Read(command) => {
            runtime("Athanor read runtime")?.block_on(direct_read::run(command))
        }
        root_command::Command::Index(command) => {
            runtime("Athanor index runtime")?.block_on(index_cli::run(command))
        }
        root_command::Command::Docs(command) => {
            runtime("Athanor documentation runtime")?.block_on(docs_cli::run(command))
        }
        root_command::Command::Api(command) => {
            runtime("Athanor API contract runtime")?.block_on(api_cli::run(command))
        }
        root_command::Command::Projects(command) => projects_cli::run(command),
        root_command::Command::Analysis(command) => {
            runtime("Athanor analysis runtime")?.block_on(analysis_cli::run(command))
        }
        root_command::Command::Mcp(command) => {
            runtime("Athanor MCP runtime")?.block_on(mcp_cli::run(command))
        }
    }
}

fn runtime(label: &str) -> Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .with_context(|| format!("failed to start {label}"))
}

fn init_tracing() {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = fmt().with_env_filter(filter).with_target(false).try_init();
}
