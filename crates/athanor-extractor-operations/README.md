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
- docker-compose services, service commands, and environment declarations from common compose
  filenames such as `docker-compose.yml`, `compose.yaml`, and `*.compose.yml`
- GitHub Actions workflow, job, and step declarations from `.github/workflows/*.yml` and
  `.github/workflows/*.yaml`

Entities:

- `EntityKind::EnvVar` with `env://<NAME>` stable keys
- `EntityKind::ScriptCommand` for Makefile targets and Dockerfile `RUN`, `CMD`, and `ENTRYPOINT`
  instructions
- `EntityKind::ScriptCommand` for shell function declarations
- `EntityKind::ScriptCommand` for docker-compose service `command` and `entrypoint` declarations
- `EntityKind::ScriptCommand` for GitHub Actions workflows, jobs, `run` steps, and `uses` steps
- `EntityKind::DockerService` for Dockerfile stages and docker-compose services

Facts:

- `FactKind::EnvVarUsed` from the environment variable entity to the canonical file entity
- `FactKind::SymbolDefined` from operational command/stage entities to the canonical file entity

Environment fact payloads mark the declaration source as `dotenv`, `dockerfile`, `shell`,
`docker_compose`, or `github_actions`. Raw values are not stored, so real `.env`, Dockerfile
defaults, exported shell values, compose environment values, or workflow environment values do not
leak secrets into canonical snapshots.

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
- docker-compose parsing is limited to the top-level `services` map, service `image`, `build`,
  `command`, `entrypoint`, and `environment` declarations. It does not resolve `env_file`, profiles,
  includes, extends, anchors, volume semantics, healthchecks, dependencies, or networks.
- GitHub Actions parsing is limited to workflow name, top-level `env`, jobs, job `runs-on`, job
  `env`, and step `run`, `uses`, and `env` declarations. It does not evaluate expressions,
  permissions, matrices, reusable workflows, service containers, caches, artifacts, or secrets.
- Variable interpolation, shell command substitution, multiline values, and comments inside quoted
  values are not interpreted.
- deployment configs and runbooks remain separate Phase 5 work.

## Test

```bash
cargo test -p athanor-extractor-operations
```
