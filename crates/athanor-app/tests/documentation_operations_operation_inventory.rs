use athanor_app::{
    DocumentationGenerationLimits, DocumentationGenerationRequest,
    DocumentationOperationsOperationOptions, DocumentationProfile,
};

#[test]
fn operations_operation_options_bind_operations_profile_and_exact_snapshot_contract() {
    let options = DocumentationOperationsOperationOptions {
        root: ".".into(),
        request: DocumentationGenerationRequest::new(
            "snap-exact",
            DocumentationProfile::Operations,
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
    assert_eq!(options.request.profile, DocumentationProfile::Operations);
    options
        .request
        .validate()
        .expect("valid exact operations request");
}
