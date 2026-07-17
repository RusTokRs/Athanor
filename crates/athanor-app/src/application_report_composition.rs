//! Explicit-composition facades for application report operations.
//!
//! The underlying API and documentation services still expose compatibility
//! entry points that resolve stores through the legacy facade. These wrappers
//! bind that lookup to a task-local `RuntimeComposition` without mutating
//! process-global state.

use anyhow::Result;

use crate::api::{
    ApiSnapshotOptions, ApiSnapshotReport, snapshot_api_contract,
};
use crate::composition::RuntimeComposition;
use crate::docs::{
    DocsProposeFixOptions, DocsProposeFixReport, docs_propose_fix,
};
use crate::store::with_store_composition;

pub async fn snapshot_api_contract_with_composition(
    options: ApiSnapshotOptions,
    composition: &RuntimeComposition,
) -> Result<ApiSnapshotReport> {
    with_store_composition(composition.clone(), snapshot_api_contract(options)).await
}

pub async fn docs_propose_fix_with_composition(
    options: DocsProposeFixOptions,
    composition: &RuntimeComposition,
) -> Result<DocsProposeFixReport> {
    with_store_composition(composition.clone(), docs_propose_fix(options)).await
}
