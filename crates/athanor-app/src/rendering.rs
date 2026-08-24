//! Compatibility bridge for the path-attributed onboarding profile module.
//!
//! `documentation_onboarding_profile` is declared with `#[path = "documentation_onboarding_profile_v1.rs"]`
//! from `lib.rs`. Rust therefore resolves its unqualified `mod rendering;` from `src/rendering.rs`.
//! Keep the bounded implementation in its dedicated profile directory and expose only the three
//! functions consumed by the parent profile.

#[path = "documentation_onboarding_profile_v1/rendering.rs"]
mod implementation;

pub(super) fn build_draft(
    outline: &crate::DocumentationOutline,
    context: &crate::DocumentationContext,
) -> crate::DocumentationDraft {
    implementation::build_draft(outline, context)
}

pub(super) fn build_validation_report(
    draft: &crate::DocumentationDraft,
    context: &crate::DocumentationContext,
) -> crate::DocumentationValidationReport {
    implementation::build_validation_report(draft, context)
}

pub(super) fn render_markdown(
    context: &crate::DocumentationContext,
    draft: &crate::DocumentationDraft,
) -> String {
    implementation::render_markdown(context, draft)
}
