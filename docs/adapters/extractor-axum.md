---
id: doc://docs/adapters/extractor-axum.md
kind: adapter
language: en
source_language: en
status: active
---
# Axum Route Extractor

Crate: `athanor-extractor-rust`

Type: `AxumExtractor`

Port: `Extractor`

Built-in id: `builtin.extractor.axum`

## Purpose

The Axum extractor adds a bounded framework-specific projection beside the framework-neutral `RustExtractor`. It recognizes only high-confidence static Axum `.route(...)` declarations backed by `axum::routing` method constructors.

## Inputs

The adapter accepts Rust `SourceFile` values that reference `axum`. The current slice recognizes literal routes whose method router is a single supported constructor:

```rust
use axum::routing::{get, post};

Router::new()
    .route("/users", get(list_users))
    .route("/users", post(create_user));
```

Supported constructors are `get`, `post`, `put`, `delete`, `patch`, `head`, `options`, and `trace`. Direct, renamed, namespace, and fully-qualified `axum::routing` imports are accepted.

## Emitted Knowledge

Each recognized route emits:

- one adapter-scoped `EntityKind::Other("axum_route")` entity;
- one `FactKind::RouteDeclared` fact from the route entity to the canonical file entity;
- framework, HTTP method, literal route path, handler path, and normalized source path metadata.

The adapter-scoped entity kind avoids pretending that code routing is already an OpenAPI/GraphQL `ApiEndpoint`. It also prevents generic documentation profiles from treating an Axum route as one of their own anchors. The named `axum` evidence still contributes to exact completeness reporting.

Stable keys are source-backed:

```text
axum-route://src/http/routes.rs#GET:/users:list_users
```

## Evidence And Ownership

The route fact carries parser-derived line evidence from the `axum` extractor and ownership of the Rust source file. Windows separators are normalized for stable-key and payload identity; evidence retains the source-provider path.

## Side Effects

None. The adapter runs in-process, executes no project code or commands, uses no network access, publishes no artifacts, and reads no Store snapshot itself.

## Boundaries

This slice deliberately does not infer:

- dynamic/non-literal route paths;
- chained `MethodRouter` values such as `get(a).post(b)`;
- closures or generic handler expressions;
- `any`, `on`, `route_service`, `nest`, `merge`, fallback handlers, or macro-generated routes;
- request/response schemas, auth, middleware, state, extractors, or handler implementation relations.

The base `RustExtractor` remains the owner of framework-neutral Rust syntax. Further Axum semantics should be added only as separate evidence-backed slices.

## Verification

```bash
cargo test -p athanor-extractor-rust --locked
cargo test -p athanor-runtime-defaults --test axum_registry --locked
```
