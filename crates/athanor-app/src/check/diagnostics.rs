use athanor_domain::{Diagnostic, DiagnosticKind, DiagnosticStatus, Severity};

use crate::config::{ApiConfig, ApiSourceOfTruth};
use crate::json_contract::DIAGNOSTIC_CHECK_SCHEMA_V1;

use super::model::{DiagnosticCheckReport, DiagnosticCounts, DiagnosticScope};

pub fn build_check_report(
    snapshot: String,
    scope: DiagnosticScope,
    diagnostics: &[Diagnostic],
    config: &ApiConfig,
) -> DiagnosticCheckReport {
    let mut diagnostics = diagnostics
        .iter()
        .filter(|diagnostic| {
            if diagnostic.status != DiagnosticStatus::Open
                || !diagnostic_matches_scope(&diagnostic.kind, scope)
                || (scope == DiagnosticScope::Scripts
                    && diagnostic
                        .payload
                        .get("scope")
                        .and_then(serde_json::Value::as_str)
                        != Some("scripts"))
                || (scope == DiagnosticScope::Deployment
                    && diagnostic
                        .payload
                        .get("scope")
                        .and_then(serde_json::Value::as_str)
                        != Some("deployment"))
                || (scope == DiagnosticScope::Env
                    && diagnostic.kind == DiagnosticKind::MissingDocumentation
                    && diagnostic
                        .payload
                        .get("scope")
                        .and_then(serde_json::Value::as_str)
                        != Some("env"))
                || (scope == DiagnosticScope::Runbooks
                    && diagnostic
                        .payload
                        .get("scope")
                        .and_then(serde_json::Value::as_str)
                        != Some("runbooks"))
                || (scope == DiagnosticScope::Docs
                    && diagnostic
                        .payload
                        .get("scope")
                        .and_then(serde_json::Value::as_str)
                        .is_some())
            {
                return false;
            }

            match scope {
                DiagnosticScope::Api => match diagnostic.kind {
                    DiagnosticKind::ApiEndpointImplementedButNotDocumented => {
                        config.fail_on_missing_docs
                    }
                    DiagnosticKind::ApiEndpointDocumentedButNotImplemented => {
                        config.source_of_truth != ApiSourceOfTruth::CodeFirst
                    }
                    DiagnosticKind::ApiRequestSchemaMismatch
                    | DiagnosticKind::ApiResponseSchemaMismatch
                    | DiagnosticKind::ApiExampleInvalid => config.fail_on_openapi_mismatch,
                    DiagnosticKind::ApiStatusCodeUndocumented => {
                        config.fail_on_undocumented_status_code
                    }
                    _ => true,
                },
                DiagnosticScope::Docs
                | DiagnosticScope::Env
                | DiagnosticScope::Scripts
                | DiagnosticScope::Deployment
                | DiagnosticScope::Runbooks
                | DiagnosticScope::RustokFfa
                | DiagnosticScope::RustokFba
                | DiagnosticScope::RustokPageBuilder => true,
            }
        })
        .cloned()
        .collect::<Vec<_>>();
    diagnostics
        .sort_by_key(|diagnostic| (severity_rank(diagnostic.severity), diagnostic.id.0.clone()));

    DiagnosticCheckReport {
        schema: DIAGNOSTIC_CHECK_SCHEMA_V1.to_string(),
        snapshot,
        scope,
        counts: diagnostic_counts(&diagnostics),
        diagnostics,
    }
}

pub(crate) fn diagnostic_matches_scope(kind: &DiagnosticKind, scope: DiagnosticScope) -> bool {
    match scope {
        DiagnosticScope::Api => matches!(
            kind,
            DiagnosticKind::OpenapiMismatch
                | DiagnosticKind::DeadEndpoint
                | DiagnosticKind::ApiEndpointMissingInOpenapi
                | DiagnosticKind::ApiEndpointDocumentedButNotImplemented
                | DiagnosticKind::ApiEndpointImplementedButNotDocumented
                | DiagnosticKind::ApiMethodMismatch
                | DiagnosticKind::ApiPathMismatch
                | DiagnosticKind::ApiRequestSchemaMismatch
                | DiagnosticKind::ApiResponseSchemaMismatch
                | DiagnosticKind::ApiStatusCodeUndocumented
                | DiagnosticKind::ApiAuthRequirementMismatch
                | DiagnosticKind::ApiPermissionMismatch
                | DiagnosticKind::ApiExampleInvalid
                | DiagnosticKind::ApiErrorModelMismatch
                | DiagnosticKind::ApiBreakingChangeDetected
        ),
        DiagnosticScope::Docs => matches!(
            kind,
            DiagnosticKind::EmptyDocumentationPage
                | DiagnosticKind::DocumentationPageMissingTitle
                | DiagnosticKind::MissingDocumentation
                | DiagnosticKind::StaleDocumentation
                | DiagnosticKind::DocumentationReferenceUnresolved
                | DiagnosticKind::DuplicateDocumentationId
                | DiagnosticKind::OrphanDoc
                | DiagnosticKind::TranslationOutdated
        ),
        DiagnosticScope::Env => matches!(
            kind,
            DiagnosticKind::MissingEnvVar | DiagnosticKind::MissingDocumentation
        ),
        DiagnosticScope::Scripts => matches!(kind, DiagnosticKind::MissingDocumentation),
        DiagnosticScope::Deployment => matches!(kind, DiagnosticKind::MissingDocumentation),
        DiagnosticScope::Runbooks => matches!(
            kind,
            DiagnosticKind::MissingDocumentation | DiagnosticKind::StaleDocumentation
        ),
        DiagnosticScope::RustokFfa => {
            matches!(kind, DiagnosticKind::Other(value) if value.starts_with("rustok_ffa_"))
        }
        DiagnosticScope::RustokFba => {
            matches!(kind, DiagnosticKind::Other(value) if value.starts_with("rustok_fba_"))
        }
        DiagnosticScope::RustokPageBuilder => {
            matches!(kind, DiagnosticKind::Other(value) if value.starts_with("rustok_page_builder_"))
        }
    }
}

pub(super) fn diagnostic_counts(diagnostics: &[Diagnostic]) -> DiagnosticCounts {
    let mut counts = DiagnosticCounts {
        total: diagnostics.len(),
        ..DiagnosticCounts::default()
    };
    for diagnostic in diagnostics {
        match diagnostic.severity {
            Severity::Critical => counts.critical += 1,
            Severity::High => counts.high += 1,
            Severity::Medium => counts.medium += 1,
            Severity::Low => counts.low += 1,
        }
    }
    counts
}

pub(super) fn severity_rank(severity: Severity) -> u8 {
    match severity {
        Severity::Critical => 0,
        Severity::High => 1,
        Severity::Medium => 2,
        Severity::Low => 3,
    }
}

pub(super) fn sum_counts<const N: usize>(reports: [&DiagnosticCheckReport; N]) -> DiagnosticCounts {
    reports
        .into_iter()
        .fold(DiagnosticCounts::default(), |mut counts, report| {
            counts.total += report.counts.total;
            counts.critical += report.counts.critical;
            counts.high += report.counts.high;
            counts.medium += report.counts.medium;
            counts.low += report.counts.low;
            counts
        })
}
