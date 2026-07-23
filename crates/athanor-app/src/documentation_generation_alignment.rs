//! Cross-contract alignment for the documentation-generation contract chain.

use crate::{
    DocumentationContext, DocumentationContractError, DocumentationDraft,
    DocumentationGenerationRequest, DocumentationOutline, DocumentationValidationReport,
};

/// Validates request, outline, context, and draft as one snapshot-bound contract chain.
pub fn validate_documentation_draft_chain(
    request: &DocumentationGenerationRequest,
    outline: &DocumentationOutline,
    context: &DocumentationContext,
    draft: &DocumentationDraft,
) -> Result<(), DocumentationContractError> {
    outline.validate_for_request(request)?;
    context.validate_for_request_and_outline(request, outline)?;
    draft.validate_for_context_and_outline(context, outline)?;

    if context.snapshot != outline.snapshot || context.profile != outline.profile {
        return Err(error(
            "documentation context does not match outline snapshot and profile",
        ));
    }
    if context.outline_schema != outline.schema {
        return Err(error(
            "documentation context does not match the supplied outline schema",
        ));
    }

    for (draft_section, outline_section) in draft.sections.iter().zip(&outline.sections) {
        if draft_section.id != outline_section.id || draft_section.title != outline_section.title {
            return Err(error(
                "documentation draft section identity or title differs from the outline",
            ));
        }
    }
    Ok(())
}

/// Validates the complete chain including the final validation report.
pub fn validate_documentation_report_chain(
    request: &DocumentationGenerationRequest,
    outline: &DocumentationOutline,
    context: &DocumentationContext,
    draft: &DocumentationDraft,
    report: &DocumentationValidationReport,
) -> Result<(), DocumentationContractError> {
    validate_documentation_draft_chain(request, outline, context, draft)?;
    report.validate_for_draft_and_context(draft, context)?;

    if draft.snapshot != context.snapshot || draft.profile != context.profile {
        return Err(error(
            "documentation draft does not match context snapshot and profile",
        ));
    }
    if draft.context_schema != context.schema || draft.outline_schema != outline.schema {
        return Err(error(
            "documentation draft does not match context or outline schema",
        ));
    }
    if report.snapshot != context.snapshot || report.profile != context.profile {
        return Err(error(
            "documentation validation report does not match the bounded context",
        ));
    }
    Ok(())
}

fn error(message: impl Into<String>) -> DocumentationContractError {
    DocumentationContractError(message.into())
}
