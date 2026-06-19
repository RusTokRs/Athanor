//! Core ports for Athanor.
//!
//! Implementations live in adapter crates. This crate owns only contracts and
//! request/response shapes used by the application layer.

pub mod ports;

pub use ports::*;
