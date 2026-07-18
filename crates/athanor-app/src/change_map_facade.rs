//! Composition-only public facade for ChangeMap.
//!
//! The legacy core remains private while its internal fallback branches are removed incrementally.
//! External callers must supply `RuntimeComposition`.

use anyhow::Result;

use crate::RuntimeComposition;

#[path = "change_map.rs"]
mod core;

pub use core::{
    ChangeMapAnnotation, ChangeMapCompleteness, ChangeMapCounts, ChangeMapEndpoint, ChangeMapFile,
    ChangeMapItem, ChangeMapLimits, ChangeMapOptions, ChangeMapPathStep, ChangeMapQuery,
    ChangeMapReport, ChangeMapTestCoverage, ChangeMapTestStatus,
};

/// Builds a bounded change map with explicitly supplied runtime dependencies.
pub async fn change_map_project_with_composition(
    options: ChangeMapOptions,
    composition: &RuntimeComposition,
) -> Result<ChangeMapReport> {
    core::change_map_project_with_composition(options, composition).await
}
