# athanor-extractor-operations

Operations source extractor adapter.

Implements: `Extractor`

## What It Emits

The current slice parses:

- dotenv-style files such as `.env.example`, `.env`, and `*.env`
- Makefile targets from `Makefile`, `makefile`, and `*.mk`
- Dockerfile stages, command instructions, and `ENV` declarations from `Dockerfile` and
  `*.Dockerfile`

Entities:

- `EntityKind::EnvVar` with `env://<NAME>` stable keys
- `EntityKind::ScriptCommand` for Makefile targets and Dockerfile `RUN`, `CMD`, and `ENTRYPOINT`
  instructions
- `EntityKind::DockerService` for Dockerfile stages

Facts:

- `FactKind::EnvVarUsed` from the environment variable entity to the canonical file entity
- `FactKind::SymbolDefined` from operational command/stage entities to the canonical file entity

Environment fact payloads mark the declaration source as `dotenv` or `dockerfile`. Raw values are
not stored, so real `.env` or Dockerfile defaults do not leak secrets into canonical snapshots.

## Inputs

`SourceFile` with UTF-8 text content and a supported operations project-relative path.

## Side Effects

None. The adapter does not run commands, use the network, or modify project files.

## Limitations

- Only simple dotenv `KEY=value` and `export KEY=value` declarations are parsed.
- Makefile parsing only recognizes top-level target declarations and prerequisites.
- Dockerfile parsing recognizes line-continued instructions, but does not execute shell syntax or
  interpret JSON-array command forms.
- Variable interpolation, shell command substitution, multiline values, and comments inside quoted
  values are not interpreted.
- Shell scripts, docker-compose, deployment configs, CI files, and runbooks remain separate Phase 5
  work.

## Test

```bash
cargo test -p athanor-extractor-operations
```
