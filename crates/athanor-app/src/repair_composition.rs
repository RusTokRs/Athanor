//! Explicit-composition facades for repair operations that require canonical storage.

mod direct;

use anyhow::Result;

use crate::composition::RuntimeComposition;
use crate::repair::{
    RepairCanonicalLatestOptions, RepairCanonicalLatestReport, RepairRecoverIndexOptions,
    RepairRecoverIndexReport,
};

pub async fn recover_index_publication_with_composition(
    options: RepairRecoverIndexOptions,
    composition: &RuntimeComposition,
) -> Result<RepairRecoverIndexReport> {
    direct::recover_index(options, composition).await
}

pub async fn repair_canonical_latest_with_composition(
    options: RepairCanonicalLatestOptions,
    composition: &RuntimeComposition,
) -> Result<RepairCanonicalLatestReport> {
    direct::repair_latest(options, composition).await
}
