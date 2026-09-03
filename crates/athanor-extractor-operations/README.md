# athanor-extractor-operations

Operations source extractor adapter.

Implements: `Extractor`

## What It Emits

The current slice parses:

- dotenv-style files such as `.env.example`, `.env`, and `*.env`
- Makefile targets from `Makefile`, `makefile`, and `*.mk`
- Dockerfile stages, command instructions, and `ENV` declarations from `Dockerfile` and
  `*.Dockerfile`
- shell script functions and exported environment variables from `*.sh`, `*.bash`, and `*.zsh`
- bounded PowerShell environment references from `*.ps1`
- docker-compose services, service commands, and environment declarations from common compose
  filenames such as `docker-compose.yml`, `compose.yaml`, and `*.compose.yml`
- GitHub Actions workflow, job, and step declarations from `.github/workflows/*.yml` and
  `.github/workflows/*.yaml`
- first-party GitHub composite actions from `.github/actions/**/action.yml` and `action.yaml` when
  `runs.using: composite`, including `run`/`uses` steps and step environment declarations
- first-party Dependabot version-2 update policies from `.github/dependabot.yml` and
  `.github/dependabot.yaml`
- first-party cargo-deny supply-chain policy summaries from the root `deny.toml`
- Cargo package manifests from `Cargo.toml`
- Kubernetes YAML manifests from common deployment paths and filenames
- SQL database migrations from common migration paths and filenames
- JSON, TOML, and YAML runtime configuration files from common config/settings paths and filenames,
  plus the first-party root `athanor.toml`

Entities:

- `EntityKind::EnvVar` with `env://<NAME>` stable keys
- `EntityKind::DbMigration` for SQL migration files
- `EntityKind::DbTable` for tables declared by SQL migrations
- `EntityKind::Feature` for runtime configuration keys and bounded dependency/supply-chain policies
- `EntityKind::Package` for Cargo packages and workspaces
- `EntityKind::Dependency` for Cargo dependencies, dev-dependencies, build-dependencies,
  workspace dependencies, and target-specific dependencies
- `EntityKind::ScriptCommand` for Makefile targets and Dockerfile `RUN`, `CMD`, and `ENTRYPOINT`
  instructions
- `EntityKind::ScriptCommand` for shell function declarations
- `EntityKind::ScriptCommand` for docker-compose service `command` and `entrypoint` declarations
- `EntityKind::ScriptCommand` for GitHub Actions workflows, jobs, composite actions, `run` steps, and
  `uses` steps
- `EntityKind::ScriptCommand` for Kubernetes container `command` and `args` declarations
- `EntityKind::DockerService` for Dockerfile stages and docker-compose services
- `EntityKind::DockerService` for Kubernetes workloads, services, ConfigMaps, Secrets, and related
  manifest resources

Facts:

- `FactKind::EnvVarUsed` from the environment variable entity to the canonical file entity
- `FactKind::MigrationCreatesTable` from SQL migration entities to table entities
- `FactKind::SymbolDefined` from Cargo package, workspace, dependency, runtime-config, Dependabot,
  and cargo-deny policy entities to the canonical file entity
- `FactKind::SymbolDefined` from operational command/stage entities to the canonical file entity

Environment fact payloads mark the declaration source as `dotenv`, `dockerfile`, `shell`,
`powershell`, `docker_compose`, `github_actions`, `kubernetes`, or `runtime_config`. Raw values are
not stored, so real `.env`, Dockerfile defaults, exported shell values, PowerShell environment
references, compose environment values, workflow/composite-action environment values, Kubernetes
Secret/ConfigMap/container environment values, or runtime config values do not leak into canonical
snapshots.

## Inputs

`SourceFile` with UTF-8 text content and a supported operations project-relative path.

## Side Effects

None. The adapter does not run commands, use the network, or modify project files.

## Limitations

- Only simple dotenv `KEY=value` and `export KEY=value` declarations are parsed.
- Makefile parsing only recognizes top-level target declarations and prerequisites.
- Dockerfile parsing recognizes line-continued instructions, but does not execute shell syntax or
  interpret JSON-array command forms.
- Shell script parsing recognizes `export KEY=value`, `readonly KEY=value`, `name() {`,
  `function name {`, and `function name() {`; it does not parse command invocations, sourced files,
  control flow, traps, or here-documents.
- PowerShell parsing is intentionally lexical and limited to simple `$env:NAME` and `${env:NAME}`
  references in `*.ps1`. It does not parse cmdlets, functions, assignments, control flow, AST
  semantics, or inline comment/string boundaries, and it never stores referenced environment values.
- docker-compose parsing is limited to the top-level `services` map, service `image`, `build`,
  `command`, `entrypoint`, and `environment` declarations. It does not resolve `env_file`, profiles,
  includes, extends, anchors, volume semantics, healthchecks, dependencies, or networks.
- GitHub Actions workflow parsing is limited to workflow name, top-level `env`, jobs, job `runs-on`,
  job `env`, and step `run`, `uses`, and `env` declarations. It does not evaluate expressions,
  permissions, matrices, reusable workflows, service containers, caches, artifacts, or secrets.
- GitHub composite-action parsing is limited to first-party `.github/actions/**/action.yml|yaml`
  metadata with `runs.using: composite`, plus `run`/`uses` steps and step `env`. Inputs/outputs
  expression semantics, JavaScript/Docker actions, permissions, secrets, and generic YAML remain
  outside this slice.
- Dependabot parsing is limited to first-party `.github/dependabot.yml|yaml`, requires `version: 2`,
  and projects only `updates[]` entries with `package-ecosystem`, `directory`, plus optional
  `schedule.interval` and `target-branch`. Registries/credentials, groups, ignore/allow rules,
  labels, reviewers, commit-message behavior, and generic YAML semantics remain outside this slice.
- cargo-deny parsing is limited to the root `deny.toml` and the `advisories`, `licenses`, `bans`, and
  `sources` tables. It records selected scalar enforcement modes plus list counts only; advisory IDs,
  license allowlists/exceptions, registry URLs, git sources, and arbitrary TOML fields are not copied
  into canonical payloads.
- Cargo manifest parsing is limited to package/workspace metadata and direct dependency sections.
  It does not resolve inherited workspace fields, features, target expressions, patches,
  replacements, profiles, or build scripts.
- Kubernetes parsing is limited to YAML documents with `kind` and `metadata.name`. It recognizes
  container images, container `command`/`args`, container `env`, and ConfigMap/Secret `data` keys,
  but it does not evaluate Helm/Kustomize templates, `envFrom`, projected volumes, probes,
  selectors, RBAC semantics, or rollout strategy.
- SQL migration parsing recognizes simple `CREATE TABLE [IF NOT EXISTS] [schema.]table`
  statements. It does not parse quoted dotted identifiers, column definitions, constraints,
  `ALTER TABLE`, views, indexes, triggers, functions, down migrations, or ORM-specific migration
  metadata.
- Runtime configuration parsing flattens scalar JSON, TOML, and YAML keys into redacted
  configuration knowledge. Root-level recognition is intentionally bounded to known runtime
  configuration names such as `athanor.toml`; `deny.toml` is handled separately by its cargo-deny
  policy projection rather than generic runtime-config parsing. It does not interpret framework-specific
  config schemas, environment interpolation, includes/imports, profiles, encrypted values, or arrays
  of objects.
- Variable interpolation, shell command substitution, multiline values, and comments inside quoted
  values are not interpreted.
- runbooks remain separate Phase 5 work.

## Test

```bash
cargo test -p athanor-extractor-operations
```
