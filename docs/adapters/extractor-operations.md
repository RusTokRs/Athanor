---
id: doc://docs/adapters/extractor-operations.md
kind: module_documentation
language: en
source_language: en
last_verified_snapshot: snap_jsonl_00000063
status: verified
---

# Operations Extractor

Crate: `athanor-extractor-operations`

Port: `Extractor`

The operations extractor reads operational configuration files and emits canonical operations
knowledge. The current slice parses dotenv-style files, Makefile targets, Dockerfile stages,
commands and environment declarations, shell script functions plus exported environment variables,
docker-compose services, commands, and environment declarations, and GitHub Actions workflow jobs,
steps, actions, and environment declarations.

## Inputs

Reads `SourceFile` values whose project-relative file name matches one of these supported
operations files and whose content is UTF-8 text:

- `.env`, `.env.example`, `*.env`, and `*.env.example`
- `Makefile`, `makefile`, and `*.mk`
- `Dockerfile` and `*.Dockerfile`
- `*.sh`, `*.bash`, and `*.zsh`
- `docker-compose.yml`, `docker-compose.yaml`, `compose.yml`, `compose.yaml`, `*.compose.yml`,
  and `*.compose.yaml`
- `.github/workflows/*.yml` and `.github/workflows/*.yaml`

Supported dotenv declarations:

```text
KEY=value
export KEY=value
```

Supported Makefile declarations:

```text
target: prerequisite
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

## Outputs

Entities:

- `EntityKind::EnvVar` with stable keys like `env://DATABASE_URL`
- `EntityKind::ScriptCommand` for Makefile targets and Dockerfile command instructions
- `EntityKind::ScriptCommand` for shell script functions
- `EntityKind::ScriptCommand` for docker-compose service `command` and `entrypoint` declarations
- `EntityKind::ScriptCommand` for GitHub Actions workflows, jobs, `run` steps, and `uses` steps
- `EntityKind::DockerService` for Dockerfile stages and docker-compose services

Facts:

- `FactKind::EnvVarUsed` with `mechanism = "dotenv"` and `source_kind = "dotenv"`
- `FactKind::EnvVarUsed` with `mechanism = "dockerfile"` and `source_kind = "dockerfile"`
- `FactKind::EnvVarUsed` with `mechanism = "shell"` and `source_kind = "shell"`
- `FactKind::EnvVarUsed` with `mechanism = "docker_compose"` and `source_kind = "docker_compose"`
- `FactKind::EnvVarUsed` with `mechanism = "github_actions"` and `source_kind = "github_actions"`
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
- deployment configs and runbooks remain separate Phase 5 work.

## Tests

```bash
cargo test -p athanor-extractor-operations
```
