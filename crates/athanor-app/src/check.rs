use std::path::PathBuf;

use crate::store::init_store;
use anyhow::{Context, Result};
use athanor_core::CanonicalSnapshotStore;
use athanor_domain::{Diagnostic, DiagnosticKind, DiagnosticStatus, Severity};
use serde::{Deserialize, Serialize};

use crate::project_path::normalize_canonical_path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticScope {
    Api,
    Docs,
}

#[derive(Debug, Clone)]
pub struct DiagnosticCheckOptions {
    pub root: PathBuf,
    pub scope: DiagnosticScope,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct DiagnosticCounts {
    pub total: usize,
    pub critical: usize,
    pub high: usize,
    pub medium: usize,
    pub low: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticCheckReport {
    pub schema: String,
    pub snapshot: String,
    pub scope: DiagnosticScope,
    pub counts: DiagnosticCounts,
    pub diagnostics: Vec<Diagnostic>,
}

use crate::config::{ApiConfig, ApiSourceOfTruth, load_config};

pub async fn check_project(options: DiagnosticCheckOptions) -> Result<DiagnosticCheckReport> {
    let root = normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );
    let config = load_config(&root)?;
    let store = init_store(&root, &config).await?;
    let snapshot = store
        .load_latest_snapshot()
        .await
        .context("failed to load latest canonical snapshot")?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "no canonical snapshot found; run `ath index {}` first",
                root.display()
            )
        })?;
    let snapshot_id = snapshot
        .snapshot
        .as_ref()
        .map_or_else(|| "unknown".to_string(), |snapshot| snapshot.0.clone());

    Ok(build_check_report(
        snapshot_id,
        options.scope,
        &snapshot.diagnostics,
        &config.api,
    ))
}

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
                DiagnosticScope::Docs => true,
            }
        })
        .cloned()
        .collect::<Vec<_>>();
    diagnostics
        .sort_by_key(|diagnostic| (severity_rank(diagnostic.severity), diagnostic.id.0.clone()));

    let mut counts = DiagnosticCounts {
        total: diagnostics.len(),
        ..DiagnosticCounts::default()
    };
    for diagnostic in &diagnostics {
        match diagnostic.severity {
            Severity::Critical => counts.critical += 1,
            Severity::High => counts.high += 1,
            Severity::Medium => counts.medium += 1,
            Severity::Low => counts.low += 1,
        }
    }

    DiagnosticCheckReport {
        schema: "athanor.diagnostic_check.v1".to_string(),
        snapshot,
        scope,
        counts,
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
                | DiagnosticKind::MissingEnvVar
        ),
    }
}

fn severity_rank(severity: Severity) -> u8 {
    match severity {
        Severity::Critical => 0,
        Severity::High => 1,
        Severity::Medium => 2,
        Severity::Low => 3,
    }
}

#[cfg(test)]
mod tests {
    use athanor_domain::{DiagnosticId, EntityId, SnapshotId};
    use serde_json::json;

    use super::*;

    #[test]
    fn filters_open_api_diagnostics_and_counts_severity() {
        let diagnostics = vec![
            diagnostic(
                "diag_api_high",
                DiagnosticKind::ApiRequestSchemaMismatch,
                Severity::High,
                DiagnosticStatus::Open,
            ),
            diagnostic(
                "diag_api_medium",
                DiagnosticKind::ApiEndpointImplementedButNotDocumented,
                Severity::Medium,
                DiagnosticStatus::Open,
            ),
            diagnostic(
                "diag_docs",
                DiagnosticKind::DocumentationPageMissingTitle,
                Severity::Medium,
                DiagnosticStatus::Open,
            ),
            diagnostic(
                "diag_resolved",
                DiagnosticKind::ApiPathMismatch,
                Severity::Critical,
                DiagnosticStatus::Resolved,
            ),
        ];

        let report = build_check_report(
            "snap_test".to_string(),
            DiagnosticScope::Api,
            &diagnostics,
            &ApiConfig::default(),
        );

        assert_eq!(report.schema, "athanor.diagnostic_check.v1");
        assert_eq!(report.counts.total, 2);
        assert_eq!(report.counts.high, 1);
        assert_eq!(report.counts.medium, 1);
        assert_eq!(report.diagnostics[0].id.0, "diag_api_high");
    }

    #[test]
    fn filters_documentation_diagnostics() {
        let diagnostics = vec![
            diagnostic(
                "diag_docs",
                DiagnosticKind::EmptyDocumentationPage,
                Severity::High,
                DiagnosticStatus::Open,
            ),
            diagnostic(
                "diag_docs_reference",
                DiagnosticKind::DocumentationReferenceUnresolved,
                Severity::Medium,
                DiagnosticStatus::Open,
            ),
            diagnostic(
                "diag_api",
                DiagnosticKind::ApiMethodMismatch,
                Severity::High,
                DiagnosticStatus::Open,
            ),
        ];

        let report = build_check_report(
            "snap_test".to_string(),
            DiagnosticScope::Docs,
            &diagnostics,
            &ApiConfig::default(),
        );

        assert_eq!(report.counts.total, 2);
        assert_eq!(report.diagnostics[0].id.0, "diag_docs");
    }

    #[test]
    fn openapi_first_policy_filters_out_missing_docs_unless_fail_on_missing_docs() {
        let diagnostics = vec![
            diagnostic(
                "diag_missing_docs",
                DiagnosticKind::ApiEndpointImplementedButNotDocumented,
                Severity::Medium,
                DiagnosticStatus::Open,
            ),
            diagnostic(
                "diag_missing_impl",
                DiagnosticKind::ApiEndpointDocumentedButNotImplemented,
                Severity::High,
                DiagnosticStatus::Open,
            ),
        ];

        // Case 1: openapi-first and fail_on_missing_docs is false
        let mut config = ApiConfig {
            source_of_truth: ApiSourceOfTruth::OpenapiFirst,
            fail_on_missing_docs: false,
            ..Default::default()
        };
        let report = build_check_report(
            "snap_test".to_string(),
            DiagnosticScope::Api,
            &diagnostics,
            &config,
        );
        assert_eq!(report.counts.total, 1);
        assert_eq!(report.diagnostics[0].id.0, "diag_missing_impl");

        // Case 2: openapi-first and fail_on_missing_docs is true
        config.fail_on_missing_docs = true;
        let report = build_check_report(
            "snap_test".to_string(),
            DiagnosticScope::Api,
            &diagnostics,
            &config,
        );
        assert_eq!(report.counts.total, 2);
    }

    #[test]
    fn code_first_policy_filters_out_missing_implementations() {
        let diagnostics = vec![diagnostic(
            "diag_missing_impl",
            DiagnosticKind::ApiEndpointDocumentedButNotImplemented,
            Severity::High,
            DiagnosticStatus::Open,
        )];

        let config = ApiConfig {
            source_of_truth: ApiSourceOfTruth::CodeFirst,
            ..Default::default()
        };
        let report = build_check_report(
            "snap_test".to_string(),
            DiagnosticScope::Api,
            &diagnostics,
            &config,
        );
        assert_eq!(report.counts.total, 0);
    }

    #[test]
    fn fail_on_openapi_mismatch_filters_out_mismatches() {
        let diagnostics = vec![diagnostic(
            "diag_mismatch",
            DiagnosticKind::ApiRequestSchemaMismatch,
            Severity::High,
            DiagnosticStatus::Open,
        )];

        let config = ApiConfig {
            fail_on_openapi_mismatch: false,
            ..Default::default()
        };
        let report = build_check_report(
            "snap_test".to_string(),
            DiagnosticScope::Api,
            &diagnostics,
            &config,
        );
        assert_eq!(report.counts.total, 0);
    }

    #[test]
    fn fail_on_undocumented_status_code_filters_out_undocumented_codes() {
        let diagnostics = vec![diagnostic(
            "diag_code",
            DiagnosticKind::ApiStatusCodeUndocumented,
            Severity::Medium,
            DiagnosticStatus::Open,
        )];

        let config = ApiConfig {
            fail_on_undocumented_status_code: false,
            ..Default::default()
        };
        let report = build_check_report(
            "snap_test".to_string(),
            DiagnosticScope::Api,
            &diagnostics,
            &config,
        );
        assert_eq!(report.counts.total, 0);
    }

    fn diagnostic(
        id: &str,
        kind: DiagnosticKind,
        severity: Severity,
        status: DiagnosticStatus,
    ) -> Diagnostic {
        Diagnostic {
            id: DiagnosticId(id.to_string()),
            kind,
            severity,
            status,
            title: id.to_string(),
            message: id.to_string(),
            entities: vec![EntityId("ent_test".to_string())],
            evidence: Vec::new(),
            ownership: Vec::new(),
            snapshot: SnapshotId("snap_test".to_string()),
            suggested_fix: None,
            payload: json!({}),
        }
    }
}
