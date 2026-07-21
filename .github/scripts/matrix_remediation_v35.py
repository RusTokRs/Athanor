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


def patch_context_inventory() -> None:
    path = "crates/athanor-app/tests/context_composition_inventory.rs"
    replace_once(
        path,
        '    assert!(APP_LIB_SOURCE.contains("#[path = \\"context_composition.rs\\"]\\npub mod context;"));',
        '    let normalized_app_lib = APP_LIB_SOURCE.replace("\\r\\n", "\\n");\n'
        '    assert!(normalized_app_lib.contains("#[path = \\"context_composition.rs\\"]\\npub mod context;"));',
    )


def patch_graph_inventory() -> None:
    path = "crates/athanor-app/tests/graph_composition_inventory.rs"
    replace_once(
        path,
        '    assert!(APP_LIB_SOURCE.contains("#[path = \\"graph/mod.rs\\"]\\npub mod graph;"));',
        '    let normalized_app_lib = APP_LIB_SOURCE.replace("\\r\\n", "\\n");\n'
        '    let normalized_graph_root = GRAPH_ROOT_SOURCE.replace("\\r\\n", "\\n");\n'
        '    assert!(normalized_app_lib.contains("#[path = \\"graph/mod.rs\\"]\\npub mod graph;"));',
    )
    replace_once(
        path,
        '    assert!(GRAPH_ROOT_SOURCE.contains("#[cfg(test)]\\nmod tests;"));',
        '    assert!(normalized_graph_root.contains("#[cfg(test)]\\nmod tests;"));',
    )


def patch_read_service_inventory() -> None:
    path = "crates/athanor-app/tests/read_service_composition_inventory.rs"
    replace_once(
        path,
        'fn assert_conventional_root(root: &str, modules: &[&str], exports: &[&str]) {\n',
        'fn assert_conventional_root(root: &str, modules: &[&str], exports: &[&str]) {\n'
        '    let normalized_root = root.replace("\\r\\n", "\\n");\n',
    )
    replace_once(
        path,
        '    assert!(root.contains("#[cfg(test)]\\nmod tests;"));',
        '    assert!(normalized_root.contains("#[cfg(test)]\\nmod tests;"));',
    )


def patch_publication_inventory() -> None:
    path = "crates/athanor-app/tests/publication_semantics_inventory.rs"
    old = (
        '    assert!(\n'
        '        PROJECTOR_SUPPORT\n'
        '            .contains("cleanup_backup_after_publish(&backup, output_kind);\\n    Ok(())")\n'
        '    );'
    )
    new = (
        '    let normalized_projector_support = PROJECTOR_SUPPORT.replace("\\r\\n", "\\n");\n'
        '    assert!(\n'
        '        normalized_projector_support\n'
        '            .contains("cleanup_backup_after_publish(&backup, output_kind);\\n    Ok(())")\n'
        '    );'
    )
    replace_once(path, old, new)


def patch_mcp_control_inventory() -> None:
    path = "crates/athanor-transport-mcp/tests/control_plane_saturation_inventory.rs"
    replace_once(
        path,
        'fn stdin_and_notifications_bypass_ordinary_request_saturation() {\n',
        'fn stdin_and_notifications_bypass_ordinary_request_saturation() {\n'
        '    let lifecycle = LIFECYCLE.replace("\\r\\n", "\\n");\n',
    )
    replace_once(
        path,
        '    assert!(LIFECYCLE.contains("tokio::select! {\\n                biased;"));',
        '    assert!(lifecycle.contains("tokio::select! {\\n                biased;"));',
    )
    replace_once(
        path,
        'fn inline_responses_never_hold_the_only_stdin_reader() {\n',
        'fn inline_responses_never_hold_the_only_stdin_reader() {\n'
        '    let lifecycle = LIFECYCLE.replace("\\r\\n", "\\n");\n',
    )
    replace_once(
        path,
        '        !LIFECYCLE\n            .contains("try_send(response)\\n            .context(\\"MCP response queue is saturated")',
        '        !lifecycle\n            .contains("try_send(response)\\n            .context(\\"MCP response queue is saturated")',
    )
    replace_once(
        path,
        'fn disconnect_cancels_registered_operations_before_task_drain() {\n',
        'fn disconnect_cancels_registered_operations_before_task_drain() {\n'
        '    let lifecycle = LIFECYCLE.replace("\\r\\n", "\\n");\n',
    )
    replace_once(
        path,
        '    assert!(LIFECYCLE.contains("*stdin_open = false;\\n    cancel_all(active_reads).await;"));',
        '    assert!(lifecycle.contains("*stdin_open = false;\\n    cancel_all(active_reads).await;"));',
    )
    replace_once(
        path,
        'fn request_runtime_owns_lifecycle_dependencies_and_lint_fixes() {\n',
        'fn request_runtime_owns_lifecycle_dependencies_and_lint_fixes() {\n'
        '    let lifecycle = LIFECYCLE.replace("\\r\\n", "\\n");\n',
    )
    replace_once(
        path,
        '    assert!(LIFECYCLE.contains(\n        "pub(super) async fn process_line(\\n    runtime: &RequestRuntime,\\n    requests: &mut RequestTasks,\\n    line: String,"\n    ));',
        '    assert!(lifecycle.contains(\n        "pub(super) async fn process_line(\\n    runtime: &RequestRuntime,\\n    requests: &mut RequestTasks,\\n    line: String,"\n    ));',
    )


def patch_mcp_publication_inventory() -> None:
    path = "crates/athanor-transport-mcp/tests/index_publication_cancellation_inventory.rs"
    replace_once(
        path,
        'fn pre_commit_pipeline_boundaries_check_operation_before_and_after_work() {\n',
        'fn pre_commit_pipeline_boundaries_check_operation_before_and_after_work() {\n'
        '    let pipeline_support = PIPELINE_SUPPORT_SOURCE.replace("\\r\\n", "\\n");\n',
    )
    replace_once(
        path,
        '        PIPELINE_SUPPORT_SOURCE\n            .contains("operation.check_active()?;\\n    let result = match operation.remaining()")',
        '        pipeline_support\n            .contains("operation.check_active()?;\\n    let result = match operation.remaining()")',
    )
    replace_once(
        path,
        '        PIPELINE_SUPPORT_SOURCE\n            .contains("None => future.await,\\n    }?;\\n    operation.check_active()?;")',
        '        pipeline_support\n            .contains("None => future.await,\\n    }?;\\n    operation.check_active()?;")',
    )


def patch_plan_and_cleanup() -> None:
    plan = Path("athanor_implementation_plan_ru.md")
    text = plan.read_text()
    run_id = os.environ["GITHUB_RUN_ID"]
    generated = (
        f"- [x] Run `{run_id}` (V33) подтвердил полный quality chain на Linux, macOS и Windows, "
        "installer checks, docs smoke, exact `store-surreal` gate и allocation stress перед публикацией."
    )
    replacement = (
        "- [x] Run `29806288730` (V33) подтвердил Linux/macOS quality, feature gate и stress; "
        "Windows открыл оставшиеся newline-sensitive inventories.\n"
        "- [x] Run `29806847096` локализовал Context inventory; run `29807541503` перечислил "
        "весь оставшийся класс multiline `include_str!` assertions.\n"
        f"- [x] Run `{run_id}` (V35) подтвердил полный quality chain на Linux, macOS и Windows, "
        "installer checks, docs smoke, exact `store-surreal` gate и allocation stress перед публикацией."
    )
    if text.count(generated) != 1:
        raise SystemExit(f"plan: expected one generated V33 line, found {text.count(generated)}")
    text = text.replace(generated, replacement, 1)
    cleanup = (
        "- [x] Временные V25–V33 workflows и remediation scripts физически удалены "
        "validated commit."
    )
    cleanup_new = (
        "- [x] Временные V25–V35 workflows и remediation scripts физически удалены "
        "validated commit."
    )
    if text.count(cleanup) != 1:
        raise SystemExit(f"plan: expected one V25–V33 cleanup line, found {text.count(cleanup)}")
    text = text.replace(cleanup, cleanup_new, 1)
    old_table = (
        "| `VERIFY-001G` | P1 | `[x] implemented` | Full workspace and cross-platform blockers "
        "closed by validated V21/V24/V33 |"
    )
    new_table = (
        "| `VERIFY-001G` | P1 | `[x] implemented` | Full workspace and cross-platform blockers "
        "closed by validated V21/V24/V35 |"
    )
    if text.count(old_table) != 1:
        raise SystemExit(f"plan: expected one VERIFY-001G V33 row, found {text.count(old_table)}")
    plan.write_text(text.replace(old_table, new_table, 1))

    for temporary in [
        ".github/workflows/matrix-remediation-v35.yml",
        ".github/scripts/matrix_remediation_v35.py",
    ]:
        Path(temporary).unlink()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--publish", action="store_true")
    args = parser.parse_args()

    patch_context_inventory()
    patch_graph_inventory()
    patch_read_service_inventory()
    patch_publication_inventory()
    patch_mcp_control_inventory()
    patch_mcp_publication_inventory()
    if args.publish:
        patch_plan_and_cleanup()


if __name__ == "__main__":
    main()
