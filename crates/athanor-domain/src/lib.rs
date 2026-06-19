//! Domain model for Athanor.
//!
//! This crate is deliberately free of storage, transport, parser, and UI
//! dependencies. It contains only the canonical knowledge vocabulary shared by
//! all adapters.

pub mod model;

pub use model::*;
