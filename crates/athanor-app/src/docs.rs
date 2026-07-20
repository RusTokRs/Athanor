//! Documentation completeness, drift, repair proposal, and patch application services.

mod api_docs;
mod check;
pub(crate) mod frontmatter;
mod model;
mod operations;
mod proposal;

#[cfg(test)]
mod tests;

pub use model::*;

pub(crate) use check::build_docs_drift_report;
pub(crate) use proposal::build_docs_patch_proposal_from_snapshot;
