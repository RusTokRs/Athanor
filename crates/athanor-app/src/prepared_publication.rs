//! Compatibility re-export for the core-owned prepared publication protocol.
//!
//! `PreparedSnapshot` and `PreparedSnapshotPublication` belong beside `KnowledgeStore` in
//! `athanor-core`. Keeping this module preserves the existing `athanor_app` public API and internal
//! paths while stores can exercise the same typed lifecycle without depending on the application
//! crate.

pub use athanor_core::{PreparedSnapshot, PreparedSnapshotPublication};
