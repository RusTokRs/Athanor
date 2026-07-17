//! Explicit-composition facades for application report and documentation operations.
//!
//! The underlying API and documentation services still expose compatibility
//! entry points that resolve stores through the legacy facade. These wrappers
//! bind that lookup to a task-local `RuntimeComposition` without mutating
//! process-global state.

use anyhow::Result;

use crate::api::{ApiSnapshotOptions, ApiSnapshotReport, snapshot_api_contract};
use crate::api_registry::{ApiRegistryOptions, ApiRegistryReport, query_api_registry};
use crate::composition::RuntimeComposition;
use crate::docs::{
    DocsApplyPatchOptions, DocsApplyPatchReport, DocsCheckOptions, DocsCheckReport,
    DocsDriftOptions, DocsDriftReport, DocsProposeFixOptions, DocsProposeFixReport, check_docs,
    docs_apply_patch, docs_drift, docs_propose_fix,
};
use crate::store::with_store_composition;

pub async fn snapshot_api_contract_with_composition(
    options: ApiSnapshotOptions,
    composition: &RuntimeComposition,
) -> Result<ApiSnapshotReport> {
    with_store_composition(composition.clone(), snapshot_api_contract(options)).await
}

pub async fn query_api_registry_with_composition(
    options: ApiRegistryOptions,
    composition: &RuntimeComposition,
) -> Result<ApiRegistryReport> {
    with_store_composition(composition.clone(), query_api_registry(options)).await
}

pub async fn check_docs_with_composition(
    options: DocsCheckOptions,
    composition: &RuntimeComposition,
) -> Result<DocsCheckReport> {
    with_store_composition(composition.clone(), check_docs(options)).await
}

pub async fn docs_drift_with_composition(
    options: DocsDriftOptions,
    composition: &RuntimeComposition,
) -> Result<DocsDriftReport> {
    with_store_composition(composition.clone(), docs_drift(options)).await
}

pub async fn docs_propose_fix_with_composition(
    options: DocsProposeFixOptions,
    composition: &RuntimeComposition,
) -> Result<DocsProposeFixReport> {
    with_store_composition(composition.clone(), docs_propose_fix(options)).await
}

pub async fn docs_apply_patch_with_composition(
    options: DocsApplyPatchOptions,
    composition: &RuntimeComposition,
) -> Result<DocsApplyPatchReport> {
    with_store_composition(composition.clone(), docs_apply_patch(options)).await
}
