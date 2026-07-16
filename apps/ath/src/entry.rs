use anyhow::{Context, Result};

mod direct_check_cli;
mod direct_graph_cli;
mod direct_operation;
mod direct_read_cli;
mod direct_rustok_cli;
mod direct_rustok_help;
mod direct_search_cli;
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

    pub(crate) fn print_related_graph_bridge(report: &athanor_app::GraphRelated) {
        print_related_graph(report);
    }

    pub(crate) fn print_graph_path_bridge(report: &athanor_app::GraphPath) {
        print_graph_path(report);
    }

    pub(crate) fn print_graph_hubs_bridge(report: &athanor_app::GraphHubs) {
        print_graph_hubs(report);
    }

    pub(crate) fn print_graph_pagerank_bridge(report: &athanor_app::GraphPageRank) {
        print_graph_pagerank(report);
    }

    pub(crate) fn print_graph_cycles_bridge(report: &athanor_app::GraphCycles) {
        print_graph_cycles(report);
    }

    pub(crate) fn print_rustok_architecture_context_bridge(
        report: &athanor_app::RustokArchitectureContext,
    ) {
        print_rustok_architecture_context(report);
    }

    pub(crate) fn print_rustok_ffa_audit_bridge(report: &athanor_app::RustokFfaAudit) {
        print_rustok_ffa_audit(report);
    }

    pub(crate) fn print_rustok_ffa_graph_bridge(report: &athanor_app::RustokFfaGraph) {
        print_rustok_ffa_graph(report);
    }

    pub(crate) fn print_rustok_fba_audit_bridge(report: &athanor_app::RustokFbaAudit) {
        print_rustok_fba_audit(report);
    }

    pub(crate) fn print_rustok_fba_graph_bridge(report: &athanor_app::RustokFbaGraph) {
        print_rustok_fba_graph(report);
    }

    pub(crate) fn print_rustok_page_builder_audit_bridge(
        report: &athanor_app::RustokPageBuilderAudit,
    ) {
        print_rustok_page_builder_audit(report);
    }

    pub(crate) fn print_rustok_page_builder_graph_bridge(
        report: &athanor_app::RustokPageBuilderGraph,
    ) {
        print_rustok_page_builder_graph(report);
    }

    pub(crate) fn print_affected_check_bridge(
        report: &athanor_app::AffectedCheckReport,
    ) -> anyhow::Result<()> {
        print_affected_check_report(report)
    }

    pub(crate) fn print_check_bridge(
        report: &athanor_app::DiagnosticCheckReport,
    ) -> anyhow::Result<()> {
        print_check_report(report)
    }

    pub(crate) fn print_api_contract_diff_bridge(
        report: &athanor_app::ApiContractDiff,
    ) -> anyhow::Result<()> {
        print_api_contract_diff(report)
    }
}

fn main() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if direct_rustok_help::print_if_requested(&args) {
        return Ok(());
    }
    if let Some(command) = repair_cli::parse(&args)? {
        #[allow(deprecated)]
        {
            athanor_runtime_defaults::install();
        }
        return runtime("Athanor repair runtime")?.block_on(repair_cli::run(command));
    }
    if let Some(command) = direct_check_cli::parse(&args)? {
        return runtime("Athanor direct check runtime")?.block_on(direct_check_cli::run(command));
    }
    if let Some(command) = direct_rustok_cli::parse(&args)? {
        return runtime("Athanor direct Rustok runtime")?
            .block_on(direct_rustok_cli::run(command));
    }
    if let Some(command) = direct_graph_cli::parse(&args)? {
        return runtime("Athanor direct graph runtime")?.block_on(direct_graph_cli::run(command));
    }
    if let Some(command) = direct_search_cli::parse(&args)? {
        return runtime("Athanor direct search runtime")?.block_on(direct_search_cli::run(command));
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
