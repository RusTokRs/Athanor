//! Compatibility bridge for the path-attributed onboarding profile module.
//!
//! `documentation_onboarding_profile` is declared with `#[path = "documentation_onboarding_profile_v1.rs"]`
//! from `lib.rs`. Rust therefore resolves its unqualified `mod rendering;` from `src/rendering.rs`.
//! Keep the bounded implementation in its dedicated profile directory and re-export only the three
//! functions consumed by the parent profile.

#[path = "documentation_onboarding_profile_v1/rendering.rs"]
mod implementation;

pub(super) use implementation::{build_draft, build_validation_report, render_markdown};
