//! Stable Store types for application embedders.
//!
//! Store construction is owned by `RuntimeComposition::init_store`; this module exports only the
//! backend-neutral Store handle and factory contract.

#[path = "store.rs"]
mod core;

pub use core::{AthanorStore, StoreFactory};
