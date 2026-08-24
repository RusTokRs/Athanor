# athanor-extractor-rust

Rust source-code extractor adapters.

Implements: `Extractor`

The crate exposes two extractor types. `RustExtractor` owns framework-neutral Rust symbols and environment-variable usage. `AxumExtractor` is a separate bounded framework projection for static Axum route declarations and is registered independently as `builtin.extractor.axum`.

## What It Emits

`RustExtractor` emits:

- `EntityKind::Module` for inline module declarations
- `EntityKind::Function` for free functions and implementation methods
- `EntityKind::Symbol` for structs, enums, traits, unions, type aliases, constants, and statics
- `FactKind::SymbolDefined` from every emitted symbol to its canonical file entity
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
- Functions declared inside traits are not emitted yet.
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
