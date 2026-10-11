# athanor-extractor-treesitter

Generic tree-sitter extractor adapter: one engine, one pack per language.

Implements: `Extractor`

`TreeSitterExtractor` hosts every grammar the engine knows and is registered as
`builtin.extractor.treesitter`. A language is a data pack — node kinds, field names, and the mapping
from matched nodes onto canonical entity kinds — so adding a language is a pack module plus a fixture
rather than a new crate or a new adapter id. The packs that ship today are Python (`.py`, `.pyi`) and
Go (`.go`).

## What It Emits

- `EntityKind::Module` for every supported source file, with `parser`, `language_hint`, `imports`,
  `exports`, module-level `calls`, and `declaration_count` in the payload
- `EntityKind::Function` for functions, and for methods with `symbol_kind = "method"`
- `EntityKind::Class` for Python classes and Go structs (`symbol_kind = "class"` / `"struct"`)
- `EntityKind::Symbol` for Go interfaces and other named types
- `FactKind::SymbolDefined` for the module and every emitted declaration
- `DiagnosticKind::Other("<language>_parse_error")` when the grammar reports an error or missing node
- `DiagnosticKind::Other("<language>_duplicate_declaration")` when a later declaration shares a
  stable key with an earlier one, which is recorded once
- `DiagnosticKind::Other("<language>_truncation")` when a bound below clipped the result

Every entity carries ownership for its source file; every fact and diagnostic carries source
evidence with the exact line range.

## Packs

| Pack | Extensions | Declarations | Calls | Imports | Exports |
| --- | --- | --- | --- | --- | --- |
| `python` | `py`, `pyi` | `function_definition`, `class_definition` (decorated definitions are unwrapped, a function inside a class is a method) | `call` over `attribute` chains | `import a.b`, `from a.b import c as d` | `__all__ = [...]` |
| `go` | `go` | `function_declaration`, `method_declaration`, `type_declaration` → `type_spec` classified by `struct_type`/`interface_type` | `call_expression` over `selector_expression` chains | `import f "path"` specs | a leading upper-case letter |

## Stable Keys

```text
module://python:api/client.py
symbol://python:api/client.py#Client.send
module://go:api/server.go
symbol://go:api/server.go#Server.Serve
```

## Bounds

Payloads stay bounded and every bound is disclosed:

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

## Semantics

- Only statically named callees are recorded. Identifier and member chains rooted in an identifier
  are recorded as dotted callee paths; subscripts, call results, parenthesized and generic
  instantiations, and function literals are omitted rather than guessed.
- Call sites belong to the declaration body that contains them; nested declarations own their own
  call sites, so a closure inside a function contributes to the enclosing declaration.
- Declaration facts record `stable_key`, `path`, and `symbol_kind`, and the module fact records the
  module as the object of every declaration fact.
- Python `__all__` lists the exported names; a Go declaration is exported when its name starts with
  an upper-case letter. The `exported` payload field is a property of the pack, never inferred from
  usage.
- Go `type_alias` declarations (`type A = B`) stay out of the graph; a `type_spec` whose type node a
  pack does not classify is recorded as `symbol_kind = "type"` instead of being mislabelled.
- A Go method is identified by its receiver type (`Server.Close`), so same-named methods of different
  types in one file stay distinct entities; the receiver variable is recorded as `receiver_binding`
  when it is written.
- A file whose parse contains errors still contributes the declarations and calls the grammar could
  build, plus a parse-error diagnostic; the damaged region is never interpreted.

## Side Effects

None. The extractor does not run commands, use the network, or modify project files.

## Limitations

- Cross-file resolution is not part of this adapter: module payloads carry the import records the
  linkers need, and resolution for these languages is a separate slice.
- Type annotations, decorators, generic parameters, fields, and interface method signatures are not
  projected into entities or facts.
- Python `try`/`except` fallback imports and conditional redefinitions are recorded verbatim; a
  redefinition of the same qualified name is recorded once and disclosed as a diagnostic.
- Go package-level variables, constants, and function literals assigned to variables are not
  declarations of this pack.
- Framework projections (for example FastAPI, Django, Gin, or Echo) are not part of this crate.

## Test

```bash
cargo test -p athanor-extractor-treesitter
```
