use athanor_app::{
    DocumentationGenerationLimits, DocumentationGenerationRequest,
    DocumentationOnboardingOperationOptions, DocumentationProfile,
};

#[test]
fn onboarding_operation_options_bind_onboarding_profile_and_exact_snapshot_contract() {
    let options = DocumentationOnboardingOperationOptions {
        root: ".".into(),
        request: DocumentationGenerationRequest::new(
            "snap-exact",
            DocumentationProfile::Onboarding,
            DocumentationGenerationLimits {
                max_entities: 8,
                max_facts: 8,
                max_relations: 8,
                max_diagnostics: 4,
            },
        ),
        force: false,
    };

    assert_eq!(options.request.snapshot, "snap-exact");
    assert_eq!(options.request.profile, DocumentationProfile::Onboarding);
    options
        .request
        .validate()
        .expect("valid exact onboarding request");
}
