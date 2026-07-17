use anyhow::{Context, Result};

mod direct_application_report_cli;
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
mod render;
mod repair;
mod root_command;
mod rustok_cli;

mod legacy {
    include!("main.rs");

    pub(crate) fn run() -> anyhow::Result<()> {
        main()
    }
}

fn main() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    match root_command::parse(&args)? {
        root_command::Command::Handled => Ok(()),
        root_command::Command::Plugin(command) => direct_plugin_cli::run(command),
        root_command::Command::ValidateChanged(command) => {
            runtime("Athanor direct changed validation runtime")?
                .block_on(direct_validate_changed_cli::run(command))
        }
        root_command::Command::Repair(command) => {
            runtime("Athanor repair runtime")?.block_on(repair::run(command))
        }
        root_command::Command::ApplicationReport(command) => {
            runtime("Athanor versioned application report runtime")?
                .block_on(direct_application_report_cli::run(command))
        }
        root_command::Command::Generation(command) => {
            runtime("Athanor direct generation report runtime")?
                .block_on(direct_generation_cli::run(command))
        }
        root_command::Command::Config(command) => direct_config_cli::run(command),
        root_command::Command::Check(command) => {
            runtime("Athanor direct check runtime")?.block_on(direct_check_cli::run(command))
        }
        root_command::Command::Rustok(command) => {
            runtime("Athanor direct Rustok runtime")?.block_on(rustok_cli::run(command))
        }
        root_command::Command::Graph(command) => {
            runtime("Athanor direct graph runtime")?.block_on(direct_graph_cli::run(command))
        }
        root_command::Command::Context(command) => {
            runtime("Athanor direct context runtime")?.block_on(direct_context_cli::run(command))
        }
        root_command::Command::Search(command) => {
            runtime("Athanor direct search runtime")?.block_on(direct_search_cli::run(command))
        }
        root_command::Command::Read(command) => {
            runtime("Athanor direct read runtime")?.block_on(direct_read::run(command))
        }
        root_command::Command::Legacy => legacy::run(),
    }
}

fn runtime(label: &str) -> Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .with_context(|| format!("failed to start {label}"))
}
