---
id: doc://docs/adapters/extractor-operations.md
kind: module_documentation
language: en
last_verified_snapshot: snap_jsonl_00000261
source_language: en
status: verified
---
# Operations Extractor

Crate: `athanor-extractor-operations`

Port: `Extractor`

The operations extractor reads operational configuration files and emits canonical operations
knowledge. The current slice parses dotenv-style files, Cargo package manifests, Makefile targets,
Dockerfile stages, commands and environment declarations, shell script functions plus exported
environment variables, docker-compose services, commands, and environment declarations, GitHub
Actions workflow jobs, steps, actions, and environment declarations, Kubernetes YAML deployment
manifests, SQL database migrations, and runtime configuration files.

## Inputs

Reads `SourceFile` values whose project-relative file name matches one of these supported
operations files and whose content is UTF-8 text:

- `.env`, `.env.example`, `*.env`, and `*.env.example`
- `Cargo.toml`
- `Makefile`, `makefile`, and `*.mk`
- `Dockerfile` and `*.Dockerfile`
- `*.sh`, `*.bash`, and `*.zsh`
- `docker-compose.yml`, `docker-compose.yaml`, `compose.yml`, `compose.yaml`, `*.compose.yml`,
  and `*.compose.yaml`
- `.github/workflows/*.yml` and `.github/workflows/*.yaml`
- Kubernetes-style YAML files in `k8s/`, `kubernetes/`, `deploy/`, or `deployments/`, and common
  manifest filenames such as `deployment.yaml`, `service.yaml`, `configmap.yaml`, and `secret.yaml`
- SQL migration files in `migrations/`, `db/`, `sqlx/`, `diesel/`, or `prisma/`, plus migration-like
  `.sql` filenames
- JSON, TOML, or YAML config files in `config/`, `configs/`, or `settings/`, plus common
  `config.*`, `settings.*`, `appsettings.*`, and `*.config.*` filenames

Supported dotenv declarations:

```text
KEY=value
export KEY=value
```

Supported Makefile declarations:

```text
target: prerequisite
```

Supported Cargo manifest declarations:

```toml
[package]
name = "example"
version = "0.1.0"

[workspace]
members = ["crates/example"]

[dependencies]
serde = "1"
tokio = { version = "1", features = ["macros"] }
local = { path = "../local" }
```

Supported Dockerfile declarations:

```text
FROM image AS stage
ENV KEY=value
RUN command
CMD command
ENTRYPOINT command
```

Supported shell script declarations:

```text
export KEY=value
readonly KEY=value
name() {
function name {
function name() {
```

Supported docker-compose declarations:

```yaml
services:
  web:
    image: example/web
    build: .
    command: ["ath", "serve"]
    entrypoint: ./entrypoint.sh
    environment:
      KEY: value
      OTHER_KEY:
```

Supported GitHub Actions declarations:

```yaml
name: CI
env:
  KEY: value
jobs:
  quality:
    runs-on: ubuntu-latest
    env:
      OTHER_KEY: value
    steps:
      - uses: actions/checkout@v7
      - run: cargo test
        env:
          TEST_KEY:
```

Supported Kubernetes declarations:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: api
spec:
  template:
    spec:
      containers:
        - name: web
          image: example/api:latest
          command: ["ath", "serve"]
          env:
            - name: DATABASE_URL
              valueFrom:
              secretKeyRef:
                  name: api-secret
                  key: database-url
```

Supported SQL migration declarations:

```sql
CREATE TABLE IF NOT EXISTS public.users (
  id uuid primary key
);
```

Supported runtime configuration declarations:

```toml
[server]
port = 8080
DATABASE_URL = "postgres://example"
```

## Outputs

Entities:

- `EntityKind::EnvVar` with stable keys like `env://DATABASE_URL`
- `EntityKind::DbMigration` for SQL migration files
- `EntityKind::DbTable` for tables declared by SQL migrations
- `EntityKind::Feature` for runtime configuration keys
- `EntityKind::Package` for Cargo packages and workspaces
- `EntityKind::Dependency` for Cargo dependencies, dev-dependencies, build-dependencies,
  workspace dependencies, and target-specific dependencies
- `EntityKind::ScriptCommand` for Makefile targets and Dockerfile command instructions
- `EntityKind::ScriptCommand` for shell script functions
- `EntityKind::ScriptCommand` for docker-compose service `command` and `entrypoint` declarations
- `EntityKind::ScriptCommand` for GitHub Actions workflows, jobs, `run` steps, and `uses` steps
- `EntityKind::ScriptCommand` for Kubernetes container `command` and `args` declarations
- `EntityKind::DockerService` for Dockerfile stages and docker-compose services
- `EntityKind::DockerService` for Kubernetes workloads, services, ConfigMaps, Secrets, and related
  manifest resources

Facts:

- `FactKind::EnvVarUsed` with `mechanism = "dotenv"` and `source_kind = "dotenv"`
- `FactKind::EnvVarUsed` with `mechanism = "dockerfile"` and `source_kind = "dockerfile"`
- `FactKind::EnvVarUsed` with `mechanism = "shell"` and `source_kind = "shell"`
- `FactKind::EnvVarUsed` with `mechanism = "docker_compose"` and `source_kind = "docker_compose"`
- `FactKind::EnvVarUsed` with `mechanism = "github_actions"` and `source_kind = "github_actions"`
- `FactKind::EnvVarUsed` with `mechanism = "kubernetes"` and `source_kind = "kubernetes"`
- `FactKind::EnvVarUsed` with `mechanism = "runtime_config"` and `source_kind = "runtime_config"`
- `FactKind::MigrationCreatesTable` from SQL migration entities to table entities
- `FactKind::SymbolDefined` for Cargo packages, workspaces, and dependencies
- `FactKind::SymbolDefined` for Makefile targets, Dockerfile stages, Dockerfile command
  instructions, shell functions, docker-compose services or service commands, and GitHub Actions
  workflow/job/step declarations

The adapter records whether an environment default value was present, but it does not store raw
values. This prevents accidental secret leakage from real `.env` files or Dockerfile defaults into
canonical snapshots.

## Evidence And Ownership

Every emitted entity and fact is owned by the source file. Facts include line evidence for the
declaration.

## Commands And Network

- Does not run external commands.
- Does not use the network.
- Does not modify project files.

## Limitations

- Does not interpret variable interpolation or shell command substitution.
- Does not parse multiline values.
- Does not understand comments inside quoted values.
- Makefile parsing is limited to top-level target declarations and prerequisites.
- Dockerfile parsing does not execute shell syntax, resolve copied files, or infer runtime services
  beyond stages and command instructions.
- Shell script parsing is limited to exported or readonly environment declarations and function
  declarations; it does not parse command invocations, control flow, sourced files, traps, or
  here-documents.
- docker-compose parsing is limited to the top-level `services` map, service `image`, `build`,
  `command`, `entrypoint`, and `environment` declarations. It does not resolve `env_file`, profiles,
  includes, extends, anchors, volume semantics, healthchecks, dependencies, or networks.
- GitHub Actions parsing is limited to workflow name, top-level `env`, jobs, job `runs-on`, job
  `env`, and step `run`, `uses`, and `env` declarations. It does not evaluate expressions,
  permissions, matrices, reusable workflows, service containers, caches, artifacts, or secrets.
- Cargo manifest parsing is limited to package/workspace metadata and direct dependency sections.
  It records dependency version/path/git/registry/package/optional/features metadata where present,
  but it does not resolve inherited workspace fields, target expressions, patches, replacements,
  profiles, or build scripts.
- Kubernetes parsing is limited to YAML documents with `kind` and `metadata.name`. It recognizes
  container images, container `command`/`args`, container `env`, and ConfigMap/Secret `data` keys,
  but it does not evaluate Helm/Kustomize templates, `envFrom`, projected volumes, probes,
  selectors, RBAC semantics, or rollout strategy.
- SQL migration parsing recognizes simple `CREATE TABLE [IF NOT EXISTS] [schema.]table`
  statements. It does not parse quoted dotted identifiers, column definitions, constraints,
  `ALTER TABLE`, views, indexes, triggers, functions, down migrations, or ORM-specific migration
  metadata.
- Runtime configuration parsing flattens scalar JSON, TOML, and YAML keys into redacted
  configuration knowledge. It does not interpret framework-specific config schemas, environment
  interpolation, includes/imports, profiles, encrypted values, or arrays of objects.
- runbooks remain separate Phase 5 work.

## Tests

```bash
cargo test -p athanor-extractor-operations
```
