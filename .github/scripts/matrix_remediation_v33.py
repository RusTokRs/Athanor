#!/usr/bin/env python3
from __future__ import annotations

import argparse
import os
from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file_path = Path(path)
    text = file_path.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one target, found {count}: {old!r}")
    file_path.write_text(text.replace(old, new, 1))


def patch_check_inventory() -> None:
    replace_once(
        "crates/athanor-app/tests/check_composition_inventory.rs",
        '    assert!(CHECK_ROOT.contains("#[cfg(test)]\\nmod tests;"));',
        '    let normalized_root = CHECK_ROOT.replace("\\r\\n", "\\n");\n'
        '    assert!(normalized_root.contains("#[cfg(test)]\\nmod tests;"));',
    )


def patch_plan_and_cleanup() -> None:
    plan = Path("athanor_implementation_plan_ru.md")
    text = plan.read_text()
    run_id = os.environ["GITHUB_RUN_ID"]
    generated = (
        f"- [x] Run `{run_id}` (V31) подтвердил полный quality chain на Linux, macOS и Windows, "
        "installer checks и exact `store-surreal` feature slice перед публикацией."
    )
    replacement = (
        "- [x] Run `29804950206` (V31) подтвердил Linux/macOS quality и полный "
        "`store-surreal` gate; Windows остановился на CRLF-sensitive Check inventory.\n"
        "- [x] Run `29805615869` (V32) подтвердил 12-кратный Surreal stress и локализовал "
        "единственный Windows blocker в `check_composition_inventory`.\n"
        f"- [x] Run `{run_id}` (V33) подтвердил полный quality chain на Linux, macOS и Windows, "
        "installer checks, docs smoke, exact `store-surreal` gate и allocation stress перед публикацией."
    )
    if text.count(generated) != 1:
        raise SystemExit(f"plan: expected one generated V31 line, found {text.count(generated)}")
    text = text.replace(generated, replacement, 1)
    cleanup = (
        "- [x] Временные V25–V31 workflows и remediation scripts физически удалены "
        "validated commit."
    )
    cleanup_new = (
        "- [x] Временные V25–V33 workflows и remediation scripts физически удалены "
        "validated commit."
    )
    if text.count(cleanup) != 1:
        raise SystemExit(f"plan: expected one V25–V31 cleanup line, found {text.count(cleanup)}")
    text = text.replace(cleanup, cleanup_new, 1)
    old_table = (
        "| `VERIFY-001G` | P1 | `[x] implemented` | Full workspace and cross-platform blockers "
        "closed by validated V21/V24/V28 |"
    )
    new_table = (
        "| `VERIFY-001G` | P1 | `[x] implemented` | Full workspace and cross-platform blockers "
        "closed by validated V21/V24/V33 |"
    )
    if text.count(old_table) != 1:
        raise SystemExit(f"plan: expected one VERIFY-001G table row, found {text.count(old_table)}")
    plan.write_text(text.replace(old_table, new_table, 1))

    for temporary in [
        ".github/workflows/matrix-diagnostics-v32.yml",
        ".github/workflows/matrix-remediation-v33.yml",
        ".github/scripts/matrix_remediation_v33.py",
    ]:
        Path(temporary).unlink()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--publish", action="store_true")
    args = parser.parse_args()

    patch_check_inventory()
    if args.publish:
        patch_plan_and_cleanup()


if __name__ == "__main__":
    main()
