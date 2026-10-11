# athanor-linker-treesitter

Cross-file resolution for the tree-sitter language packs: module imports, the static call graph,
and test coverage bridges for Python and Go.

Implements: `Linker`

Adapter id: `builtin.linker.treesitter`

## What It Emits

`TreeSitterLinker` emits:

- `RelationKind::Imports` from a module to an existing module when a recorded import statement
  resolves inside the snapshot.
- `RelationKind::Calls` from a calling declaration to the declaration it reaches, when the call site
  is statically named and resolution has evidence.
- `RelationKind::TestedBy` from a tested declaration to a declaration in a test location that calls
  it.

Import relations are verified when the match is an exact module path and inferred when an import
path is matched by the longest package-directory suffix. Call and test relations are always inferred,
because resolution is name-based evidence rather than a linker proof. Every relation is owned by both
source files and records the resolution reason and the confidence of the step that matched it.

## Language Layouts

Resolution is driven by the language id on each stable key and by the import `style` the extractor
recorded, so a new language pack receives cross-file resolution without a new crate:

| Language | Unit | Module layout | Own instance | Test convention |
| --- | --- | --- | --- | --- |
| `python` | File | Dotted module paths, `.py`/`.pyi` files, `__init__.py`/`__init__.pyi` packages | `self` | `test_*.py`, `*_test.py`, test directories |
| `go` | Directory | Package directories matched by the longest import-path suffix | — | `*_test.go` |

A pack id without a layout entry keeps resolving through the file layout and the generic test
directory convention.

## Import Resolution

`dotted` and `from` statements resolve through dotted module paths. Relative imports count their
leading dots, walk up from the importing module's directory, and try source files before package
files, in a fixed order, so output is deterministic. `spec` statements resolve import paths by the
longest matching package-directory suffix and skip standard-library paths, which stay unresolved
because they leave the repository.

## Call Resolution

The call linker consumes the bounded `calls` arrays recorded per declaration and resolves each call
site in this order:

1. `same_unit_own_receiver` — `self.method()` inside `Class.method` resolves to the sibling member.
2. `same_unit_exact_name` — an exact qualified-name match inside the caller's resolution unit, which
   is the file for Python and the whole package directory for Go.
3. `imported_exact_name` / `imported_symbol_name` — the callee head matches an import binding of the
   calling module, and the remainder matches a declaration of the resolved target unit.
4. `same_unit_member_name` — a member call with an unknown receiver resolves only when the unit
   contains exactly one member with that name.
5. `unique_exported_name` — a bare call with no local or imported target resolves only when exactly
   one exported declaration in the snapshot carries that name.

Steps 4 and 5 require a unique candidate, so ambiguity stays unresolved rather than becoming a
guessed relation. Confidence runs from `0.9` for an exact unit match down to `0.5` for a unique name
match. Relations are sorted by id and capped at 10,000 per snapshot.

## Coverage Measurement

`resolution_coverage` measures recorded references against resolved ones per language. The
denominator is the evidence the extractor recorded: import statements and call sites inside
declaration bodies. Module-level call sites are counted separately, because no declaration owns them,
so they stay out of the denominator instead of silently lowering it. The crate's corpus test pins the
exact numbers for Python and Go.

## Side Effects

None. The linker reads canonical entities and writes relations. It does not read files, run commands,
use the network, or modify project files.

## Limitations

- Package-style and standard-library import paths that leave the repository stay unresolved.
- Wildcard (`from x import *`) bindings, dynamic imports, reflective dispatch, computed call targets,
  and receiver typing are not resolved.
- Go method calls whose receiver variable is not resolvable resolve through the unique member-name
  step or stay unresolved; the receiver type recorded on the declaration is not used to type
  variables.
- Go methods are identified by their bare name (`symbol://go:<path>#Serve` with `receiver: "Server"`),
  so same-named methods of different types in one file collide on the stable key and the extractor
  discloses the second as a duplicate declaration.

## Test

```bash
cargo test -p athanor-linker-treesitter
```
