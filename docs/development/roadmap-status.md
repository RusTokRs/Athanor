---
id: doc://docs/development/roadmap-status.md
kind: developer_guide
language: en
source_language: en
status: active
---
# Roadmap Status

This compact ledger describes source state, including labelled branch-local work, but is not execution evidence.
Detailed evidence lives in `athanor_implementation_plan_ru.md`; long-range work lives in `start.md`.

## Status Rules

- **Implemented** means code, documentation, and source regressions are present.
- **Verified** means the required format/build/test/Clippy/smoke/security/feature/coverage matrix
  succeeded on one exact source commit.
- Documentation metadata is not execution evidence.

## Current Architecture

### Explicit Runtime Composition

Application services receive `RuntimeComposition`; Store, search, projector, extractor, and transport
factories are selected by `ath`, `athd`, or MCP composition roots. Process-global installers are gone.
Adapter plugin admission pins launch digests to the user trust record (program plus regular files named
in `args`), rejects duplicate adapter ids and symlink manifests, streams payload hashes, kills oversized
external processes immediately, and honors `[adapters].disabled` plus configurable process limits.

### Bounded Owners And Publication

Indexing uses bounded phases and staged publication. Documentation generation uses strict request,
manifest, outline, context, citation, draft, validation, and current-pointer contracts.

The architecture profile loads one exact committed canonical snapshot through the configured Store,
applies hard limits, emits cited Markdown/Mermaid output, and publishes immutable checksum-bound
generations. Its CLI and validated inspection are execution-confirmed.

The module profile is source-implemented through Slice 2C: pure inventory, module-scoped evidence,
immutable publication, exact Store loading, CLI generation, and validated inspection.

The API profile is source-implemented through Slice 3C: endpoint/schema/example inventory, scoped facts,
supported canonical relations, open diagnostics, immutable publication, exact Store loading, CLI, and
validated inspection.

Operations documentation is source-implemented through Slice 4C. Slices 4A–4B provide bounded exact-snapshot
inventory/evidence with deterministic limits, citations, omissions, Mermaid, and SHA-256 output. Slice 4C adds
immutable `operations/index.md`, exact Store loading, CLI generation, and validated inspection.

Onboarding documentation is source-implemented and exact-execution-verified through Slice 5C. Slices 5A–5B
provide bounded documentation/package/command/environment/test-CI anchors, scoped facts/relations/open
diagnostics, shared 256-item budgeting, cited Mermaid, and fail-closed evidence. Slice 5C adds immutable
`onboarding/index.md`, exact Store loading, CLI generation, and validated inspection.

Completeness reporting is exact-execution-verified through Slices 6A–6B. The package computes canonical `file`
inventory coverage with processed/unprocessed paths, per-language basis-point coverage, named adapter
contribution, deterministic limits/omissions, exact Store loading, read-only CLI, cancellation/drain, and
versioned JSON transport.

Framework projections are exact-execution-verified through Slices 7A–7C. Bounded Next.js, Axum, and Express
route projections remain framework-specific while the base JS/TS and Rust extractors stay framework-neutral.
Schema/auth/middleware, route composition, and handler linking remain deferred.

Slice 8A adds bounded PowerShell `$env:NAME` / `${env:NAME}` references through the existing redacted
env://<NAME> / EnvVarUsed contract. Exact self-evaluation `33676558603` on `75562e19…` confirms `ps1 = 2/2`.

Slice 8B recognizes only root `athanor.toml` as first-party runtime config, reusing redacted `Feature` /
`SymbolDefined`. Exact self-evaluation `33680051765` on `e219157a…` confirms 665/718 (`9261` bps), TOML 34/35.

Slice 8C recognizes `.github/actions/**/action.yml|yaml` only when `runs.using: composite`, projecting bounded
composite `ScriptCommand`, `run`/`uses` steps, and redacted step environment declarations. Exact self-evaluation
`33712810332` on `5d195069…` confirms 667/719 (`9276` bps), YAML 11/15.

Slice 8D recognizes only first-party `.github/dependabot.yml|yaml` version-2 update policies through existing
`Feature` / `SymbolDefined` contracts. Exact self-evaluation `33714498524` on `424da862…` confirms 669/720
(`9291` bps), TOML 34/35 and YAML 12/15. Post-merge Verification Matrix `33714498496` exposed test-only rustfmt
hunks in Slices 8C/8D; #91 applied exactly those formatter changes.

Slice 8E recognizes only root `deny.toml` cargo-deny policy domains `advisories`, `licenses`, `bans`, and
`sources`. It records selected scalar enforcement modes plus list counts without raw advisory/license/source
lists or generic TOML expansion. Exact self-evaluation `33722472453` on `c10ab848e…` is green: 670/720
processed (`9305` bps), TOML 35/35, YAML 12/15, 9051 facts, 13208 relations, 169 diagnostics. The artifact
contains all four `config://deny.toml#cargo-deny:<section>` keys. Post-merge CI `33722472454` passed formatting
and workspace tests before exposing one `clippy::collapsible_if`; #93 applied exactly the requested let-chain,
and PR CI `33723374660` confirmed formatting, workspace tests, and Clippy green on macOS before merge.

### Slice 8F adds bounded first-party runtime artifact projections for the root `install.sh` and
`scripts/verify_release_version.py`.

### Slice 8G adds a bounded first-party projection for the repository-root `install.ps1`: the Windows installer
entry point, `ath.exe`/`athd.exe`, `SHA256SUMS`, and `Get-FileHash` verification as one `ScriptCommand` with
contract anchors and source evidence. Runtime paths, environment values, filesystem state, and generic
PowerShell semantics remain out of scope, and nothing is executed.

### Slice 8H adds a bounded first-party projection for single-level `.github/ISSUE_TEMPLATE/*.yml|yaml` files.
It records form identity and bounded body-field metadata with evidence, rejects nested paths and empty form
names, accepts only native `input`, `textarea`, `dropdown`, `checkboxes`, and `markdown` body kinds, and rejects
mixed unsupported forms atomically.

### Slice 8I adds a bounded first-party projection for agent-tool MCP configuration.

Slice 8I recognizes only the first-party root `.codex.json` (Codex CLI `mcp.servers`) and
`antigravity.json` (`mcpServers`) agent-tool MCP configuration files. It records one `ScriptCommand` per
configured MCP server with the agent tool, server name, and literal command plus argument tokens through
the existing `ScriptCommand` / `SymbolDefined` contracts with source evidence and ownership. Malformed,
partially supported, or server-less files are rejected atomically; other JSON files are not projected and
the configured commands are never executed. The slice was selected from the exact post-gate completeness
artifact on `aaf79d6…` (run `37290849626`): `688/734` processed (`9373` bps), 46 unprocessed files, and the
two agent-tool MCP configurations were the only in-policy product-semantic gap. Exact self-evaluation run
`37292220926` on the Slice 8I source commit `6fb5e34…` is green: `691/735` processed (`9401` bps),
JSON `2/35`, facts `9204`, relations `13274`, diagnostics `208`.

### Exact Evidence

Released baseline remains `609027eb02caa05346ebfea8538552c42b588c31`: CI `29995959544`, AppSec
`29995960063`, Store `29995959512`, release `29996579628`, clean-install smoke `29998347890`.

Documentation matrix evidence remains recorded for the earlier verified profiles. Later profile source landings
include module Slice 2C and API Slice 3A; these do not substitute for focused profile execution evidence.

Historical architecture and Rustok execution evidence remains recorded explicitly for the status inventory:
Slice 1C1 and Slice 1C2; both slices are retained in the roadmap ledger so the documentation status inventory can verify their exact execution evidence. source `042d02ac6b4c89d90a5b76c818098eb0c6b41920`; CI `30025932615`; AppSec `30025931953`;
Store `30025932704`. The first Rustok failure is recorded as run `30029451096` with `citation budgeting`; the
isolated probe is `30030131126`.

The exact cross-platform verification matrix on `037a28d6e037061f87c2f395764b5e6783b18ca0` is green for the
current coordinated main source: CI `34604807206`, AppSec `34604807217`, Store Conformance `34604807194`.
The CI matrix covers formatting, workspace tests, Clippy, indexing smoke, editable documentation, feature
matrix, cargo-deny, and source coverage; AppSec covers CodeQL, Zizmor, and Gitleaks. Rustok docgen evaluation
`34604807150` and probe `34604807128` are also green on the same source. This exact evidence promotes operations
Slices 4A–4C to `Verified`.

The exact onboarding focused matrix on `76f90d69975a7ae0a29463413f88f40d6b8aca83` is green across Ubuntu, Windows, and macOS in run `34644333635`, covering formatting, onboarding operation/publication/inspection inventories, and focused Clippy. This exact evidence promotes onboarding Slices 5A–5C to `Verified`.

On the same exact `76f90d69975a7ae0a29463413f88f40d6b8aca83`, completeness run `34644333634` is green across
Ubuntu, Windows, and macOS, covering profile/operation/transport/CLI inventories and focused Clippy; framework
projection run `34644333569` is green across Ubuntu, Windows, and macOS, covering Next.js/Express and Axum tests
plus focused Clippy. These exact gates promote Slices 6A–6B and 7A–7C to `Verified` on one shared source commit.

Completeness progression is exact and monotonic for selected semantic gaps: 8B `665/718` (`9261` bps),
8C `667/719` (`9276` bps), 8D `669/720` (`9291` bps), 8E `670/720` (`9305` bps). After 8E, TOML is 35/35.
The exact post-8H self-evaluation on `e246a71b…` is green at `685/731` (`9370` bps), YAML `15/16`; the only
remaining YAML gap is `crates/athanor-extractor-openapi/tests/fixtures/basic.openapi.yaml`. Generic JSON/test
fixtures, `Cargo.lock`, and non-semantic repository files remain out of scope unless separately justified.
The exact pre-8I baseline on `aaf79d6…` is `688/734` (`9373` bps) with 46 unprocessed files; the Slice 8I
self-evaluation on `6fb5e34…` is green at `691/735` (`9401` bps), JSON `2/35`. The last non-policy-excluded
gap, the typo-named `for agents. md` briefing, was fixed by renaming it to
`architecture_audit_briefing_ru.md` on `0b42c28…`: self-evaluation run `37310134250` is green at
`692/735` (`9414` bps), Markdown `151/151`, and every remaining unprocessed file is policy-excluded.

## Implemented Packages

- `COMP-003` / `COMP-003C2B2C2B`: explicit composition and bounded owners.
- `MCP-007`: cancellation-safe transactional Index publication.
- `JSON-003`: recursive, disjoint, fixture-backed contract lifecycle.
- `DOC-001` / `DOC-002`: status hygiene and bounded architecture documents.
- `MCP-004`: responsive control input under saturation.
- `VERIFY-001`: exact cross-platform CI/AppSec/Store evidence.
- `API-001`: verified GraphQL/OpenAPI request/response/security consistency.
- `REL-001`: verified immutable `v0.2.1` release and clean installs.

## Active Work

### `PARITY-001` — CodeGraph-Parity Code Intelligence

- [x] Slices P1/P2: JS/TS call graph — import bindings, static call sites, `builtin.linker.js_ts_calls`; `implements` bridges — Rust `trait_impl` facts plus JS/TS class heritage (prior source slice).
- [x] P2A–P2F verified: P2A Rust/JS/TS `Extends`, impact/ranking/profile, index v47, CI `38722056518` on `4a23498`; P2B/P2C/P2D/P2E/P2F named/star/default/direct-namespace/multi-source calls, v48–v52; CI `38742833180`/`5b5f8a1`, `38748788007`/`49444d5`, `38757025492`/`c105cad`, exact-commit `38772624382`/`6aefc28`, and `38785145039`/`a67782f` ; P2G JS/TS callback and promise-handler bridges — bounded `callback_arguments` on allowlisted combinator call sites with indirect `Calls` edges (reason `js_ts_callback_bridge`) and `TestedBy` from test locations, index state v52→v53, CI `38960635498`/`c817b6d`, AppSec `38960635435`, and Store Conformance `38960635396` passed. P2J module-resolution configuration with path-alias and workspace-package origins, index v55→v56, CI `39034523205`/`dedde51`; P2M typed receivers and deep member paths, index v58→v59, CI `39052821710`/`3244768`; P2L package `exports` subpaths, `pnpm-workspace.yaml` layouts, and merged `rootDirs` roots, index v57→v58, CI `39043475500`/`3a1dc86`; P2K anonymous default exports bound to the canonical `default` name, index v56→v57, CI `39038926472`/`5fdf6e9`; P2H default class exports and construction targets, index v53→v54, CI `38975578247`/`05455a5`; P2I multi-hop namespace aliases, index v54→v55, CI `39005812205`/`c12b7ed`;
- [x] Slice P2N verified: JS/TS typed receiver paths across modules — a receiver path reads a field's type from the module that declares its owner, so `this.repo.items.map()` resolves when `Repo.items` is declared in another file; interface property signatures join class fields as receiver-type evidence, only an exported top-level class/interface/type alias owns field types, and a name declared in two modules owns neither. CI `39059761912`/`f27019c`, AppSec `39059761908`, Store Conformance `39059761903`, and Production Gate `39059761955` passed on Rust 1.95 (ubuntu/windows/macos).
- [x] Slice P2O verified: JS/TS receivers typed by declared return types — a declaration records the return type it declares, a module records the bindings it initializes with a statically named call, and `const repo = createRepo(); repo.items.map()` resolves through `createRepo(): Repo`; only a local, imported, or re-exported factory counts, and an unannotated factory, an ambiguous binding, and a name-unique factory in an unimported module stay unresolved; index state v59→v60. CI `39064135600`/`720a6e5`, AppSec `39064135635`, Store Conformance `39064135612`, and Production Gate `39064135603` passed on Rust 1.95 (ubuntu/windows/macos).
- [x] Slice P2P verified: Rust method calls through declared receiver types — call sites record their callee, name, kind, line, and receiver expression; a function records the receiver types it states from parameters and annotated locals; `self.m()` dispatches inside its own impl at `0.8`, a concrete receiver resolves to its type's method at `0.8`, and a `&dyn Trait` or `impl Trait` receiver resolves to the trait's own method declaration at `0.7`, while a nested, unannotated, or ambiguously named receiver stays unresolved; trait method declarations become entities; index state v60→v61. CI `39071179686`/`55b75b8`, AppSec `39071179761`, Store Conformance `39071179699`, and Production Gate `39071179694` passed on Rust 1.95 (ubuntu/windows/macos).
- [x] Slice P2Q verified: Rust dynamic-dispatch candidates — a `&dyn Trait` or `impl Trait` call resolves to the trait declaration and then records one candidate edge per declared implementation of the method at `0.5`, ordered by qualified name and bounded to eight per call site with the recorded count in the payload; a concrete receiver claims no candidates and a candidate never becomes test coverage; the receiver rows a declaration records are bounded at 64 with an omission count; index state v61→v62. CI `39074951063`/`e0c64b2`, AppSec `39074951139`, Store Conformance `39074951176`, and Production Gate `39074951169` passed on Rust 1.95 (ubuntu/windows/macos).
- [x] Slice P2R verified: symbol-level incremental invalidation — a changed file invalidates the declarations that moved, not every declaration it contains; a declaration is fingerprinted by everything a linker or checker reads from it, a restated one leaves the affected set, and its relations, facts, and diagnostics are carried forward instead of being pruned with the file; no index-state bump because the stored objects are the same canonical objects. CI `39104548532`/`6cd4489`, AppSec `39104548493`, Store Conformance `39104548628`, and Production Gate `39104548491` passed on Rust 1.95 (ubuntu/windows/macos).
- [x] Slice P2S verified: JS/TS inherited members through declared heritage — a member the receiver's type does not declare itself resolves through the `extends` chain the source states, up to four hops, at confidence 0.6 below the 0.8/0.7 of a member the type declares; both ends of a hop must be class or interface declarations the snapshot states, so a base two files declare, a base no file declares, and a cyclic chain end the walk; no index-state bump. CI `39114040880`/`7e057288`, AppSec `39114040746`, Store Conformance `39114040866`, and Production Gate `39114040834` passed on Rust 1.95 (ubuntu/windows/macos).
- [x] Slice P2T verified: JS/TS element types from a declared collection — a `for...of` iteration variable and the first parameter of an array combinator's callback carry the element type of `Item[]` or `Array<Item>`, so `item.save()` resolves at confidence 0.65, below an annotation the receiver's own binding carries and above an inherited member; `for...in`, `reduce`, a nested array, a tuple, and an unannotated collection name nothing, and a collection annotation no longer records the wrapper as the binding's own type; no index-state bump. CI `39117819177`/`b1c9dc4a`, AppSec `39117819151`, Store Conformance `39117819160`, and Production Gate `39117819148` passed on Rust 1.95 (ubuntu/windows/macos).
- [x] Slice P2U verified: JS/TS awaited types from a declared promise — `const repo = await createRepo(); repo.save()` takes the type `Promise<Repo>` or `PromiseLike<Repo>` yields, and `async function load(): Repo` records that as its awaited return type rather than as a return type, since an `async` function returns a promise; an awaited receiver resolves at confidence 0.65, the tier shared with the element of a collection, and an unawaited promise names nothing; no index-state bump. CI `39136025025`/`3bd4b018`, AppSec `39136025040`, Store Conformance `39136025057`, and Production Gate `39136025000` passed on Rust 1.95 (ubuntu/windows/macos).
- [x] Slice P2V verified: JS/TS returned types from a constructed return — a declaration records the one type every `return` of its body constructs, so `function createRepo() { return new Repo(); }` types `const repo = createRepo()` at confidence 0.8 without an annotation, while an annotation still wins when the declaration has one; a return of a call, a bare return, two disagreeing returns, and a nested function's return name nothing, and an `async` declaration awaits to the type it constructs; no index-state bump. CI `39228710568`/`c8011cb9`, AppSec `39228710459`, Store Conformance `39228710456`, and Production Gate `39228710863` passed on Rust 1.95 (ubuntu/windows/macos).
- [x] Slice P2W verified: JS/TS structural types from an object literal — `const config = { logger: new Logger() }` records `config.logger` as a `Logger` under a structural type named for the binding (`config$object`), so `config.logger.flush()` and a class field's `this.deps.repo.read()` resolve at confidence 0.8 with no linker change; only a construction is evidence, and a literal that constructs nothing records no rows; no index-state bump. CI `39232772125`/`be7a943c`, AppSec `39232771441`, Store Conformance `39232771683`, and Production Gate `39232772436` passed on Rust 1.95 (ubuntu/windows/macos).
- [ ] Slice P2X implemented: JS/TS index-access element types — `items[0].save()` and `this.fields[i].save()` record as `items[].save` / `this.fields[].save` and resolve at confidence 0.65 from the collection annotation; `handlers[key]()` and nested index access stay omitted, and an untyped index does not fall through to a unique member name; index state v62→v63. Source complete; verification pending a Rust toolchain.
- [ ] Slice P2Y implemented: JS/TS array-literal element constructions — `const items = [new Item()]; items[0].save()` records `items[]` as kind `construction` and resolves at 0.8; an empty array, mixed constructed types, a nested array, a spread, and a call element name nothing; index state v63→v64. Source complete; verification pending a Rust toolchain.
- [ ] Slice P2Z implemented: JS/TS factory wrappers that return a call — `function createRepo() { return makeRepo(); }` records `returned_callee` and the linker inherits `makeRepo`'s annotation or constructed return one hop; a second hop, mixed construction and call, two callees, a member callee, an `async` wrapper, and an inner factory that yields a promise name nothing; index state v64→v65. Source complete; verification pending a Rust toolchain.
- [ ] Slice P2AA implemented: JS/TS peeled returns and awaited async wrappers — `return (new Repo())`, `return await makeRepo()`, and `const repo = await wrap()` through `async function wrap() { return makeRepo(); }` resolve; an unawaited async wrapper names nothing; index state v65→v66. Source complete; verification pending a Rust toolchain.
- [ ] Slice P2AB implemented: JS/TS type assertions — `return load() as Repo` and `const repo = load() as Repo` type a receiver at 0.7; a construction still wins, `as const` and mixed assertions name nothing; index state v66→v67. Source complete; verification pending a Rust toolchain.
- [ ] Slice P2AC implemented: JS/TS call-result receivers — `createRepo().save()`, `(await createRepo()).save()`, and `new Repo().save()` record as `createRepo().save` / `createRepo()@.save` / `Repo{}.save` and resolve from the factory's stated type or the constructor name; a member factory, a nested call result, a call of a call result, an inline assertion, and an untyped factory result stay omitted; index state v67→v68. Source complete; verification pending a Rust toolchain.
- [ ] Slice P2AD implemented: JS/TS member-factory receivers — `svc.build().save()`, `this.factory().save()`, and `createRepo().getStore().save()` record the call of the named method and resolve from its stated return type, including an inherited method; `factory()()`, an inline assertion, and an untyped member factory stay omitted; index state v68→v69. Source complete; verification pending a Rust toolchain.
- [ ] Slice P2AE implemented: JS/TS inline receiver assertions — `(value as Repo).save()`, `(<Repo>value).save()`, and `(value satisfies Repo).save()` record as `Repo#.save` and resolve at 0.7; `as const` and mixed assertions stay omitted; index state v69→v70. Source complete; verification pending a Rust toolchain.
- [ ] Slice P2AF implemented: JS/TS qualified construction receivers — `new ns.Repo().save()` records as `Repo{}.save` and resolves from the terminal constructor name; a computed constructor stays omitted; index state v70→v71. Source complete; verification pending a Rust toolchain.
- [ ] Slice P2AG implemented: JS/TS `super` calls — `super.save()` resolves to the method on the class the owner `extends` at 0.8; a `super` call without a declared base does not guess a unique member; index state v71→v72. Source complete; verification pending a Rust toolchain.
- [ ] Slice P2AH implemented: JS/TS type-alias unfolding — `type Client = Repo` types `repo.save()` as `Repo.save` one hop; a union, an array, and a function alias name nothing; index state v72→v73. Source complete; verification pending a Rust toolchain.
- [ ] Slice P2AI implemented: JS/TS constrained type parameters — `function run<T extends Repo>(repo: T)` records `repo` as `Repo`; an unconstrained parameter, a union bound, and a default without `extends` leave the parameter name; index state v73→v74. Source complete; verification pending a Rust toolchain.
- [ ] Slice P2AJ implemented: Rust constrained type parameters — `fn run<T: Reader>(item: T)` records `item` as `impl Reader`; a `where` clause, impl generic, and trait generic are the same bound; unconstrained `T`, extra bounds, and generic bounds leave the parameter name; index state v74→v75. Source complete; verification pending a Rust toolchain.
- [ ] Slice P2AK implemented: Rust type-alias unfolding — `type Client = Store` types `item.write()` as `Store::write` one hop; a generic, a tuple, a reference, and a primitive alias name nothing; index state v75→v76. Source complete; verification pending a Rust toolchain.
- [ ] Slice P2AL implemented: Rust nested `self.field` receivers — `self.reader.write()` resolves from the named field the owner struct declares; a `dyn`/`impl` field, a type-alias field, and `(&mut self.reader).write()` are the same one hop; two hops, a missing field, an ambiguous `cfg` field, and a tuple struct name nothing; index state v76→v77. Source complete; verification pending a Rust toolchain.
- [ ] Slice P2AM implemented: JS/TS constrained return types — `function create<T extends Repo>(): T` records `Repo` so `create().save()` resolves from that bound; a class/method generic and `async function wrap<T extends Repo>(): T` are the same bound; unconstrained `T`, a union bound, and a default without `extends` leave the parameter name; index state v77→v78. Source complete; verification pending a Rust toolchain.
- [ ] Slice P2AN implemented: JS/TS invoked factory results — `function create(): () => Repo` records `Repo` as `returned_function_type` so `create()().save()` and `const repo = create()()` resolve from that yield; `create().save()` does not take it; a third hop, an awaited invocation, a member factory as that extra hop, a generic function type, and an `async` factory name nothing; index state v78→v79. Source complete; verification pending a Rust toolchain.
- [ ] Slice P2AO implemented: Rust factory return types — `fn create_repo() -> Store` records `Store` so `create_repo().write()` resolves from that annotation; `-> Box<Store>` and `-> &Store` are the same type; an `impl`/`dyn` return, a type parameter, a missing `->`, a second hop, and a member factory name nothing; index state v79→v80. Source complete; verification pending a Rust toolchain.
- [ ] Slice P2AP implemented: Rust constrained factory returns — `fn wrap<T: Reader>() -> T` and `-> impl Reader` record `Reader` as `impl_trait` so `wrap().read()` dispatches to the trait method; `-> Box<dyn Reader>` is `trait_object`; unconstrained `T` and extra bounds name nothing; index state v80→v81. Source complete; verification pending a Rust toolchain.
- [ ] Slice P2AQ implemented: Rust member-factory receivers — `svc.create().write()` and `self.create().write()` resolve from the type that method declares it returns; a two-hop walk, a missing method, an untyped owner, and an untyped method name nothing; index state v81→v82. Source complete; verification pending a Rust toolchain.
- [ ] Slice P2AR implemented: Rust nested factory hops — `create_repo().get().write()`, `svc.create().get().write()`, and `self.create().get().write()` resolve from the inner method's declared return; a third hop, a field after the calls, a missing method, and an untyped inner method name nothing; index state v82→v83. Source complete; verification pending a Rust toolchain.
- [ ] Slice P2AS implemented: Rust associated-function factories — `Store::new().write()` resolves from the type `Store::new` declares it returns; a path-qualified type, a nested hop, a missing method, and an untyped associated function name nothing; index state v83→v84. Source complete; verification pending a Rust toolchain.
- [ ] Slice P2AT implemented: Rust factory-initialized bindings — `let store = create_repo(); store.write()` and `let store = Store::new(); store.write()` resolve from that factory's declared return; an annotated let still wins; a nested call, a member factory, a path-qualified callee, an untyped factory, and two disagreeing factories name nothing; index state v84→v85. Source complete; verification pending a Rust toolchain.
- [ ] Slice P2AU implemented: Rust member-factory initialized bindings — `let store = svc.create(); store.write()` and `let store = self.create(); store.write()` resolve from that method's declared return; an untyped owner and a missing method name nothing; index state v85→v86. Source complete; verification pending a Rust toolchain.
- [ ] Slice P2AV implemented: Rust crate-path factories — `crate::create_repo().write()`, `crate::app::Store::new().write()`, and `let store = crate::create_repo(); store.write()` resolve from that factory's declared return; a missing path and a nested hop name nothing; index state v86→v87. Source complete; verification pending a Rust toolchain.
- [ ] Slice P2AW implemented: Rust nested associated-factory hops — `Store::new().get().write()` resolves from the inner method's declared return; a third hop, a missing inner method, and a path-qualified nested hop name nothing; index state v87→v88. Source complete; verification pending a Rust toolchain.
- [ ] Slice P2AX implemented: Rust nested factory-initialized bindings — `let store = create_repo().get(); store.write()`, `let store = Store::new().get(); store.write()`, and `let store = svc.create().get(); store.write()` resolve from the inner method's declared return; a nested crate-path, a third hop, and a missing inner method name nothing; index state v88→v89. Source complete; verification pending a Rust toolchain.
- [ ] Slice P2AY implemented: Rust try-operator factory yields — `create_repo()?.write()`, `create_repo().get()?.write()`, and `let store = create_repo()?; store.write()` take the Ok/Some type a `Result`/`Option` factory declares; a Result factory without `?`, a `?` on a non-Result factory, and a `?` in the middle of a hop name nothing; index state v89→v90. Source complete; verification pending a Rust toolchain.
- [ ] Slice P2AZ implemented: Rust nested crate-path factory hops — `crate::create_repo().get().write()`, `crate::app::Store::new().get().write()`, and `let store = crate::create_repo().get(); store.write()` resolve from the inner method's declared return; a third hop, a missing inner method, and a relative nested path name nothing; index state v90→v91. Source complete; verification pending a Rust toolchain.
- [ ] Slice P2BA implemented: Rust inner-try factory hops — `create_repo()?.get().write()`, `svc.create()?.get().write()`, `Store::new()?.get().write()`, `crate::create_repo()?.get().write()`, and `let store = create_repo()?.get(); store.write()` type from the inner Result/Option factory then the next method; a third hop, a Result factory without inner `?`, and a `?` on a non-Result inner factory name nothing; index state v91→v92. Source complete; verification pending a Rust toolchain.
- [ ] Slice P2BB implemented: Rust unwrap/expect factory yields — `create_repo().unwrap().write()`, `create_repo().expect("failed").write()`, `create_repo().get().unwrap().write()`, and `let store = create_repo().unwrap(); store.write()` take the Ok/Some type a `Result`/`Option` factory declares; unwrap on a non-Result factory, `unwrap_or_default`, an inner unwrap hop, and `store.unwrap()` name nothing; index state v92→v93. Source complete; verification pending a Rust toolchain.
- [ ] Slice P2BC implemented: Rust inner-unwrap factory hops — `create_repo().unwrap().get().write()`, `svc.create().unwrap().get().write()`, `Store::new().unwrap().get().write()`, `crate::create_repo().unwrap().get().write()`, and `let store = create_repo().unwrap().get(); store.write()` type from the inner Result/Option factory then the next method; a third hop, unwrap on a non-Result inner factory, and `store.unwrap().get()` name nothing; index state v93→v94. Source complete; verification pending a Rust toolchain.
- [ ] Slice P2BD implemented: Rust inherent `-> Self` factory returns — `impl Store { fn new() -> Self }` records `Store` so `Store::new().write()`, `-> Box<Self>`, `-> &Self`, and `-> Result<Self, E>` type from that impl; a free-function `-> Self` and a trait or trait-impl `-> Self` name nothing; index state v94→v95. Source complete; verification pending a Rust toolchain.
- [x] Slice P3: `builtin.extractor.treesitter` plus `builtin.linker.treesitter` host and resolve receiver-qualified Python and Go packs, with a per-language coverage measurement published.
- [x] Slice P4: deterministic operations documentation — DOCGEN-001 4A–4C; the monitoring category derives declared health/metrics endpoints and telemetry variable names, with probes/alerts still open.
- [x] Slices P5/P5b: bounded `explore` MCP query — ranked entries, one-hop relations, citations, budgets; P5b added `explore` multi-hop code-flow chains, aggregate blast radius, exact `target` anchor, and `ath explore` over `athanor.explore.v1`; CI `38869503550`/`c48ca69`, AppSec `38869503667`, and Store Conformance `38869503725` passed on Rust 1.95 (ubuntu/windows/macos).
- [ ] Slice P5c implemented: `explore` inlines bounded declaration excerpts — each entry and path target carries the cited source slice, clipped to 32 lines and 4,096 bytes; absolute paths, `..` components, missing files, and non-UTF-8 contents are counted in `omitted.source_unread`; schema stays `athanor.explore.v1` additively. Source complete; verification pending a Rust toolchain.
- Historical: pre-P2A P1/P2/P5 run `38152033388` (`191c65e`, not this worktree); P3 `38157055780` (`e07547e`), P4b `38160162759` (`69d8918`), and P3 linker `38165153178` (`09ce6ab`) passed with zero-line rustfmt diffs and successful or skipped checks.

### `DOCGEN-001` — Evidence-Backed Documentation Generation

- [x] Slices 0A–1C: contracts, architecture profile, immutable publication, exact Store, CLI/inspection;
- [x] repaired aggregate citation selection, exact Rustok evaluation, and relation disclosure;
- [x] Slices 2A–5C: module/API/operations/onboarding source surfaces and publication flows;
- [x] Slices 6A–6B: pure completeness plus exact Store/read-only CLI/versioned JSON transport;
- [x] Slices 7A–7C: bounded built-in Next.js, Axum, and Express route projections;
- [x] Slices 8A–8E source implementation plus exact completeness confirmation;
- [x] Slice 8F source implementation for `install.sh` and `verify_release_version.py`;
- [x] Slice 8G source implementation for the first-party `install.ps1` installer and Slice 8H source implementation and hardening for bounded GitHub Issue Forms;
- [x] focused exact verification for Slices 8F–8H on `e246a71bcb094cb3553bf6770954a4617dc1a376`;
- [x] focused execution verification for Slices 2A–2C on `00d649d43117c842f24f806d45ee2fdb4ad56aa7`;
- [x] focused execution verification for Slices 3A–3C on `1a7da6817f162ae26ad9500ca94408e14f8ae94a`;
- [x] focused execution verification for Slices 4A–4C on `037a28d6e037061f87c2f395764b5e6783b18ca0`;
- [x] focused execution verification for Slices 5A–5C, 6A–6B, and 7A–7C on `76f90d69975a7ae0a29463413f88f40d6b8aca83`.
- [x] Slice 8I source implementation for first-party agent-tool MCP configuration, selected from the exact post-gate artifact on `aaf79d6…`;

`DOCGEN-001` remains in progress. Slices 2A–2C, 3A–3C, 4A–4C, 5A–5C, 6A–6B, 7A–7C, and 8F–8H are promoted to `Verified` by exact-commit execution matrices. In-policy coverage is now complete: after the
`0b42c28…` briefing rename, every remaining unprocessed file is policy-excluded, so further semantic gaps require a separately justified product scope rather than generic fixture coverage.

The existing coordinated `ath generate` command remains unchanged.

## Product Backlog

- [x] run the focused verification for Slices 8F–8H and rerun exact completeness/self-evaluation;
- [x] run exact cross-platform execution verification for Slices 2A–2C on one `main` commit;
- [x] run exact cross-platform execution verification for Slices 3A–3C on one exact `main` source commit;
- [x] run exact cross-platform execution verification for Slices 4A–4C on one exact `main` source commit;
- [x] run exact cross-platform execution verification for Slices 5A–5C on one exact `main` source commit;
- [x] run exact cross-platform execution verification for Slices 6A–6B on one exact `main` source commit;
- [x] run exact cross-platform execution verification for Slices 7A–7C on one exact `main` source commit;
- [x] select the next bounded semantic gap from the exact post-gate artifact: Slice 8I first-party agent-tool MCP configuration;
- [x] rename the typo-named `for agents. md` audit briefing to `architecture_audit_briefing_ru.md` (`0b42c28…`);
- [x] start the semantic retrieval slice: local hashing embeddings implement the `EmbeddingProvider`/`VectorIndex` ports, persist in a JSON sidecar beside the Tantivy index, expose `lexical`/`semantic`/`hybrid` modes in `ath search --mode` and MCP `search`, and keep only hits at or above `SEMANTIC_MIN_SIMILARITY` (0.2 cosine) so unrelated documents are not returned as hashing noise;
- [ ] keep issue forms, OpenAPI fixtures, and generic JSON parsing out unless justified by product semantics;
- [ ] keep Next.js/Axum/Express schemas/auth/middleware and route-composition expansion evidence-driven;
- Dart/Flutter remains blocked on a portable DartScope dependency boundary rather than a local-only path dependency;
- optional i18n, provider, daemon, and MCP integration after quality gates; semantic/vector retrieval is in progress (local hashing embeddings + hybrid `ath search`/MCP `search` modes).
