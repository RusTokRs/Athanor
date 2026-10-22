# Changelog

All notable user-visible changes to Athanor are recorded in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and release versions use
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). The top `[Unreleased]` section collects
changes after the latest frozen candidate. A release version must have exactly one section with an ISO
`YYYY-MM-DD` date and at least one substantive note before its matching `v<version>` tag may be
published. HTML comments, thematic breaks, empty list markers, and empty fenced code blocks are not
substantive release notes.

## [Unreleased]

### Added

- Add Rust trait-supertrait and JS/TS class/interface `Extends`, preserving Rust/JS/TS `Implements`; bump index state v46→v47.
- Add the bounded `explore` MCP tool: one query returns ranked entries, citations, and one-hop relations.
- Extend `explore` with bounded multi-hop relation chains and an aggregate blast radius (`--depth`/`--paths`, default two hops and 16 chains, every truncation disclosed), let it anchor on an exact stable key or repository path through `target`, and expose the same answer as `ath explore` so one call replaces a query loop.
- Add `builtin.extractor.treesitter` over Python and Go: one data-driven engine records declarations, receiver-qualified methods and other declarations, imports, exports, and statically named call sites.
- Add `builtin.linker.treesitter`: Python and Go imports, cross-file calls through package units, receiver variables, and import bindings, test bridges, and a per-language coverage measurement.
- Add a monitoring operations category: the operations profile derives declared health, status, and metrics endpoints plus telemetry variable names from canonical evidence; probes, alerts, dashboards, and SLOs stay absent without an evidence source.
- Add the JavaScript/TypeScript call graph: the extractor records each import statement's local bindings (`default_name`, `namespace_name`, `named`) and a bounded `calls` array per declaration with the statically named callee, receiver, kind, argument count, and line/column, and `builtin.linker.js_ts_calls` resolves those into evidence-backed `Calls` relations and emits `TestedBy` from test locations.
- Preserve JS/TS aliases; resolve bounded named/default and direct-namespace calls plus uniquely aggregated multi-source wildcards, exclude wildcard `default`, and bump index v47→v52.
- Add JS/TS callback and promise-handler bridges: combinator call sites record bounded `callback_arguments` that the call linker resolves into indirect evidence-backed `Calls` edges; index state bumps v52→v53.
- Add strict versioned request, outline, context, citation, draft, manifest, and validation-report
  contracts for documentation generation: exact snapshot binding, portable paths, stable-key/evidence traceability, cited diagram edges, metrics, and checksums.
- Add a minimal fixture repository and Rustok evaluation corpus with expected sections, citation paths, diagram edges, known gaps, repeatability, unsupported-relation disclosure, and review criteria.
- Add a deterministic architecture profile that converts one explicit canonical snapshot into cited Markdown, relation-backed Mermaid, omission disclosure, footnotes, metrics, and checksums.
- Add an application-layer immutable documentation publisher with staged generation directories, checksum-bound artifacts, an atomic `documentation/current.json` pointer, and exact `UpToDate` reuse.
- Add `ath docs generate-architecture` for one exact committed snapshot with hard-limit flags, `--force`, text/JSON reports, and Ctrl-C cancellation; the existing coordinated `ath generate` remains unchanged.
- Add validated `ath docs architecture current`, `manifest`, and `validation` inspection commands that reject pointer escape, identity drift, unsupported artifact layouts, and checksum mismatch.
- Add `overview` and `capabilities` MCP tools exposing the repository overview and the bounded analysis-completeness report to MCP clients.
- Add a bounded first-party projection for agent-tool MCP configuration: `.codex.json` and `antigravity.json` project one `ScriptCommand` per server, with evidence and ownership.
- Add JS/TS default class export binding: `export default class Foo {}` records its default name and a default-imported or same-module `new Foo()` resolves to the class; index state bumps v53→v54.
- Add JS/TS multi-hop namespace aliases: a namespace name forwarded across modules by named re-exports resolves `alias.member()`; index state bumps v54→v55.
- Add local semantic retrieval with zero new dependencies: hashing-trick embeddings over character trigrams and word n-grams (FNV-1a buckets) power `ath search --mode semantic|hybrid` and MCP `search`.
- Add first-party JavaScript/TypeScript module-resolution configuration: `tsconfig.json`, `jsconfig.json`, and their named `.json` variants project their `baseUrl`, `paths` mappings, and inherited `extends` with disclosed bounds and JSONC tolerance, and `package.json` records `main` and `workspaces`.
- Resolve JS/TS non-relative specifiers in the import and call linkers through that evidence, so a path alias such as `@/lib/api` and a workspace-package specifier such as `@scope/ui/button` become real module and call edges while unprovable specifiers stay unresolved; index state bumps v55→v56.
- Add JavaScript/TypeScript anonymous default export binding: `export default function () {}`, `export default class {}`, and `export default () => {}` are recorded under the canonical `default` name with `anonymous_default` disclosure, so a default import, a default-imported construction, and a forwarded `default` alias resolve to them; index state bumps v56→v57.
- Resolve JavaScript/TypeScript package `exports` subpaths and conditional exports (longest declared prefix first, unknown conditions skipped, undeclared subpaths unreachable), read workspace membership from `pnpm-workspace.yaml`, and retry relative imports in the other roots of `rootDirs`; index state bumps v57→v58.
- Invalidate the declarations that moved instead of every declaration a changed file contains: a declaration is fingerprinted by everything a linker or checker reads from it, a restated one leaves the affected set, and the relations, facts, and diagnostics it already had are carried forward instead of being pruned with the file.
- Resolve JavaScript/TypeScript calls whose receiver is only identifiable through its declared type: `new Type()` initializers, annotated class fields, and TypeScript parameter properties become module-level receiver-type evidence, so `service.run()` and `this.repo.items.map()` resolve when the path types prove the member, while ambiguous, cyclic, or over-bound receivers stay unresolved; index state bumps v58→v59 and earlier snapshots reindex.
- Extend JavaScript/TypeScript typed receivers across modules: a receiver path reads a field's type from the module that declares its owner, so `this.repo.items.map()` resolves when `Repo.items` is declared in another file; interface property signatures join class fields as receiver-type evidence, and only an exported top-level class, interface, or type alias owns field types, so a name declared in two modules owns neither.
- Resolve JavaScript/TypeScript calls on a receiver produced by a factory: a declaration records the return type it declares, a module records the bindings it initializes with a call, and `repo.items.map()` resolves through `createRepo(): Repo` while an unannotated factory, a name bound to two factories, and a name-unique factory in an unimported module stay unresolved; index state bumps v59→v60 and earlier snapshots reindex.
- Resolve Rust method calls through the receiver's declared type: call sites record their callee, name, kind, line, and receiver expression, a function records the receiver types it states from parameters and annotated locals, and `store.write()`, `self.write()`, and `reader.read()` on a `&dyn Trait` or `impl Trait` receiver resolve to the declaring type's or trait's method while a nested, unannotated, or ambiguously named receiver stays unresolved; index state bumps v60→v61 and earlier snapshots reindex.
- Record the implementations a Rust dynamic call can reach: a `&dyn Trait` or `impl Trait` call resolves to the trait declaration and then records one bounded candidate edge per declared implementation of the method, so `reader.read()` names both `Reader::read` and every `read` the snapshot implements; a concrete receiver claims no candidates, and a candidate never becomes test coverage.
- Bump index state v61→v62 because Rust call relations gained dynamic-dispatch candidates; earlier snapshots reindex.
- Resolve JavaScript/TypeScript calls to a member the receiver's type inherits: the `extends` chain the source states is walked up to four hops, so `service.reset()` resolves to `Base.reset` when `Service extends Base`, at confidence 0.6 below the 0.8/0.7 of a member the receiver's own type declares; a base two files declare, a base no file declares, and a cyclic chain end the walk.
- Type a JavaScript/TypeScript receiver by the element type a collection annotation states: a `for...of` iteration variable and the first parameter of an array combinator's callback carry the element type of `Item[]` or `Array<Item>`, so `item.save()` resolves at confidence 0.65; `for...in`, `reduce`, a nested array, a tuple, and a collection the file never annotates name nothing, and a collection annotation no longer records the wrapper as the binding's own type.
- Type a JavaScript/TypeScript receiver by the value an awaited promise yields: `const repo = await createRepo(); repo.save()` takes the type `Promise<Repo>` or `PromiseLike<Repo>` resolves to, and `async function load(): Repo` now records that type as its awaited return type rather than as a return type, since an `async` function returns a promise; an awaited receiver resolves at confidence 0.65 and an unawaited promise names nothing.
- Type a JavaScript/TypeScript receiver by the type a factory constructs and returns: a declaration records the one type every `return` of its body builds, so `function createRepo() { return new Repo(); }` types `const repo = createRepo()` at confidence 0.8 without an annotation, while a return of a call, a bare return, two disagreeing returns, and a nested function's return name nothing.
- Type a JavaScript/TypeScript receiver by the members an object literal constructs: `const config = { logger: new Logger() }` records `config.logger` as a `Logger` under a structural type named for the binding, so `config.logger.flush()` and a class field's `this.deps.repo.read()` resolve at confidence 0.8; a property whose value is a literal, a call, or a shorthand names nothing.
- Type a JavaScript/TypeScript receiver by the element of an indexed collection: `items[0].save()` and `this.fields[i].save()` record as `items[].save` / `this.fields[].save` and resolve at confidence 0.65 from the collection annotation (`Item[]` / `Array<Item>`); a computed call of the element itself (`handlers[key]()`), nested index access, and an untyped index stay unresolved instead of guessing a unique member name; index state bumps v62→v63 and earlier snapshots reindex.
- Inline bounded declaration excerpts in `explore`: one call returns the cited source slice of each entry and path target (clipped to 32 lines and 4,096 bytes) with the call path and blast radius; a missing, escaped, or non-UTF-8 file is counted in `omitted.source_unread` instead of dumping the file.
- Type a JavaScript/TypeScript indexed receiver by the element an array literal constructs: `const items = [new Item()]; items[0].save()` records `items[]` as a `Item` construction and resolves at confidence 0.8; an empty array, a mixed constructed type, a nested array, a spread, and a call element name nothing; index state bumps v63→v64 and earlier snapshots reindex.
- Type a JavaScript/TypeScript receiver through a factory that only returns another factory's call: `function createRepo() { return makeRepo(); }` records `returned_callee` and the linker reads `makeRepo`'s annotation or constructed return one hop, so `const repo = createRepo(); repo.save()` resolves; a second hop, a mix of construction and call, two callees, a member callee, an `async` wrapper, and an inner factory that yields a promise name nothing; index state bumps v64→v65 and earlier snapshots reindex.
- Peel parentheses, `await`, `as`, and non-null assertions on a returned construction or call, and type a receiver the module awaits through an `async` wrapper: `return (new Repo())`, `return await makeRepo()`, and `const repo = await wrap()` where `async function wrap() { return makeRepo(); }` resolve; an unawaited async wrapper still holds a promise and names nothing; index state bumps v65→v66 and earlier snapshots reindex.
- Type a JavaScript/TypeScript receiver by a type assertion the source writes: `return load() as Repo` and `const repo = load() as Repo` record `Repo` as annotation evidence at confidence 0.7; a construction still wins, `as const` and two disagreeing assertions name nothing; index state bumps v66→v67 and earlier snapshots reindex.
- Type a JavaScript/TypeScript member call whose receiver is a factory or construction the source names: `createRepo().save()`, `(await createRepo()).save()`, and `new Repo().save()` record as `createRepo().save` / `createRepo()@.save` / `Repo{}.save` and resolve from the factory's stated type or the constructor name; a member factory, a nested call result, a call of a call result, an inline assertion, and an untyped factory result stay unresolved instead of guessing a unique member name; index state bumps v67→v68 and earlier snapshots reindex.
- Type a JavaScript/TypeScript member call through a member factory or nested identifier-rooted call: `svc.build().save()`, `this.factory().save()`, and `createRepo().getStore().save()` resolve from the method's stated return type, including a method the receiver's type inherits; a call of a call result, an inline assertion, and an untyped member factory stay unresolved; index state bumps v68→v69 and earlier snapshots reindex.
- Type a JavaScript/TypeScript member call whose receiver is an inline type assertion: `(value as Repo).save()`, `(<Repo>value).save()`, and `(value satisfies Repo).save()` record as `Repo#.save` and resolve at annotation confidence 0.7; `as const` and mixed assertions stay omitted, and an assertion of a type with no such member does not fall through to a unique name; index state bumps v69→v70 and earlier snapshots reindex.
- Type a JavaScript/TypeScript member call on a qualified construction: `new ns.Repo().save()` records as `Repo{}.save` and resolves from the terminal constructor name at construction confidence 0.8, the same as `new Repo().save()`; a computed constructor (`new ns[key].Repo()`) stays omitted; index state bumps v70→v71 and earlier snapshots reindex.
- Resolve a JavaScript/TypeScript `super.method()` call through the class the owner `extends` at construction confidence 0.8; a `super` call without a declared base does not fall through to a unique member name; index state bumps v71→v72 and earlier snapshots reindex.
- Unfold a JavaScript/TypeScript type alias one hop: `type Client = Repo` types `repo.save()` as `Repo.save`; a union, an array, and a function alias name nothing; index state bumps v72→v73 and earlier snapshots reindex.
- Type a JavaScript/TypeScript receiver by a constrained type parameter: `function run<T extends Repo>(repo: T)` records `repo` as `Repo` and resolves `repo.save()` from that bound; an unconstrained parameter, a union bound, and a default without `extends` leave the parameter name; index state bumps v73→v74 and earlier snapshots reindex.
- Type a Rust receiver by a constrained type parameter: `fn run<T: Reader>(item: T)` records `item` as `impl Reader` so `item.read()` dispatches to the trait method at 0.7; a `where T: Reader` clause, an impl generic, and a trait generic are the same bound; unconstrained `T`, `T: Reader + Send`, and `T: Into<U>` leave the parameter name; index state bumps v74→v75 and earlier snapshots reindex.
- Unfold a Rust type alias one hop: `type Client = Store` types `item.write()` as `Store::write`; a generic, a tuple, a reference, and a primitive alias name nothing; index state bumps v75→v76 and earlier snapshots reindex.
- Walk a Rust method call through a named struct field: `self.reader.write()` resolves from the field `App` declares; a `dyn`/`impl` field, a type-alias field, and `(&mut self.reader).write()` are the same one hop; two hops, a missing field, an ambiguous `cfg` field, and a tuple struct name nothing; index state bumps v76→v77 and earlier snapshots reindex.
- Type a JavaScript/TypeScript factory return by a constrained type parameter: `function create<T extends Repo>(): T` records `Repo` so `create().save()` resolves from that bound; a class/method generic and `async function wrap<T extends Repo>(): T` are the same bound; unconstrained `T`, a union bound, and a default without `extends` leave the parameter name; index state bumps v77→v78 and earlier snapshots reindex.
- Type a JavaScript/TypeScript call of a factory result: `function create(): () => Repo` records `Repo` as `returned_function_type` so `create()().save()` and `const repo = create()()` resolve from that yield; `create().save()` does not take it; a third hop, an awaited invocation, a member factory as that extra hop, a generic function type, and an `async` factory name nothing; index state bumps v78→v79 and earlier snapshots reindex.
- Type a Rust method call on a factory result: `fn create_repo() -> Store` records `Store` so `create_repo().write()` resolves from that annotation; `-> Box<Store>` and `-> &Store` are the same type; an `impl`/`dyn` return, a type parameter, a missing `->`, a second hop, and a member factory name nothing; index state bumps v79→v80 and earlier snapshots reindex.
- Type a Rust factory return by a constrained type parameter or trait bound: `fn wrap<T: Reader>() -> T` and `fn implied() -> impl Reader` record `Reader` as `impl_trait` so `wrap().read()` dispatches to the trait method at 0.7; `-> Box<dyn Reader>` is `trait_object`; unconstrained `T`, extra bounds, and a missing `->` name nothing; index state bumps v80→v81 and earlier snapshots reindex.
- Type a Rust method call on a member factory result: `svc.create().write()` and `self.create().write()` resolve from the type that method declares it returns; a two-hop walk, a missing method, an untyped owner, and an untyped method name nothing; index state bumps v81→v82 and earlier snapshots reindex.
- Type a Rust method call through one extra factory hop: `create_repo().get().write()`, `svc.create().get().write()`, and `self.create().get().write()` resolve from the inner method's declared return; a third hop, a field after the calls, a missing method, and an untyped inner method name nothing; index state bumps v82→v83 and earlier snapshots reindex.
- Type a Rust method call on an associated-function factory result: `Store::new().write()` resolves from the type `Store::new` declares it returns; a path-qualified type, a nested hop, a missing method, and an untyped associated function name nothing; index state bumps v83→v84 and earlier snapshots reindex.
- Type a Rust receiver initialized by a factory call: `let store = create_repo(); store.write()` and `let store = Store::new(); store.write()` resolve from that factory's declared return; an annotated let still wins; a nested call, a member factory, a path-qualified callee, an untyped factory, and two disagreeing factories name nothing; index state bumps v84→v85 and earlier snapshots reindex.
- Type a Rust receiver initialized by a member factory: `let store = svc.create(); store.write()` and `let store = self.create(); store.write()` resolve from that method's declared return; an untyped owner and a missing method name nothing; index state bumps v85→v86 and earlier snapshots reindex.
- Type a Rust method call on a `crate::` / `self::` / `super::` factory path: `crate::create_repo().write()`, `crate::app::Store::new().write()`, and `let store = crate::create_repo(); store.write()` resolve from that factory's declared return; a missing path and a nested hop name nothing; index state bumps v86→v87 and earlier snapshots reindex.
- Type a Rust method call through one extra associated-factory hop: `Store::new().get().write()` resolves from the inner method's declared return; a third hop, a missing inner method, and a path-qualified nested hop name nothing; index state bumps v87→v88 and earlier snapshots reindex.
- Type a Rust receiver initialized by a nested factory hop: `let store = create_repo().get(); store.write()`, `let store = Store::new().get(); store.write()`, and `let store = svc.create().get(); store.write()` resolve from the inner method's declared return; a nested crate-path, a third hop, and a missing inner method name nothing; index state bumps v88→v89 and earlier snapshots reindex.
- Type a Rust receiver through a trailing `?`: `create_repo()?.write()`, `create_repo().get()?.write()`, and `let store = create_repo()?; store.write()` take the Ok/Some type a `Result`/`Option` factory declares; a `Result` factory without `?`, a `?` on a non-Result factory, and a `?` in the middle of a hop name nothing; index state bumps v89→v90 and earlier snapshots reindex.
- Type a Rust method call through one extra crate-path factory hop: `crate::create_repo().get().write()`, `crate::app::Store::new().get().write()`, and `let store = crate::create_repo().get(); store.write()` resolve from the inner method's declared return; a third hop, a missing inner method, and a relative nested path name nothing; index state bumps v90→v91 and earlier snapshots reindex.
- Type a Rust method call through a `?` on the inner factory hop: `create_repo()?.get().write()`, `svc.create()?.get().write()`, `Store::new()?.get().write()`, `crate::create_repo()?.get().write()`, and `let store = create_repo()?.get(); store.write()` type from the inner Result/Option factory then the next method; a third hop, a Result factory without inner `?`, and a `?` on a non-Result inner factory name nothing; index state bumps v91→v92 and earlier snapshots reindex.
- Type a Rust method call through trailing `.unwrap()` / `.expect(...)`: `create_repo().unwrap().write()`, `create_repo().expect("failed").write()`, `create_repo().get().unwrap().write()`, and `let store = create_repo().unwrap(); store.write()` take the Ok/Some type a `Result`/`Option` factory declares; unwrap on a non-Result factory, `unwrap_or_default`, an inner unwrap hop, and `store.unwrap()` name nothing; index state bumps v92→v93 and earlier snapshots reindex.
- Type a Rust method call through a `.unwrap()` / `.expect(...)` on the inner factory hop: `create_repo().unwrap().get().write()`, `svc.create().unwrap().get().write()`, `Store::new().unwrap().get().write()`, `crate::create_repo().unwrap().get().write()`, and `let store = create_repo().unwrap().get(); store.write()` type from the inner Result/Option factory then the next method; a third hop, unwrap on a non-Result inner factory, and `store.unwrap().get()` name nothing; index state bumps v93→v94 and earlier snapshots reindex.
- Type a Rust method call through an inherent `-> Self`: `impl Store { fn new() -> Self }` records `Store` so `Store::new().write()`, `svc.wrap().write()`, `-> Box<Self>`, `-> &Self`, and `-> Result<Self, E>` type from that impl; a free-function `-> Self` and a trait or trait-impl `-> Self` name nothing; index state bumps v94→v95 and earlier snapshots reindex.

### Fixed

- Stop generated module/API/operations/onboarding documents from claiming shipped publication, Store loading, CLI, and transport integration; headers name the deferred scope and its limits.
- Reject Windows drive paths, reserved device names, trailing-dot/space components, and
  case-insensitive output-path collisions before documentation artifacts can be published.
- Reject raw-file/secret context access, out-of-context citations and diagram endpoints, uncited factual
  claims, malformed evidence ranges, provider metrics without opt-in, and inconsistent valid reports;
  reject tampered manifests/artifacts as up to date and rebuild instead of reusing them.
- Reject missing or uncommitted documentation snapshot IDs instead of falling back to the latest visible
  snapshot, and preserve the previous current pointer when CLI generation is cancelled.
- Restore the bounded snapshot-search module after the semantic-search slice: mode dispatch and
  hybrid ranking move to `search/execution.rs`, and `athanor-search-tantivy` compiles again with
  the declared `serde` dependency, score-ordered `TopDocs`, and a 0.2 cosine-similarity floor.

## [0.2.1] - 2026-07-23

### Fixed

- Package the actual `*.cdx.json` files emitted by `cargo-cyclonedx 0.5.9` instead of searching only for
  `bom.json`.
- Validate that the workspace SBOM inventory is non-empty and that every packaged document is a
  non-empty CycloneDX JSON object before signing or attesting the archive.

### Release

- Preserve `v0.2.0` as an immutable failed publication attempt: its release contract and Linux artifacts
  passed, but the SBOM inventory gate stopped publication before a GitHub Release was created.

## [0.2.0] - 2026-07-22

### Added

- Local-first `ath` CLI and `athd` daemon for indexing and querying repository knowledge.
- Explicit runtime composition for stores, search, projectors, extractors, and transports.
- Transactional index publication with cancellation-safe rollback and durable post-commit success.
- Memory, JSONL, and SurrealDB storage backends with shared conformance coverage.
- Rust, JavaScript/TypeScript, Markdown, OpenAPI, GraphQL, and Rustok-oriented extraction and linking.
- MCP transport with bounded request admission, responsive cancellation, and typed index reports.
- Cross-protocol OpenAPI/GraphQL consistency checks for requests, responses, status policy,
  authentication families, and permission scopes.
- Evidence-backed documentation lifecycle, JSON contract inventory, and repository indexing smoke tests.
- Cross-platform CI on Linux, macOS, and Windows with security, feature, coverage, installer, AppSec,
  Store Conformance, and Production Gate workflows.
- Release archives for Linux and Windows with checksums, Sigstore bundles, provenance attestations,
  and CycloneDX SBOMs.

### Changed

- Rename the root architecture audit briefing from `for agents. md` to `architecture_audit_briefing_ru.md`
  so the first-party Markdown briefing is named and indexed like the other root planning artifacts.
- Release tags are gated by repository-owned package-version, changelog, artifact, signature, provenance,
  SBOM, and release-note checks.
- Release preparation rejects occupied tag names and pins an annotated tag to the exact verified candidate
  SHA instead of assuming the current `main` HEAD is still the candidate.

### Security

- Exact commit statuses are published for the CI matrix, AppSec, and Store Conformance workflows.
- Release workflows use pinned actions, least-privilege job permissions, signed assets, and provenance.
- Release publication rejects malformed or impossible calendar dates, duplicate version sections,
  decorative-only notes, and empty fenced blocks before artifact jobs run.

## [0.1.0] - 2026-06-25

### Added

- Historical development snapshot establishing the initial `ath` and `athd` package versions.
