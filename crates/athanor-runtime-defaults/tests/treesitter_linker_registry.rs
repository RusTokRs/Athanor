use athanor_app::{AdapterPluginKind, AdapterRegistry};
use athanor_runtime_defaults::resolve_builtin_adapter;

#[test]
fn builtin_treesitter_linker_resolves_without_external_process_configuration() {
    let resolved = resolve_builtin_adapter(
        AdapterRegistry::empty(),
        AdapterPluginKind::Linker,
        "builtin.linker.treesitter",
    );

    assert!(resolved.is_some());
}
