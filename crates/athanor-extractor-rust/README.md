# athanor-extractor-rust

Rust source-code extractor adapters.

Implements: `Extractor`

The crate exposes two extractor types. `RustExtractor` owns framework-neutral Rust symbols and environment-variable usage. `AxumExtractor` is a separate bounded framework projection for static Axum route declarations and is registered independently as `builtin.extractor.axum`.

## What It Emits

`RustExtractor` emits:

- `EntityKind::Module` for inline module declarations
- `EntityKind::Function` for free functions and implementation methods
- `EntityKind::Symbol` for structs, enums, traits, unions, type aliases, constants, and statics.
  A type alias payload carries `aliased_type` when the right-hand side is a bare named type:
  `type Client = Store` records `Store`. A generic, a tuple, a reference, and a primitive record
  nothing.
- `FactKind::SymbolDefined` from every emitted symbol to its canonical file entity
- `supertraits` path arrays and containing-module imports on trait declaration payloads for linker resolution
- `calls` objects on declarations: `{callee, name, receiver, kind, line}`, where a method call records the receiver expression with spaces around `.` and wrapping parentheses collapsed so `self.reader.write()` and `(&mut self.reader).write()` record as `self.reader` / `&mut self.reader`, `svc.create().write()` records receiver `svc.create()`, `create_repo().get().write()` records receiver `create_repo().get()`, `Store::new().write()` records receiver `Store::new()`, `Store::new().get().write()` records receiver `Store::new().get()`, `crate::create_repo().write()` records receiver `crate::create_repo()`, `crate::create_repo().get().write()` records receiver `crate::create_repo().get()`, `create_repo()?.write()` records receiver `create_repo()?`, `create_repo()?.get().write()` records receiver `create_repo()?.get()`, `create_repo().unwrap().write()` records receiver `create_repo().unwrap()`, `create_repo().unwrap().get().write()` records receiver `create_repo().unwrap().get()`, and a bare path call keeps `receiver: null`
- A `return_type` on function, method, and trait-method payloads: the bare named type the signature returns, so `fn create_repo() -> Store` types `create_repo().write()`. On an inherent impl, `-> Self` is that impl type, so `impl Store { fn new() -> Self }` types `Store::new().write()`. `Box`/`Rc`/`Arc` and a reference unwrap to the type they name. `-> Result<Store, E>` and `-> Option<Store>` record `Store` with `try_yield: true`, so only a `?`, `.unwrap()`, or `.expect(...)` on that hop may take it. A type parameter that names one trait bound (`T: Reader` or `where T: Reader`) records that trait with `return_kind: impl_trait`, and so do `-> impl Reader` and `-> Box<dyn Reader>` (`trait_object`). Unconstrained `T`, extra bounds, a tuple, a missing `->`, and `Self` outside an inherent impl record nothing.
- `typed_bindings` rows on declarations: `{name, type, kind, line}` from typed parameters, annotated locals, and named struct fields, with `kind` one of `annotation`, `trait_object`, or `impl_trait`. A type parameter that names one trait bound (`T: Reader` or `where T: Reader`) records that trait as `impl_trait`. A tuple or unit struct records no field.
- `call_bindings` rows on the same payloads: `{name, callee, line}` for an unannotated `let store = create_repo()`, `let store = Store::new()`, `let store = svc.create()` / `self.create()`, `let store = crate::create_repo()`, one extra hop such as `let store = create_repo().get()` or `let store = crate::create_repo().get()`, or a trailing `?` such as `let store = create_repo()?` (`try: true`), or an inner-try hop such as `let store = create_repo()?.get()`, or a trailing `.unwrap()` / `.expect(...)` such as `let store = create_repo().unwrap()` (`try: true`), or an inner unwrap hop such as `let store = create_repo().unwrap().get()`, so the linker can read that factory's declared return. A third hop names nothing.
- `EntityKind::Function` for the methods a trait declares, with `symbol_kind: "trait_method"`
- `EntityKind::EnvVar` and `FactKind::EnvVarUsed` for supported Rust environment-variable access

`AxumExtractor` emits one source-backed `EntityKind::Other("axum_route")` and one `FactKind::RouteDeclared` for each supported static `.route(...)` declaration. The adapter-scoped entity kind keeps framework routes out of generic API/operations/onboarding profile anchors while exact completeness reporting still sees the named `axum` adapter contribution.

All emitted objects include ownership metadata for the Rust source file. Facts include exact parser-derived line evidence.

## Inputs

`RustExtractor` accepts `SourceFile` with:

```text
language_hint = rust
content = UTF-8 Rust source
```

`AxumExtractor` accepts Rust files that reference `axum` and recognizes bounded declarations of the form:

```rust
use axum::routing::{get, post};

Router::new()
    .route("/users", get(list_users))
    .route("/users", post(create_user));
```

Supported method constructors are `get`, `post`, `put`, `delete`, `patch`, `head`, `options`, and `trace`. Direct imports, renamed imports, `axum::routing` namespace imports, and fully-qualified `axum::routing::<method>` calls are supported.

## Stable Keys

```text
symbol://rust:auth_service::auth::login
symbol://rust:auth_service::auth::Session
symbol://rust:auth_service::auth::Session::refresh
axum-route://src/http/routes.rs#GET:/users:list_users
```

The Rust module prefix is derived from the project-relative file path. A root `src/lib.rs` maps to `crate`; `crates/auth-service/src/auth.rs` maps to `auth_service::auth`. Hyphens in workspace member directory names are normalized to Rust underscores.

Axum route keys are source-backed and include the HTTP method, literal route path, and handler path. Windows source separators are normalized for stable-key/payload identity while evidence retains the source-provider path.

## Side Effects

None. The extractors do not run commands, use the network, or modify project files.

## Limitations

`RustExtractor`:

- Macro-generated declarations are not expanded.
- Out-of-line module declarations are emitted as modules but resolved by their own source files rather than linked in this adapter.
- A receiver type is recorded only when the source states one usable name: a generic argument, a
  trait object with extra bounds, a primitive, `Self`, and an unannotated local that is not a
  factory call are skipped. An unannotated `let store = create_repo()` or
  `let store = svc.create()` records `call_bindings` instead. A type
  parameter with one trait bound is that trait; unconstrained `T`, `T: Reader + Send`, and
  `T: Into<U>` are not. A declared return type is the same reduction: unconstrained `T`, extra
  bounds, a tuple, and a missing `->` record nothing.
- `typed_bindings` and `call_bindings` are bounded to 64 rows per declaration, with
  `omitted_typed_bindings` / `omitted_call_bindings` disclosing how many further rows the source
  stated.
- Trait supertraits are linked only when they resolve to an in-snapshot trait declaration; external
  traits and ambiguous names remain unresolved.
- Rust syntax errors fail extraction for the affected file.
- Alternative definitions with the same qualified name, such as platform-specific `cfg` functions, are coalesced and retain evidence for every definition.

`AxumExtractor` deliberately does not infer:

- dynamic/non-literal route paths;
- chained `MethodRouter` expressions such as `get(a).post(b)`;
- closures or generic handler expressions;
- `any`, `on`, `route_service`, `nest`, `merge`, fallback handlers, or macro-generated routes;
- request/response schemas, authentication, middleware, state, extractors, or handler implementation links.

Those are separate framework slices and should be added only when exact repository evidence justifies them.

## Test

```bash
cargo test -p athanor-extractor-rust --locked
cargo test -p athanor-runtime-defaults --test axum_registry --locked
```
