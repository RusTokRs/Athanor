use athanor_app::{AdapterPluginKind, AdapterRegistry};
use athanor_runtime_defaults::resolve_builtin_adapter;

#[test]
fn builtin_axum_extractor_resolves_without_external_process_configuration() {
    let resolved = resolve_builtin_adapter(
        AdapterRegistry::empty(),
        AdapterPluginKind::Extractor,
        "builtin.extractor.axum",
    );

    assert!(resolved.is_some());
}
