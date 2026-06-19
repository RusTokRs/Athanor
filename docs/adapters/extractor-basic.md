# Basic Extractor Adapter

Crate: `athanor-extractor-basic`

Port: `Extractor`

## Purpose

Provides the universal `FileExtractor`.

It creates canonical file-level knowledge for every discovered source file.

## Inputs

Reads `SourceFile` values produced by a `SourceProvider`.

Required fields:

- `path`
- `language_hint`
- `content_hash`
- `content`

## Outputs

Entities:

- `EntityKind::File`

Facts:

- `FactKind::FileDiscovered`

Evidence:

- `source_file`
- `extractor = "file"`
- `confidence = 1.0`
- `status = verified`

## Commands And Network

- Does not run external commands.
- Does not use the network.
- Does not modify project files.

## Tests

```bash
cargo test -p athanor-extractor-basic
```
