# Unsafe Code Policy

Athanor currently permits no Rust `unsafe` code in its workspace.

Any future exception requires an accepted ADR that explains why safe Rust is insufficient, names
the exact invariant, constrains the unsafe region to the smallest possible boundary, and adds tests
or external validation for that invariant. The security workflow compiles the workspace with
`-Dunsafe_code`, so an exception cannot be introduced silently.
