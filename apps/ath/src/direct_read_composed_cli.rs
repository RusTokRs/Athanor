// Transitional wrapper for the focused read command family.
//
// `direct_read_cli.rs` already routes every operation through an explicit
// `RuntimeComposition`, but its compatibility body still calls the historical
// installer before constructing that composition. The local module below
// deliberately shadows only that installer call while forwarding production
// composition creation to the real runtime-defaults crate. This keeps parser
// and rendering parity until CLI-001 removes the compatibility include.
mod athanor_runtime_defaults {
    pub(crate) fn install() {}

    pub(crate) fn production() -> athanor_app::RuntimeComposition {
        ::athanor_runtime_defaults::production()
    }
}

include!("direct_read_cli.rs");
