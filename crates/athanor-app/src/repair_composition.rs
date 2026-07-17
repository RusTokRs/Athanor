//! Explicit-composition facades for repair operations that require canonical storage.

use anyhow::Result;

use crate::composition::RuntimeComposition;
use crate::repair::{
    RepairCanonicalLatestOptions, RepairCanonicalLatestReport, repair_canonical_latest,
};
use crate::repair_recovery::{
    RepairRecoverIndexOptions, RepairRecoverIndexReport, recover_index_publication,
};
use crate::store::with_store_composition;

pub async fn recover_index_publication_with_composition(
    options: RepairRecoverIndexOptions,
    composition: &RuntimeComposition,
) -> Result<RepairRecoverIndexReport> {
    with_store_composition(composition.clone(), recover_index_publication(options)).await
}

pub async fn repair_canonical_latest_with_composition(
    options: RepairCanonicalLatestOptions,
    composition: &RuntimeComposition,
) -> Result<RepairCanonicalLatestReport> {
    with_store_composition(composition.clone(), repair_canonical_latest(options)).await
}
