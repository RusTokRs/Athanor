---
id: doc://docs/adapters/extractor-nextjs.md
kind: adapter
language: en
source_language: en
status: active
---
# Next.js Route Extractor

Crate: `athanor-extractor-js-ts`

Type: `NextJsExtractor`

Port: `Extractor`

Built-in id: `builtin.extractor.nextjs`

## Purpose

The Next.js extractor adds a bounded framework-specific projection beside the framework-neutral
`JsTsExtractor`. It derives route identity only from standard Next.js filesystem conventions, so
ordinary JavaScript/TypeScript parsing remains owned by the base JS/TS extractor.

## Inputs

The adapter recognizes project-relative JavaScript/TypeScript route files under either the repository
root or `src/`:

```text
app/**/page.{js,jsx,ts,tsx}
app/**/route.{js,jsx,ts,tsx}
pages/**/*.{js,jsx,ts,tsx}
```

For App Router paths, route groups such as `(marketing)` and parallel-route folders such as `@slot`
do not contribute URL segments. Private `_...` directories and interception-route notation are left
out of this bounded slice. Pages Router `_app`, `_document`, and `_error` files are not routes.

## Emitted Knowledge

Each recognized route file emits:

- one `EntityKind::Feature` with `feature_kind = "nextjs_route"`;
- one `FactKind::RouteDeclared` from that feature to the canonical file entity;
- framework, router (`app` or `pages`), route kind, normalized route pattern, dynamic-route flag,
  and normalized source path metadata.

The feature stable key is source-backed:

```text
feature://nextjs:<project-relative-source-path>#route
```

This keeps conflicting files distinct while exposing the framework route in payload metadata.
Dynamic Next.js segments such as `[id]` are preserved rather than rewritten.

## Evidence And Ownership

The route fact includes full-file source evidence from the `nextjs` extractor and ownership of the
route source file. Windows separators are normalized only for stable-key/payload identity; canonical
source evidence retains the source path supplied by the source provider.

## Side Effects

None. The adapter does not execute project code, run commands, access the network, publish artifacts,
or read a latest snapshot.

## Boundaries

This slice deliberately does not:

- infer React component semantics;
- parse Next.js configuration or custom `pageExtensions`;
- infer HTTP methods from App Router route handlers;
- infer request/response schemas or authentication;
- model layouts, loading/error boundaries, metadata, middleware, rewrites, redirects, or server actions;
- resolve interception routes or parallel-route slot semantics beyond omitting `@slot` from the URL;
- replace the base JS/TS extractor.

Those are separate framework slices and should be added only when exact repository evidence justifies
them.

## Verification

```bash
cargo test -p athanor-extractor-js-ts --locked
cargo test -p athanor-runtime-defaults --test nextjs_registry --locked
```
