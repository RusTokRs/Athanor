use anyhow::{Context, Result};

mod direct_read_cli;
mod repair_cli;

mod legacy {
    include!("main.rs");

    pub(crate) fn run() -> anyhow::Result<()> {
        main()
    }

    pub(crate) fn print_explanation_bridge(
        report: &athanor_app::EntityExplanation,
    ) -> anyhow::Result<()> {
        print_explanation(report)
    }

    pub(crate) fn print_overview_bridge(
        report: &athanor_app::RepositoryOverview,
    ) -> anyhow::Result<()> {
        print_overview(report)
    }

    pub(crate) fn print_impact_bridge(
        report: &athanor_app::ImpactAnalysis,
    ) -> anyhow::Result<()> {
        print_impact_analysis(report)
    }

    pub(crate) fn print_change_map_bridge(
        report: &athanor_app::ChangeMapReport,
    ) -> anyhow::Result<()> {
        print_change_map(report)
    }
}

fn main() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if let Some(command) = repair_cli::parse(&args)? {
        #[allow(deprecated)]
        {
            athanor_runtime_defaults::install();
        }
        return runtime("Athanor repair runtime")?.block_on(repair_cli::run(command));
    }
    if let Some(command) = direct_read_cli::parse(&args)? {
        return runtime("Athanor direct read runtime")?.block_on(direct_read_cli::run(command));
    }
    legacy::run()
}

fn runtime(label: &str) -> Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .with_context(|| format!("failed to start {label}"))
}
