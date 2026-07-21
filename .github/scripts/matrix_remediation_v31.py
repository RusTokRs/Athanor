#!/usr/bin/env python3
from __future__ import annotations

import argparse
from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file_path = Path(path)
    text = file_path.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one target, found {count}: {old!r}")
    file_path.write_text(text.replace(old, new, 1))


def patch_process_unique_snapshot_identity() -> None:
    path = "crates/athanor-store-surrealdb/src/backend_store.rs"
    replace_once(
        path,
        "use std::sync::Arc;\n",
        "use std::sync::atomic::{AtomicU64, Ordering};\n"
        "use std::sync::Arc;\n"
        "use std::time::{SystemTime, UNIX_EPOCH};\n",
    )
    replace_once(
        path,
        "impl SurrealKnowledgeStore {\n",
        "fn snapshot_allocation_nonce() -> String {\n"
        "    static NEXT_NONCE: AtomicU64 = AtomicU64::new(0);\n"
        "    let counter = NEXT_NONCE.fetch_add(1, Ordering::Relaxed);\n"
        "    let timestamp = SystemTime::now()\n"
        "        .duration_since(UNIX_EPOCH)\n"
        "        .unwrap_or_default()\n"
        "        .as_nanos();\n"
        "    format!(\"{timestamp:032x}{:08x}{counter:016x}\", std::process::id())\n"
        "}\n\n"
        "impl SurrealKnowledgeStore {\n",
    )
    replace_once(
        path,
        'let snapshot_id = format!("snap_surreal_{sequence:08}");',
        'let snapshot_id = format!(\n'
        '    "snap_surreal_{sequence:08}_{}",\n'
        '    snapshot_allocation_nonce()\n'
        ');',
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--publish", action="store_true")
    args = parser.parse_args()

    patch_process_unique_snapshot_identity()
    if args.publish:
        Path(".github/scripts/matrix_remediation_v31.py").unlink()


if __name__ == "__main__":
    main()
