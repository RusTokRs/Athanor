---
id: doc://docs/development/application-artifact-checksums.md
kind: developer_guide
language: en
source_language: en
status: verified
---
# Application Artifact Checksums

## Scope

Transactional application generations are selected through `.athanor/state/index-current.json`. Schema
`athanor.index_current.v2` binds that pointer to the exact immutable read model and immutable index state
that were ready before the pointer switch.

The checksum layer detects post-publication modification, missing files, extra files, and filesystem-type
substitution before a migrated reader receives either selected path.

## Trust Chain

```text
index-current.v2
  ├─ read_model_manifest_sha256 ──> immutable manifest.json
  │                                  └─ checksums.files
  │                                       ├─ diagnostics.jsonl
  │                                       ├─ entities.jsonl
  │                                       ├─ facts.jsonl
  │                                       └─ relations.jsonl
  └─ index_state_sha256 ──────────> immutable index-state-gen_<snapshot>.json
```

Digest values use lowercase `sha256:<64 hexadecimal characters>` form.

The manifest checksum set is deterministic:

```json
{
  "checksums": {
    "algorithm": "sha256",
    "files": {
      "diagnostics.jsonl": "sha256:...",
      "entities.jsonl": "sha256:...",
      "facts.jsonl": "sha256:...",
      "relations.jsonl": "sha256:..."
    }
  }
}
```

The file set is exact and ordered by name. `manifest.json` is not self-hashed; its digest is stored in
`index-current.v2`.

## Publication

1. The compatibility read model and state are identity-validated after canonical commit.
2. The read model and state are copied through sibling staging paths into deterministic immutable paths.
3. If an immutable path already exists during recovery, every data file and the state payload must match
   the compatibility source before it can be reused.
4. The immutable read model must contain exactly four direct regular JSONL files and `manifest.json`.
5. The manifest is atomically sealed with the four data-file digests.
6. The sealed manifest and immutable state file are hashed.
7. `IndexCurrent::write` validates identity and the complete checksum chain.
8. Only then is `index-current.v2` atomically selected and the bridge journal cleared.

The immutable generation may exist briefly while its bridge journal is still pending, but it is not visible
to migrated readers until the checksum-bound pointer switch. Recovery repeats source matching, sealing, and
pointer validation, so crashes before or during sealing are idempotent.

Symlinks, nested directories, device files, sockets, unknown extra files, and other non-regular direct
entries are rejected by the checksum contract.

## Reader And Repair Behaviour

A v2 pointer is fail-closed. Resolution verifies:

- pointer schema, snapshot, generation, and deterministic paths;
- read-model manifest identity and manifest digest;
- exact equality between the manifest file map and the four actual direct data files;
- every data-file digest;
- immutable index-state identity and digest.

`repair inspect` reports `index_current_checksum_mismatch` for a checksum, missing-file, extra-file, or
filesystem-type failure. Existing cleanup guards treat this as unsafe publication state and refuse
mutation.

Canonical latest repair uses the same `IndexCurrent::load` validation when the application pointer is the
requested target, so a tampered application generation cannot nominate canonical latest visibility.

## Migration Window

`athanor.index_current.v1` remains readable as a temporary identity-only migration format. It must not
contain checksum fields. New publication and bridge-journal recovery write v2.

A pre-checksum immutable generation may be sealed only while publication recovery owns
`.athanor/state/index-publication.lock`. Before sealing, its four data files and state payload must match
the validated compatibility source. Once external consumers have completed the compatibility release, v1
pointer acceptance can be removed together with mutable compatibility artifact writes.

## Verification

```bash
cargo fmt --all -- --check
cargo test -p athanor-app artifact_checksum --locked
cargo test -p athanor-app index_current --locked
cargo test -p athanor-app index_current_runtime_tests --locked
cargo test -p athanor-app index_publication_recovery_fault_tests --locked
cargo test -p ath --test index_checksum_cli --locked
cargo test -p ath --test index_checksum_recovery_cli --locked
cargo test -p ath --test repair_cli --locked
cargo clippy -p athanor-app --all-targets --locked -- -D warnings
cargo clippy -p ath --all-targets --locked -- -D warnings
```

Required regressions:

- a production index writes v2 and four JSONL file digests;
- a no-change run preserves the same checksum-bound pointer;
- bridge-journal recovery upgrades a v1/pre-checksum generation and is idempotent;
- data-file tampering is rejected;
- immutable state tampering is rejected;
- a missing or extra data file is rejected;
- a reused immutable target must match the compatibility source before sealing;
- path or filesystem-type substitution is rejected;
- repair inspection reports checksum corruption;
- legacy v1 remains readable only during the migration window.

Hosted compile, test, Clippy, Windows filesystem, and remote backend evidence remain separate release
evidence packages; this document does not claim them without recorded runs.
