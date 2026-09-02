use athanor_app::{DocumentationCompletenessOperationOptions, DocumentationCompletenessRequest};

#[test]
fn completeness_operation_options_bind_one_exact_snapshot_and_limit() {
    let options = DocumentationCompletenessOperationOptions {
        root: ".".into(),
        request: DocumentationCompletenessRequest::new("snap-exact", 17),
    };

    assert_eq!(options.request.snapshot, "snap-exact");
    assert_eq!(options.request.limit, 17);
    options
        .request
        .validate()
        .expect("valid exact completeness request");
}
