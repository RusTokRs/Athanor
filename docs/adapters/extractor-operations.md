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
knowledge. The current slice parses dotenv-style files, Makefile targets, and Dockerfile stages,
commands, and environment declarations.

## Inputs

Reads `SourceFile` values whose project-relative file name matches one of these supported
operations files and whose content is UTF-8 text:

- `.env`, `.env.example`, `*.env`, and `*.env.example`
- `Makefile`, `makefile`, and `*.mk`
- `Dockerfile` and `*.Dockerfile`

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

## Outputs

Entities:

- `EntityKind::EnvVar` with stable keys like `env://DATABASE_URL`
- `EntityKind::ScriptCommand` for Makefile targets and Dockerfile command instructions
- `EntityKind::DockerService` for Dockerfile stages

Facts:

- `FactKind::EnvVarUsed` with `mechanism = "dotenv"` and `source_kind = "dotenv"`
- `FactKind::EnvVarUsed` with `mechanism = "dockerfile"` and `source_kind = "dockerfile"`
- `FactKind::SymbolDefined` for Makefile targets, Dockerfile stages, and Dockerfile command
  instructions

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
- Shell scripts, docker-compose files, deployment configs, CI files, and runbooks remain separate
  Phase 5 work.

## Tests

```bash
cargo test -p athanor-extractor-operations
```
