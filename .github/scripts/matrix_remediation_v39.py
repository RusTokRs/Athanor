#!/usr/bin/env python3
from __future__ import annotations

import argparse
import os
import textwrap
from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file_path = Path(path)
    text = file_path.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one target, found {count}: {old!r}")
    file_path.write_text(text.replace(old, new, 1))


def patch_windows_process_termination() -> None:
    path = "crates/athanor-app/src/runtime/process_runner.rs"
    replace_once(
        path,
        "/// `Child::kill` is retained as a fallback because a descendant may have already exited or the\n"
        "/// platform helper may be unavailable. Unix children run in their own process group and receive a\n"
        "/// group signal. Windows `taskkill /T` reaches child processes spawned by batch files and adapter\n"
        "/// launchers; Job Object containment remains a future hardening step.\n",
        "/// `Child::kill` is retained as a fallback because a descendant may have already exited or the\n"
        "/// platform helper may be unavailable. Unix children run in their own process group and receive a\n"
        "/// group signal. Windows starts `taskkill /T` for descendant cleanup, immediately signals the direct\n"
        "/// child, then waits for both cleanup and reaping; Job Object containment remains future hardening.\n",
    )
    replace_once(
        path,
        "    #[cfg(windows)]\n"
        "    if let Some(pid) = child.id() {\n"
        "        let pid = pid.to_string();\n"
        "        let _ = Command::new(\"taskkill\")\n"
        "            .args([\"/PID\", pid.as_str(), \"/T\", \"/F\"])\n"
        "            .kill_on_drop(true)\n"
        "            .output()\n"
        "            .await;\n"
        "    }\n\n"
        "    let _ = child.kill().await;\n",
        "    #[cfg(windows)]\n"
        "    {\n"
        "        let mut tree_kill = child.id().and_then(|pid| {\n"
        "            let pid = pid.to_string();\n"
        "            Command::new(\"taskkill\")\n"
        "                .args([\"/PID\", pid.as_str(), \"/T\", \"/F\"])\n"
        "                .kill_on_drop(true)\n"
        "                .spawn()\n"
        "                .ok()\n"
        "        });\n"
        "        let _ = child.start_kill();\n"
        "        if let Some(tree_kill) = tree_kill.as_mut() {\n"
        "            let _ = tree_kill.wait().await;\n"
        "        }\n"
        "        let _ = child.wait().await;\n"
        "    }\n\n"
        "    #[cfg(not(windows))]\n"
        "    {\n"
        "        let _ = child.kill().await;\n"
        "    }\n",
    )


def patch_plan() -> None:
    plan = Path("athanor_implementation_plan_ru.md")
    text = plan.read_text()
    run_id = os.environ["GITHUB_RUN_ID"]

    old_date = "> Актуализировано: 2026-07-20"
    if text.count(old_date) != 1:
        raise SystemExit(f"plan date marker count: {text.count(old_date)}")
    text = text.replace(old_date, "> Актуализировано: 2026-07-21", 1)

    anchor = (
        "- [ ] Полная `athanor/verification-matrix` должна стать successful на опубликованном "
        "architecture commit до повышения пакетов до `[x] verified`.\n\n"
        "## 4. Следующие активные пакеты"
    )
    section = textwrap.dedent(
        f"""\
        - [ ] Полная `athanor/verification-matrix` должна стать successful на опубликованном architecture commit до повышения пакетов до `[x] verified`.

        ### 3.12 `VERIFY-001G` — full workspace и cross-platform remediation

        - [x] Run `29770365670` (V21) подтвердил полный workspace suite, `--all-features` check и оба Clippy; source commit `2b38618e4b53e3c5cbd3fc2d7c2eb2cc2cd16c43` опубликован.
        - [x] Runs `29772014063` (V22) и `29775664727` (V24) закрыли path aliases и executable mode на Linux, macOS и Windows; source commit `c7d3fb541304d3c9c308192aff0c2d6736114a1f` опубликован.
        - [x] Runs `29777905429` (V26), `29779895591` (V28), `29781244450` (V29), `29804950206` (V31) и `29805615869` (V32) закрыли stale incremental state, Surreal allocation и первые CRLF-sensitive inventories.
        - [x] Runs `29806288730` (V33), `29806847096` и `29807541503` системно перечислили и нормализовали multiline `include_str!` inventories.
        - [x] Run `29808246589` (V35) подтвердил Linux/macOS quality, exact `store-surreal` gate и 12-кратный Surreal stress; Windows workspace сохранил единственный inventory failure.
        - [x] Run `29817583809` (V36) локализовал LF-only production/test boundary; run `29818777747` (V37) подтвердил его исправление и открыл два remaining Windows process lifecycle regressions.
        - [x] Run `29819493292` (V38) сохранил exact failures `timeout_kills_process_before_delayed_side_effect` и `cancellation_kills_process_before_delayed_side_effect`.
        - [x] Run `{run_id}` (V39) подтвердил полный quality chain на Linux, macOS и Windows, installer checks, docs/index smoke, exact `store-surreal` gate и allocation stress перед публикацией source/plan commit.
        - [x] `production_prefix` одинаково исключает LF и CRLF test modules; Windows termination немедленно сигнализирует direct child до ожидания tree cleanup и reaping.
        - [x] Временные V25–V39 workflows и remediation scripts физически удалены validated source commit.
        - [ ] Финальная стандартная `athanor/verification-matrix` должна подтвердить exact опубликованный source/plan HEAD.

        ## 4. Следующие активные пакеты"""
    ).rstrip()
    if text.count(anchor) != 1:
        raise SystemExit(f"plan section anchor count: {text.count(anchor)}")
    text = text.replace(anchor, section, 1)

    old_active = (
        "- [ ] получить exact matrix result для текущего HEAD;\n"
        "- [ ] разобрать remaining tests/Clippy/coverage/smoke failures, если они останутся;\n"
        "- [ ] сверить successful evidence SHA с architecture commit;\n"
        "- [ ] повысить только доказанные packages до `[x] verified`."
    )
    new_active = (
        "- [ ] получить exact standard matrix result для опубликованного source/plan HEAD;\n"
        "- [x] known tests/Clippy/coverage/installer/feature failures разобраны и исправлены по exact diagnostics;\n"
        "- [ ] сверить successful evidence SHA с architecture commit;\n"
        "- [ ] повысить только доказанные packages до `[x] verified`."
    )
    if text.count(old_active) != 1:
        raise SystemExit(f"plan active checklist count: {text.count(old_active)}")
    text = text.replace(old_active, new_active, 1)

    old_table = (
        "| `VERIFY-001F` | P1 | `[x] implemented` | Structural MCP and execution blockers closed by validated V10 |\n"
        "| `VERIFY-001` | P1 | `[!] blocked` | Exact successful status or JSON evidence identifies one commit |"
    )
    new_table = (
        "| `VERIFY-001F` | P1 | `[x] implemented` | Structural MCP and execution blockers closed by validated V10 |\n"
        "| `VERIFY-001G` | P1 | `[x] implemented` | Full workspace and cross-platform blockers closed by validated V21/V24/V39 |\n"
        "| `VERIFY-001` | P1 | `[!] blocked` | Exact successful status or JSON evidence identifies one commit |"
    )
    if text.count(old_table) != 1:
        raise SystemExit(f"plan table anchor count: {text.count(old_table)}")
    text = text.replace(old_table, new_table, 1)

    history_anchor = (
        "## 7. Последние изменения\n\n"
        "### 2026-07-20 — Validated execution remediation V10"
    )
    history = textwrap.dedent(
        f"""\
        ## 7. Последние изменения

        ### 2026-07-21 — Cross-platform verification remediation V21–V39

        - Source fixes для workspace, Surreal allocation, documentation completeness и cross-platform inventories сведены в один fail-closed publication chain.
        - V35–V38 последовательно локализовали Windows inventory и process lifecycle defects; V39 повторил полный validation после production fixes.
        - Run `{run_id}` является pre-publication evidence; окончательный status `verified` требует обычную `athanor/verification-matrix` на опубликованном source/plan SHA.
        - Временная remediation/diagnostic infrastructure V25–V39 удалена из validated source commit.

        ### 2026-07-20 — Validated execution remediation V10"""
    ).rstrip()
    if text.count(history_anchor) != 1:
        raise SystemExit(f"plan history anchor count: {text.count(history_anchor)}")
    text = text.replace(history_anchor, history, 1)

    plan.write_text(text)


def remove_temporary_files() -> None:
    temporary_paths = [
        ".github/workflows/plan-evidence-v25.yml",
        ".github/workflows/matrix-diagnostics-v26.yml",
        ".github/workflows/matrix-remediation-v27.yml",
        ".github/workflows/matrix-remediation-v28.yml",
        ".github/workflows/matrix-remediation-v29.yml",
        ".github/workflows/matrix-diagnostics-v30.yml",
        ".github/workflows/matrix-remediation-v31.yml",
        ".github/workflows/matrix-diagnostics-v32.yml",
        ".github/workflows/matrix-remediation-v33.yml",
        ".github/workflows/matrix-remediation-v35.yml",
        ".github/workflows/matrix-diagnostics-v36.yml",
        ".github/workflows/matrix-remediation-v37.yml",
        ".github/workflows/matrix-diagnostics-v38.yml",
        ".github/workflows/matrix-remediation-v39.yml",
        ".github/scripts/matrix_remediation_v28.py",
        ".github/scripts/matrix_remediation_v30.py",
        ".github/scripts/matrix_remediation_v31.py",
        ".github/scripts/matrix_remediation_v33.py",
        ".github/scripts/matrix_remediation_v35.py",
        ".github/scripts/matrix_remediation_v37.py",
        ".github/scripts/matrix_remediation_v39.py",
    ]
    missing = [path for path in temporary_paths if not Path(path).is_file()]
    if missing:
        raise SystemExit(f"temporary files missing before cleanup: {missing}")
    for path in temporary_paths:
        Path(path).unlink()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--publish", action="store_true")
    args = parser.parse_args()

    patch_windows_process_termination()
    if args.publish:
        patch_plan()
        remove_temporary_files()


if __name__ == "__main__":
    main()
