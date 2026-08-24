---
id: doc://docs/adapters/extractor-express.md
kind: adapter
language: en
source_language: en
status: active
---
# Express Route Extractor

Crate: `athanor-extractor-js-ts`

Port: `Extractor`

Built-in id: `builtin.extractor.express`

## Purpose

`ExpressExtractor` is a framework-specific companion to the framework-neutral `JsTsExtractor`.
It projects a deliberately small set of static Express route declarations into evidence-backed
canonical knowledge without changing the shared API taxonomy or parsing framework behavior itself.

## Supported Input

The extractor accepts JavaScript, JSX, TypeScript, and TSX source-language hints already supported by
the JS/TS adapter. Source content must reference `express`, parse cleanly through the existing
tree-sitter backend, and establish an Express application/router receiver through an explicit import
or `require("express")` binding.

Supported receiver construction includes:

```text
import express from "express";
const app = express();

import { Router } from "express";
const router = Router();

import { Router as ExpressRouter } from "express";
const router = ExpressRouter();

const express = require("express");
const app = express();

const { Router: MakeRouter } = require("express");
const router = MakeRouter();
```

Supported route calls are exactly two-argument static forms:

```text
app.get("/health", health)
router.post("/users", handlers.create)
```

Methods: `GET`, `POST`, `PUT`, `DELETE`, `PATCH`, `HEAD`, `OPTIONS`.

## Emitted Knowledge

Each accepted route emits:

- `EntityKind::Other("express_route")`
- `FactKind::RouteDeclared`
- source evidence for the call expression
- source-file ownership
- payload fields for framework, receiver, method, literal route path, handler path, and normalized source path

Stable key example:

```text
express-route://src/http/server.ts#app:GET:/health:health
```

The adapter-scoped entity kind is intentional. A static Express declaration is useful framework
knowledge, but this bounded slice does not claim OpenAPI/API-schema equivalence and therefore does not
emit `EntityKind::ApiEndpoint`.

## Rejected / Deferred Forms

The extractor fails closed rather than guessing for:

- dynamic route paths
- extra middleware arguments
- inline closures or anonymous handlers
- computed receiver/method access
- `app.use` / `app.all`
- `router.route(...)` and chained route builders
- mounted or nested prefix composition
- middleware/auth/state/schema inference
- handler implementation linking
- files with tree-sitter parse errors

Those surfaces require separate evidence-backed slices rather than broad heuristics.

## Side Effects

None. The adapter runs in process, uses no network or commands, and does not modify project files.

## Verification

```bash
cargo test -p athanor-extractor-js-ts --locked
cargo test -p athanor-runtime-defaults --test express_registry --locked
```

Focused execution evidence is recorded separately from source implementation status.
