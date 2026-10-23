---
id: doc://docs/adapters/extractor-rust.md
kind: module_documentation
language: en
last_verified_snapshot: snap_jsonl_00000272
source_language: en
status: verified
---
# Rust Extractor

Crate: `athanor-extractor-rust`

Port: `Extractor`

`RustExtractor` parses source files with `syn` and emits canonical module, function, symbol, and
environment-variable entities plus `symbol_defined` and `env_var_used` facts pointing to canonical
evidence. Trait declarations include bare `supertraits` paths and their containing module's recorded
imports so the linker can represent resolvable inheritance. Framework-specific semantics remain
outside this base extractor.

The same crate now also exposes a separate bounded `AxumExtractor`, registered independently as
`builtin.extractor.axum`. It projects only high-confidence static Axum `.route(...)` declarations;
see [Axum Route Extractor](extractor-axum.md). Its source presence is not execution evidence for the
previously verified framework-neutral Rust extractor surface.

Stable keys use the `symbol://rust:` namespace and a collision-resistant crate/module prefix derived from the project-relative source path. Workspace member directory names become crate names with hyphens normalized to underscores. Parser spans provide line evidence; every entity and fact is owned by the source file for incremental replacement.

Declarations record the evidence a relation linker needs to resolve a call. Functions, methods, and
trait methods carry a bounded `calls` array of `{callee, name, receiver, kind, line}`: a bare path
call keeps `receiver: null` with `kind: "call"`, and a method call records the receiver expression
with spaces around `.` and wrapping parentheses collapsed so `self.reader.write()`
records as `self.reader`, `(&mut self.reader).write()` as `&mut self.reader`, and
`svc.create().write()` as `svc.create()`, and
`create_repo().get().write()` as `create_repo().get()`, and
`Store::new().write()` as `Store::new()`, and
`Store::new().get().write()` as `Store::new().get()`, and
`crate::create_repo().write()` as `crate::create_repo()`, and
`crate::create_repo().get().write()` as `crate::create_repo().get()`, and
`create_repo()?.write()` as `create_repo()?`, and
`create_repo()?.get().write()` as `create_repo()?.get()`, and
`create_repo().unwrap().write()` as `create_repo().unwrap()`, and
`create_repo().unwrap().get().write()` as `create_repo().unwrap().get()` with
`kind: "method"`. Function, method, and trait-method payloads also carry
`return_type` when the signature names one usable type: `fn create_repo() -> Store`
records `Store`, and so do `-> Box<Store>` and `-> &Store`. On an inherent impl,
`-> Self` is that type: `impl Store { fn new() -> Self }` records `Store`. `-> Result<Store, E>`
and `-> Option<Store>` record `Store` with `try_yield`, so only a `?`,
`.unwrap()`, or `.expect(...)` on that hop may take it. A type parameter that
names one trait bound (`T: Reader` or `where T: Reader`) records that trait with
`return_kind: impl_trait`, and so do `-> impl Reader` and `-> Box<dyn Reader>`
(`trait_object`). Unconstrained `T`, extra bounds, a tuple, a missing `->`, and `Self` outside an
inherent impl record nothing. They also carry a bounded `typed_bindings` array of rows
`{name, type, kind, line}` — `kind` is `annotation`, `trait_object`, or `impl_trait` — read from
typed parameters and annotated locals, including those nested in blocks and match arms, bounded to 64
rows with an `omitted_typed_bindings` count. An unannotated `let store = create_repo()`,
`let store = Store::new()`, `let store = svc.create()` / `self.create()`,
`let store = crate::create_repo()`, one extra hop such as
`let store = create_repo().get()`, `let store = crate::create_repo().get()`, or a trailing `?` such as
`let store = create_repo()?`, or an inner-try hop such as
`let store = create_repo()?.get()`, or a trailing `.unwrap()` / `.expect(...)` such as
`let store = create_repo().unwrap()`, or an inner unwrap hop such as
`let store = create_repo().unwrap().get()` instead records a
bounded `call_bindings` row `{name, callee, line}` so the linker can read that factory's
declared return; a trailing `?` or unwrap/expect also sets `try: true`. A third hop names nothing;
`unwrap_or`, `unwrap_or_else`, and `unwrap_or_default` are not factory bindings. Named struct fields
use the same `typed_bindings` rows, so
`self.reader.write()` can dispatch from the field `App` declares; a tuple or unit struct records
none. References
and `Box`/`Rc`/`Arc` unwrap to the type they name; a generic argument, a trait object with extra
bounds, a primitive, `Self`, and an unannotated local name nothing dispatchable and are not
recorded. A type parameter that names one trait bound — `T: Reader` on the parameter, on the
enclosing impl, trait, or struct, or in a `where T: Reader` clause — records that trait as `impl_trait`, so
the linker dispatches like `impl Reader`. Unconstrained `T`, `T: Reader + Send`, and `T: Into<U>`
leave the parameter name. A type-alias payload carries `aliased_type` when the right-hand side is a
bare named type: `type Client = Store` and `type Client = crate::app::Store` record `Store`. A
generic, a tuple, a reference, and a primitive name nothing the linker can unfold. A trait declaration's own methods are emitted as `symbol_kind: "trait_method"`
functions, so a `dyn Trait` receiver has a declaration to resolve to.

Every `impl Trait for Type` block is recorded as a `symbol_defined` fact with
`symbol_kind: "trait_impl"`, the implementing type name, the trait name, and the trait path as
written. Marker implementations without methods are recorded too, because the fact — not an
implementation-method symbol — is the anchor; the Rust linker turns these facts into `Implements`
bridges. Resolvable trait supertraits are linked separately as `Extends` relations.

Environment variables are detected from common Rust calls such as `std::env::var`,
`std::env::var_os`, and `option_env!`, then emitted as `EntityKind::EnvVar` with `env://<NAME>`
stable keys. `EnvDocsChecker` can later report undocumented variables through `ath check env`.

The base adapter runs in-process, has no network access, and has no side effects. Alternative `cfg`
definitions with the same qualified name are coalesced with multiple evidence spans. `RustExtractor`
does not expand macros, link imports/calls, or infer framework routes; trait implementation facts are
not expanded into synthetic implementation methods.

Test the framework-neutral owner with:

```bash
cargo test -p athanor-extractor-rust
```
