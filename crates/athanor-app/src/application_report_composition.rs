//! Explicit-composition facades for application report and documentation operations.

mod api_direct;
mod docs_direct;

use anyhow::Result;

use crate::api::{ApiSnapshotOptions, ApiSnapshotReport};
use crate::api_registry::{ApiRegistryOptions, ApiRegistryReport};
use crate::composition::RuntimeComposition;
use crate::docs::{
    DocsApplyPatchOptions, DocsApplyPatchReport, DocsCheckOptions, DocsCheckReport,
    DocsDriftOptions, DocsDriftReport, DocsProposeFixOptions, DocsProposeFixReport,
};

pub async fn snapshot_api_contract_with_composition(
    options: ApiSnapshotOptions,
    composition: &RuntimeComposition,
) -> Result<ApiSnapshotReport> {
    api_direct::snapshot(options, composition).await
}

pub async fn query_api_registry_with_composition(
    options: ApiRegistryOptions,
    composition: &RuntimeComposition,
) -> Result<ApiRegistryReport> {
    api_direct::registry(options, composition).await
}

pub async fn check_docs_with_composition(
    options: DocsCheckOptions,
    composition: &RuntimeComposition,
) -> Result<DocsCheckReport> {
    docs_direct::check(options, composition).await
}

pub async fn docs_drift_with_composition(
    options: DocsDriftOptions,
    composition: &RuntimeComposition,
) -> Result<DocsDriftReport> {
    docs_direct::drift(options, composition).await
}

pub async fn docs_propose_fix_with_composition(
    options: DocsProposeFixOptions,
    composition: &RuntimeComposition,
) -> Result<DocsProposeFixReport> {
    docs_direct::propose(options, composition).await
}

pub async fn docs_apply_patch_with_composition(
    options: DocsApplyPatchOptions,
    composition: &RuntimeComposition,
) -> Result<DocsApplyPatchReport> {
    docs_direct::apply(options, composition).await
}
