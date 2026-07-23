use std::collections::BTreeSet;

use athanor_app::{
    DocumentationCitation, DocumentationContext, DocumentationDraft, DocumentationGenerationRequest,
    DocumentationOutline, DocumentationValidationReport,
};
use serde_json::Value;

const CONTRACT_FIXTURE: &str =
    include_str!("fixtures/documentation_generation_slice0b.v1.json");
const EVALUATION_CORPUS: &str =
    include_str!("fixtures/documentation_generation_evaluation_corpus.v1.json");

#[test]
fn slice_0b_contracts_are_strict_aligned_and_round_trip() {
    let fixture: Value = serde_json::from_str(CONTRACT_FIXTURE).expect("valid Slice 0B fixture");
    let request: DocumentationGenerationRequest = typed(&fixture, "request");
    let outline: DocumentationOutline = typed(&fixture, "outline");
    let context: DocumentationContext = typed(&fixture, "context");
    let citation: DocumentationCitation = typed(&fixture, "citation");
    let draft: DocumentationDraft = typed(&fixture, "draft");
    let report: DocumentationValidationReport = typed(&fixture, "validation_report");

    outline
        .validate_for_request(&request)
        .expect("outline must match request");
    context
        .validate_for_request_and_outline(&request, &outline)
        .expect("context must remain bounded and aligned");
    citation.validate().expect("citation fixture must be valid");
    draft
        .validate_for_context_and_outline(&context, &outline)
        .expect("draft must remain cited and aligned");
    report
        .validate_for_draft_and_context(&draft, &context)
        .expect("validation report must match draft and policy");

    for (name, value) in [
        ("request", serde_json::to_value(request).unwrap()),
        ("outline", serde_json::to_value(outline).unwrap()),
        ("context", serde_json::to_value(context).unwrap()),
        ("citation", serde_json::to_value(citation).unwrap()),
        ("draft", serde_json::to_value(draft).unwrap()),
        ("validation_report", serde_json::to_value(report).unwrap()),
    ] {
        assert_eq!(value, fixture[name], "{name} contract drifted from fixture");
    }
}

#[test]
fn schema_policy_evidence_and_citation_failures_are_rejected() {
    let fixture: Value = serde_json::from_str(CONTRACT_FIXTURE).unwrap();
    let request: DocumentationGenerationRequest = typed(&fixture, "request");
    let outline: DocumentationOutline = typed(&fixture, "outline");
    let context: DocumentationContext = typed(&fixture, "context");

    let mut unknown_outline = fixture["outline"].clone();
    unknown_outline["provider_prompt"] = Value::String("read the repository".to_string());
    assert!(serde_json::from_value::<DocumentationOutline>(unknown_outline).is_err());

    let mut duplicate_outline: DocumentationOutline = typed(&fixture, "outline");
    duplicate_outline.sections.push(duplicate_outline.sections[0].clone());
    assert!(duplicate_outline.validate_for_request(&request).is_err());

    let mut unsafe_context: DocumentationContext = typed(&fixture, "context");
    unsafe_context.policy.raw_file_access = true;
    assert!(
        unsafe_context
            .validate_for_request_and_outline(&request, &outline)
            .is_err()
    );

    let mut malformed_relation: DocumentationContext = typed(&fixture, "context");
    malformed_relation.items[2].relation_direction = None;
    assert!(
        malformed_relation
            .validate_for_request_and_outline(&request, &outline)
            .is_err()
    );

    let mut bad_citation: DocumentationCitation = typed(&fixture, "citation");
    bad_citation.evidence[0].start_line = 0;
    assert!(bad_citation.validate().is_err());

    let mut uncited_draft: DocumentationDraft = typed(&fixture, "draft");
    uncited_draft.sections[0].claims[0].citation_ids.clear();
    uncited_draft.sections[0].claims[0].inference = None;
    assert!(
        uncited_draft
            .validate_for_context_and_outline(&context, &outline)
            .is_err()
    );

    let mut escaping_draft: DocumentationDraft = typed(&fixture, "draft");
    escaping_draft.citations[0].stable_keys = vec!["file://outside.rs".to_string()];
    assert!(
        escaping_draft
            .validate_for_context_and_outline(&context, &outline)
            .is_err()
    );
}

#[test]
fn validation_status_and_provider_metrics_fail_closed() {
    let fixture: Value = serde_json::from_str(CONTRACT_FIXTURE).unwrap();
    let context: DocumentationContext = typed(&fixture, "context");
    let draft: DocumentationDraft = typed(&fixture, "draft");

    let mut provider_metrics: DocumentationValidationReport = typed(&fixture, "validation_report");
    provider_metrics.metrics.prompt_tokens = Some(1);
    assert!(
        provider_metrics
            .validate_for_draft_and_context(&draft, &context)
            .is_err()
    );

    let mut invalid_score: DocumentationValidationReport = typed(&fixture, "validation_report");
    invalid_score.metrics.citation_validity_basis_points = 9_999;
    assert!(
        invalid_score
            .validate_for_draft_and_context(&draft, &context)
            .is_err()
    );

    let mut invalid_without_error: DocumentationValidationReport =
        typed(&fixture, "validation_report");
    invalid_without_error.status = athanor_app::DocumentationValidationStatus::Invalid;
    assert!(
        invalid_without_error
            .validate_for_draft_and_context(&draft, &context)
            .is_err()
    );
}

#[test]
fn evaluation_corpus_is_bounded_reviewable_and_policy_protected() {
    let corpus: Value = serde_json::from_str(EVALUATION_CORPUS).expect("valid evaluation corpus");
    assert_eq!(
        corpus["schema"],
        "athanor.documentation_evaluation_corpus.v1"
    );
    assert_eq!(corpus["policy"]["provider_opt_in"], true);
    assert_eq!(corpus["policy"]["raw_file_explorer_tools"], false);
    assert_eq!(corpus["policy"]["secrets_allowed"], false);

    let metrics = corpus["quality_metrics"].as_array().expect("quality metrics");
    let metric_names = metrics
        .iter()
        .map(|value| value.as_str().expect("metric name"))
        .collect::<BTreeSet<_>>();
    for required in [
        "citation_coverage",
        "citation_validity",
        "diagram_validity",
        "unsupported_relation_disclosure",
        "deterministic_repeatability",
        "prompt_token_cost",
        "human_review_score",
    ] {
        assert!(metric_names.contains(required), "missing quality metric {required}");
    }

    let cases = corpus["cases"].as_array().expect("evaluation cases");
    assert!(cases.len() >= 2);
    let mut ids = BTreeSet::new();
    for case in cases {
        let id = case["id"].as_str().expect("case id");
        assert!(ids.insert(id), "duplicate evaluation case {id}");
        assert!(!case["expected_sections"].as_array().unwrap().is_empty());
        assert!(!case["expected_citation_paths"].as_array().unwrap().is_empty());
        assert!(!case["expected_diagram_edges"].as_array().unwrap().is_empty());
        assert!(!case["known_gaps"].as_array().unwrap().is_empty());

        for path in case["expected_citation_paths"].as_array().unwrap() {
            assert_portable_relative(path.as_str().unwrap());
        }
        for file in case["files"].as_array().unwrap() {
            assert_portable_relative(file["path"].as_str().unwrap());
            assert!(!file["content"].as_str().unwrap().is_empty());
        }
    }
    assert!(ids.contains("minimal-rust-service"));
    assert!(ids.contains("rustok-architecture-baseline"));
}

fn typed<T: serde::de::DeserializeOwned>(fixture: &Value, field: &str) -> T {
    serde_json::from_value(fixture[field].clone())
        .unwrap_or_else(|error| panic!("invalid {field} fixture: {error}"))
}

fn assert_portable_relative(path: &str) {
    assert!(!path.is_empty());
    assert!(!path.starts_with('/') && !path.starts_with('\\'));
    assert!(!path.contains('\\'));
    assert!(
        path.split('/')
            .all(|component| !component.is_empty() && !matches!(component, "." | ".."))
    );
}
