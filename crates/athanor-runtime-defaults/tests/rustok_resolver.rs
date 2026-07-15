use athanor_app::{AdapterPluginKind, AdapterRegistry};
use athanor_runtime_defaults::resolve_builtin_adapter;

#[test]
fn resolves_all_rustok_linkers_and_checkers() {
    for id in [
        "builtin.linker.rustok_ffa",
        "builtin.linker.rustok_fba",
        "builtin.linker.rustok_page_builder",
    ] {
        assert!(
            resolve_builtin_adapter(AdapterRegistry::empty(), AdapterPluginKind::Linker, id)
                .is_some(),
            "missing linker resolver for {id}"
        );
    }

    for id in [
        "builtin.checker.rustok_ffa",
        "builtin.checker.rustok_fba",
        "builtin.checker.rustok_page_builder",
    ] {
        assert!(
            resolve_builtin_adapter(AdapterRegistry::empty(), AdapterPluginKind::Checker, id)
                .is_some(),
            "missing checker resolver for {id}"
        );
    }
}
