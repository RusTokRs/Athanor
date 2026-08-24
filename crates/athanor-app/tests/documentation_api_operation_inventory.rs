use athanor_app::{
    DocumentationApiOperationOptions, DocumentationGenerationLimits, DocumentationGenerationRequest,
    DocumentationProfile,
};

#[test]
fn api_operation_options_bind_api_profile_and_exact_snapshot_contract() {
    let options = DocumentationApiOperationOptions {
        root: ".".into(),
        request: DocumentationGenerationRequest::new(
            "snap-exact",
            DocumentationProfile::Api,
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
    assert_eq!(options.request.profile, DocumentationProfile::Api);
    options.request.validate().expect("valid exact API request");
}
