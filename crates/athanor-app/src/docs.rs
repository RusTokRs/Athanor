//! Documentation completeness, drift, repair proposal, and patch application services.

mod api_docs;
mod check;
mod frontmatter;
mod model;
mod operations;
mod proposal;
mod service;

#[cfg(test)]
mod tests;

pub use model::*;
pub use service::{check_docs, docs_apply_patch, docs_drift, docs_propose_fix};

pub(crate) use check::build_docs_drift_report;
pub(crate) use proposal::build_docs_patch_proposal_from_snapshot;
