use athanor_app::{
    DocumentationGenerationLimits, DocumentationGenerationRequest, DocumentationModuleOperationOptions,
    DocumentationProfile,
};

#[test]
fn module_operation_options_bind_module_profile_and_exact_snapshot_contract() {
    let options = DocumentationModuleOperationOptions {
        root: ".".into(),
        request: DocumentationGenerationRequest::new(
            "snap-exact",
            DocumentationProfile::Module,
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
    assert_eq!(options.request.profile, DocumentationProfile::Module);
    options.request.validate().expect("valid exact module request");
}
