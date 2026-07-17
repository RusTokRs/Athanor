// Transitional composition-only wrapper for repair commands.
//
// Retention and cleanup-only operations remain direct. Repair operations that
// require canonical storage bind legacy store lookup to an explicit production
// composition through the repair facade.
mod athanor_app {
    pub use ::athanor_app::{
        IndexGenerationCleanupOptions, IndexGenerationCleanupReport, RepairCanonicalLatestOptions,
        RepairCanonicalLatestReport, RepairRecoverIndexCleanupOptions,
        RepairRecoverIndexCleanupReport, RepairRecoverIndexOptions, RepairRecoverIndexReport,
        cleanup_index_generations, recover_index_cleanup,
    };

    pub async fn recover_index_publication(
        options: RepairRecoverIndexOptions,
    ) -> anyhow::Result<RepairRecoverIndexReport> {
        let composition = ::athanor_runtime_defaults::production();
        ::athanor_app::recover_index_publication_with_composition(options, &composition).await
    }

    pub async fn repair_canonical_latest(
        options: RepairCanonicalLatestOptions,
    ) -> anyhow::Result<RepairCanonicalLatestReport> {
        let composition = ::athanor_runtime_defaults::production();
        ::athanor_app::repair_canonical_latest_with_composition(options, &composition).await
    }
}

include!("repair_cli.rs");
