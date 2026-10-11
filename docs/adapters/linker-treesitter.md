---
id: doc://docs/adapters/linker-treesitter.md
kind: adapter
language: en
source_language: en
status: active
---
# Python And Go Tree-Sitter Linker

Crate: `athanor-linker-treesitter`

Type: `TreeSitterLinker`

Port: `Linker`

Adapter id: `builtin.linker.treesitter`

The linker turns the canonical payloads of `athanor-extractor-treesitter` into cross-file relations:
`imports` between module entities, `calls` from a calling declaration to the declaration it reaches,
and `tested_by` bridges from a tested declaration to the test that calls it. It reads entities from the
current snapshot and never touches the filesystem.

## Language Layouts

Resolution is pack-driven rather than language-conditional. The language id on each stable key
selects the module layout, the own-instance keyword, and the test convention; the import `style` the
extractor recorded selects dotted or package-path semantics, so a new language pack receives
cross-file resolution without a new crate.

| Language | Unit | Module layout | Own instance | Test convention |
| --- | --- | --- | --- | --- |
| `python` | File | Dotted module paths; `.py`/`.pyi` files before `__init__.py`/`__init__.pyi` packages | `self` | `test_*.py`, `*_test.py`, `tests`/`test` directories |
| `go` | Directory | Package directories matched by the longest import-path suffix | — | `*_test.go` |

A pack id without a layout entry keeps resolving through the file layout and the generic test
directory convention, so an unknown grammar keeps its references instead of dropping them silently.

## Import Resolution

`dotted` and `from` statements resolve through dotted module paths: relative imports count their
leading dots, walk up from the importing module's directory, and try source files before package
files in a fixed order, so output is deterministic. `spec` statements resolve import paths by the
longest matching package-directory suffix and skip standard-library paths, which leave the
repository.

### Emitted Knowledge

Each resolved statement emits a `RelationKind::Imports` relation from the importing module to the
target module. An exact dotted or relative module path is `verified` at confidence `1.0` with the
resolution reason `dotted_module_path` or `relative_module_path`; a package path matched by suffix
is `inferred` at `0.8` with the reason `package_suffix`, because the module prefix is not known from
the snapshot alone. Evidence covers the import line and the target module range, and ownership is the
union of both source files. One statement resolves to at most 32 modules.

## Call Resolution

The linker consumes the bounded `calls` arrays recorded per declaration and resolves each call site
in this order, from the strongest evidence to the weakest:

1. `same_unit_own_receiver` — a member called on the language's own instance resolves to the sibling
   member (`self.encode()` inside `Client.send`).
2. `same_unit_exact_name` — an exact qualified-name match inside the caller's resolution unit, which
   is the file for Python and the whole package directory for Go, so a Go call into another file of
   the same package resolves without an import.
3. `imported_exact_name` / `imported_symbol_name` — the callee head matches an import binding of the
   calling module (module, alias, or imported name) and the remainder matches a declaration of the
   resolved target unit.
4. `same_unit_member_name` — a member call with an unknown receiver resolves only when the unit
   contains exactly one member with that name.
5. `unique_exported_name` — a bare call with no local or imported target resolves only when exactly
   one exported declaration in the snapshot carries that name.

Steps 4 and 5 require a unique candidate, so ambiguity stays unresolved rather than becoming a
guessed relation. Confidence reflects the step: `0.9` for the own-receiver and exact-unit steps,
`0.85` for an imported exact name, `0.7` for an imported name, and `0.5` for a unique name. Every
call relation is `inferred`, because name-based resolution cannot prove that a rebuilt program
dispatches to the same declaration.

### Emitted Knowledge

Each resolved call emits a `RelationKind::Calls` relation from the calling declaration to the target
declaration. The payload records the matched `callee`, `callee_kind`, `resolution` reason, and the
call-site `line` and `column`; evidence covers the call-site file and line plus the target
declaration range, and ownership is the union of both source files.

A call from a test location into a non-test declaration additionally emits `RelationKind::TestedBy`
from the tested declaration to the test declaration at confidence `0.6`.

Relations are sorted by id, deduplicated, and capped at 10,000 per snapshot, so output is bounded and
reproducible for one snapshot. Relations are emitted only when either endpoint is in the affected
subset, which keeps incremental updates proportional to the change.

## Coverage Measurement

`resolution_coverage` publishes a per-language measurement of recorded references against resolved
ones: modules, declarations, import statements, call sites inside declaration bodies, and module-level
call sites. The denominator is the evidence the extractor recorded, and module-level call sites are
counted separately because no declaration owns them, so they stay out of the denominator instead of
silently lowering it. The crate's corpus test pins the exact numbers.

| Language | Declarations | Import statements | Resolved imports | Call sites | Resolved calls | Basis points |
| --- | --- | --- | --- | --- | --- | --- |
| `python` | 4 | 3 | 3 | 4 | 3 | 7,500 |
| `go` | 7 | 2 | 1 | 4 | 4 | 10,000 |

The corpus covers a Python package with a relative-file import chain, an imported binding, a test
that calls into the package, a Go package with a cross-file call inside the same package, an imported
package call, and a standard-library import that stays unresolved.

## Boundaries

Package-manager and standard-library paths that leave the repository, wildcard (`from x import *`)
bindings, dynamic imports, reflective dispatch, computed call targets, and receiver-typed dispatch
are not resolved. Go method calls whose receiver variable is not resolvable reach either the unique
member-name step or no relation at all; the receiver type recorded on the declaration is not used to
type variables.

Go methods are also identified by their bare name: the extractor records `func (s *Server) Serve()`
as `symbol://go:<path>#Serve` with the receiver type in the payload, so two same-named methods of
different types in one file collide on the stable key and the second one is disclosed as a duplicate
declaration rather than silently overwriting the first. Qualifying Go method identities by receiver
type is the next bounded improvement for this axis.

## Verification

```bash
cargo test -p athanor-linker-treesitter --locked
```
