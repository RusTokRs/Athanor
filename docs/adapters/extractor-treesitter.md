---
id: doc://docs/adapters/extractor-treesitter.md
kind: adapter
language: en
source_language: en
status: active
---
# Python And Go Tree-Sitter Extractor

Crate: `athanor-extractor-treesitter`

Type: `TreeSitterExtractor`

Port: `Extractor`

Built-in id: `builtin.extractor.treesitter`

## Purpose

One engine hosts every grammar the crate knows. A language is a data pack — node kinds, field names,
and the mapping from matched nodes onto canonical entity kinds — so a new language is a pack module
plus a fixture rather than a new crate or a new adapter id. Python (`.py`, `.pyi`) and Go (`.go`)
ship today.

## Inputs

| Pack | Extensions | Declarations | Calls | Imports | Exports |
| --- | --- | --- | --- | --- | --- |
| `python` | `py`, `pyi` | `function_definition`, `class_definition`; `decorated_definition` is unwrapped and a function inside a class body is a method | `call` over `attribute` chains | `import a.b`, `import a.b as c`, `from a.b import c as d` | `__all__ = [...]` |
| `go` | `go` | `function_declaration`, `method_declaration`, `type_declaration` → `type_spec` classified by `struct_type`/`interface_type` | `call_expression` over `selector_expression` chains | `import f "path"` specs | leading upper-case letter |

Paths are selected by lower-case extension, so `api/client.PY` and `types/stubs.pyi` are Python and
`cmd/server.go` is Go. A file whose extension no pack owns produces no output.

## Emitted Knowledge

Each supported file emits one `EntityKind::Module` and one `EntityKind::Function`/`Class`/`Symbol`
per recorded declaration:

```text
module://python:api/client.py
symbol://python:api/client.py#Client.send
symbol://go:api/server.go#Server.Serve
```

- The module payload records `language_hint`, `parser`, `imports`, `exports`, module-level `calls`,
  and `declaration_count`.
- A declaration payload records `symbol_kind` (`function`, `method`, `class`, `struct`, `interface`,
  `type`), `qualified_name`, `exported`, `async`, `source_node`, and its `calls`; a Go method is
  identified by its receiver type (`Server.Serve`) and records the receiver type plus, when it is
  written, the receiver variable (`receiver_binding`), so same-named methods of different types in one
  file stay distinct.
- Every declaration emits one `FactKind::SymbolDefined` whose value carries `stable_key`, `path`, and
  `symbol_kind`; the module is the fact subject for itself and the object of each declaration fact.
- Ownership covers the source file, and every fact and diagnostic carries line evidence.

## Call Sites

A call site is recorded only when the callee is statically nameable: an identifier, or a member chain
rooted in an identifier. The payload carries `callee`, `name`, `receiver`, `kind`, `arg_count`, and
the callee's line and column. Subscripts (`handlers[key]()`), call results, parenthesized
expressions, generic instantiations, and function literals are omitted rather than guessed, so a
recorded call set is always a lower bound that the reader can audit.

Call sites belong to the declaration body that contains them. A nested declaration owns its own call
sites, and module-level statements are recorded in the module payload. `builtin.linker.treesitter`
resolves these records into cross-file `calls` relations; this adapter resolves nothing across files.

## Import Records

Python records one payload per import statement with `style`, `module`, `names`, `aliases`, `alias`,
and `line`; wildcard imports record `*`, and `from a import b as c` records `b` with the alias map
`{"b": "c"}`. Go records one payload per `import_spec` with `style = "spec"`, the unquoted module
path, the local `alias`, `stdlib` (true when the first path segment carries no dot), and `line`.
Import records are evidence for the linkers; this adapter resolves nothing across files.

## Bounds

```text
declarations per file            512
call sites per declaration       256
call sites per file             4096
imports per file                256
exported names per file         256
parse-error diagnostics          32
duplicate diagnostics             8
traversal depth                 256
```

Every bound that clips a collection is disclosed as
`DiagnosticKind::Other("<language>_truncation")` with the affected `collection` and its `limit`, so a
count derived from the payload is always a lower bound. A redefinition that would share a stable key
is recorded once and disclosed as `DiagnosticKind::Other("<language>_duplicate_declaration")`.

## Parser Errors

A file whose parse contains an error or missing node still contributes the declarations and calls the
grammar could build, plus `DiagnosticKind::Other("<language>_parse_error")` naming the parser, the
damaged node kind, its line, and whether the node was missing. The damaged region is never
interpreted, and the diagnostic is coalesced per line and node kind. When the grammar flags a file
without an error or missing child of its own, one file-level finding is recorded with
`file_level = true` instead of dropping the disclosure.

## Evidence And Ownership

Facts and diagnostics carry parser-derived line evidence from the `treesitter` extractor and
ownership of the source file. Stable keys are scoped to the project-relative path, so the same
declaration name in two files never collides.

## Side Effects

None. The adapter runs in-process, executes no project code or commands, uses no network access, and
reads no Store snapshot itself.

## Boundaries

This slice deliberately does not infer:

- cross-file symbol resolution; import records are inputs for a separate linker slice;
- type annotations, decorators, generic parameters, fields, or interface method signatures;
- Python fallback imports and conditional redefinitions beyond the duplicate disclosure;
- Go package-level variables, constants, and function literals assigned to variables;
- framework semantics for FastAPI, Django, Gin, Echo, or any other web framework.

Go type aliases (`type A = B`) stay out of the graph rather than being reported as declarations of a
type they merely rename.

## Verification

```bash
cargo test -p athanor-extractor-treesitter --locked
cargo test -p athanor-runtime-defaults --locked
```
