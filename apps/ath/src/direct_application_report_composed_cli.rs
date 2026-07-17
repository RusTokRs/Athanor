// Transitional composition-only wrapper for versioned application reports.
//
// The existing parser and renderers remain unchanged until CLI-001 removes the
// compatibility include. Store access is bound to an explicit production
// composition through the application report facade; no process-global runtime
// is installed by this active path.
mod athanor_runtime_defaults {
    pub(crate) fn install() {}
}

mod athanor_app {
    pub use ::athanor_app::{
        ApiRetentionOverrides, ApiSnapshotOptions, ApiSnapshotReport, DocsProposeFixOptions,
        DocsProposeFixReport, VersionedApiSnapshotReport, VersionedDocsProposeFixReport,
    };

    pub async fn snapshot_api_contract(
        options: ApiSnapshotOptions,
    ) -> anyhow::Result<ApiSnapshotReport> {
        let composition = ::athanor_runtime_defaults::production();
        ::athanor_app::snapshot_api_contract_with_composition(options, &composition).await
    }

    pub async fn docs_propose_fix(
        options: DocsProposeFixOptions,
    ) -> anyhow::Result<DocsProposeFixReport> {
        let composition = ::athanor_runtime_defaults::production();
        ::athanor_app::docs_propose_fix_with_composition(options, &composition).await
    }
}

include!("direct_application_report_cli.rs");
