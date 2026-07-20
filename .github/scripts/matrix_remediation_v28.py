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


def patch_api_inventory() -> None:
    replace_once(
        "crates/athanor-app/tests/api_composition_inventory.rs",
        '    assert!(API_ROOT.contains("#[cfg(test)]\\nmod tests;"));',
        '    let normalized_root = API_ROOT.replace("\\r\\n", "\\n");\n'
        '    assert!(normalized_root.contains("#[cfg(test)]\\nmod tests;"));',
    )


def patch_process_tests() -> None:
    path = "crates/athanor-app/src/runtime/tests/process.rs"
    replace_once(
        path,
        "use std::fs;\nuse std::path::{Path, PathBuf};",
        "#[cfg(unix)]\nuse std::fs;\nuse std::path::Path;\n"
        "#[cfg(unix)]\nuse std::path::PathBuf;",
    )
    replace_once(
        path,
        "#[cfg(unix)]\nuse super::fixtures::sh_path;\n"
        "use super::fixtures::{failing_command, sleep_command, stdout_bytes_command, test_working_dir};",
        "#[cfg(unix)]\nuse super::fixtures::{sh_path, test_working_dir};\n"
        "use super::fixtures::{failing_command, sleep_command, stdout_bytes_command};",
    )
    replace_once(
        path,
        "fn temp_root(label: &str) -> PathBuf {",
        "#[cfg(unix)]\nfn temp_root(label: &str) -> PathBuf {",
    )


def patch_index_runtime() -> None:
    path = Path("crates/athanor-app/src/index_runtime.rs")
    lines = path.read_text().splitlines(keepends=True)

    start_line = '    let previous_state = state_store.load().context("failed to load index state")?;\n'
    output_line = "    let output_result = if options.validate_only {\n"
    try:
        start = lines.index(start_line)
        end = lines.index(output_line, start)
    except ValueError as error:
        raise SystemExit(f"index runtime baseline marker missing: {error}") from error

    baseline = textwrap.dedent(
        """\
        let previous_state = state_store.load().context("failed to load index state")?;
        let previous_snapshot = match previous_state.snapshot.as_ref() {
            Some(snapshot) => canonical_store
                .load_snapshot(&SnapshotId(snapshot.clone()))
                .await
                .context("failed to load previous canonical snapshot")?,
            None => None,
        };
        let has_previous_canonical_snapshot = previous_snapshot.is_some();
        let incremental = if previous_state.snapshot.is_some()
            && !has_previous_canonical_snapshot
        {
            IncrementalIndexContext::default()
        } else {
            IncrementalIndexContext {
                previous_state: previous_state.clone(),
                previous_snapshot,
            }
        };

        """
    )
    lines[start:end] = [f"    {line}" if line.strip() else line for line in baseline.splitlines(keepends=True)]

    replacements = 0
    index = 0
    while index + 3 < len(lines):
        if (
            lines[index].strip() == "IncrementalIndexContext {"
            and lines[index + 1].strip() == "previous_state: previous_state.clone(),"
            and lines[index + 2].strip() == "previous_snapshot,"
            and lines[index + 3].strip() == "},"
        ):
            indent = lines[index][: len(lines[index]) - len(lines[index].lstrip())]
            lines[index : index + 4] = [f"{indent}incremental.clone(),\n"]
            replacements += 1
            index += 1
            continue
        index += 1
    if replacements != 4:
        raise SystemExit(f"index runtime: expected four incremental contexts, found {replacements}")

    no_op = "    if previous_state.snapshot.as_deref() == Some(output.snapshot.0.as_str())\n"
    try:
        no_op_index = lines.index(no_op)
    except ValueError as error:
        raise SystemExit("index runtime no-op marker missing") from error
    lines.insert(no_op_index, "    if has_previous_canonical_snapshot\n")
    lines[no_op_index + 1] = (
        "        && previous_state.snapshot.as_deref() == Some(output.snapshot.0.as_str())\n"
    )

    path.write_text("".join(lines))


def patch_index_runtime_tests() -> None:
    path = Path("crates/athanor-app/src/index_runtime_tests.rs")
    text = path.read_text()
    anchor = textwrap.dedent(
        """\
        #[tokio::test]
        async fn cancelled_index_does_not_publish_snapshot_state_or_read_model() {
        """
    )
    regression = textwrap.dedent(
        """\
        #[tokio::test]
        async fn missing_canonical_snapshot_invalidates_stale_incremental_state() {
            let root = test_root("missing-canonical");
            fs::create_dir_all(root.join("src")).expect("create source directory");
            fs::write(root.join("src/lib.rs"), "pub fn recovered() {}\\n")
                .expect("write source file");
            let composition = crate::test_runtime::composition();

            let first = run_index(&root, &composition).await;
            let canonical_root = root.join(".athanor/store/canonical/jsonl");
            fs::remove_dir_all(&canonical_root).expect("remove canonical store only");
            assert!(root.join(".athanor/state/index-state.json").is_file());

            let rebuilt = run_index(&root, &composition).await;
            assert_eq!(rebuilt.changed_files, 1);
            let latest = JsonlKnowledgeStore::new(&canonical_root)
                .load_latest_snapshot()
                .await
                .expect("load rebuilt canonical snapshot")
                .expect("rebuilt canonical snapshot exists");
            assert_eq!(
                latest.snapshot.as_ref().map(|snapshot| snapshot.0.as_str()),
                Some(rebuilt.snapshot.as_str())
            );
            let docs = crate::check_docs_with_composition(
                crate::DocsCheckOptions { root: root.clone() },
                &composition,
            )
            .await
            .expect("docs check loads rebuilt canonical snapshot");
            assert_eq!(docs.snapshot, rebuilt.snapshot);
            assert!(docs.passed);
            assert_eq!(first.files_indexed, rebuilt.files_indexed);

            fs::remove_dir_all(root).expect("remove missing-canonical fixture");
        }

        #[tokio::test]
        async fn cancelled_index_does_not_publish_snapshot_state_or_read_model() {
        """
    )
    count = text.count(anchor)
    if count != 1:
        raise SystemExit(f"index runtime tests: expected one anchor, found {count}")
    path.write_text(text.replace(anchor, regression, 1))


def patch_surreal_backend() -> None:
    path = Path("crates/athanor-store-surrealdb/src/backend_store.rs")
    lines = path.read_text().splitlines(keepends=True)
    begin_marker = (
        "    async fn begin_snapshot(&self, repo: RepoId, base: SnapshotBase) "
        "-> CoreResult<SnapshotId> {\n"
    )
    try:
        start = lines.index(begin_marker)
        end = next(
            index
            for index in range(start + 1, len(lines))
            if lines[index].startswith("    async fn put_entities")
        )
    except (ValueError, StopIteration) as error:
        raise SystemExit(f"Surreal begin_snapshot marker missing: {error}") from error

    function = textwrap.indent(
        textwrap.dedent(
            """\
            async fn begin_snapshot(&self, repo: RepoId, base: SnapshotBase) -> CoreResult<SnapshotId> {
                let _guard = self.write_gate.lock().await;
                let repo = repo.0;
                let base_branch = base.branch;
                let base_commit = base.commit;
                let base_parent_snapshot = base.parent_snapshot.map(|snapshot| snapshot.0);
                let base_working_tree = base.working_tree;

                const MAX_ATTEMPTS: usize = 32;
                for attempt in 0..MAX_ATTEMPTS {
                    let sequence = self.allocate_snapshot_sequence().await?;
                    let snapshot_id = format!("snap_surreal_{sequence:08}");
                    let record = SnapshotRecord {
                        id: snapshot_id.clone(),
                        repo: repo.clone(),
                        base_branch: base_branch.clone(),
                        base_commit: base_commit.clone(),
                        base_parent_snapshot: base_parent_snapshot.clone(),
                        base_working_tree,
                        sequence,
                        prepared: false,
                        committed: false,
                        generation: None,
                        allocation_operation_id: None,
                        allocation_created_at_unix_ms: None,
                    };

                    let result: Result<Option<SnapshotRecord>, _> = self
                        .db
                        .create(("snapshot", &snapshot_id))
                        .content(record)
                        .await;
                    match result {
                        Ok(_) => return Ok(SnapshotId(snapshot_id)),
                        Err(error)
                            if attempt + 1 < MAX_ATTEMPTS
                                && (is_retryable_counter_conflict(&error.to_string())
                                    || is_snapshot_record_collision(&error.to_string())) =>
                        {
                            sleep(Duration::from_millis(1_u64 << attempt.min(6))).await;
                        }
                        Err(error) => {
                            return Err(CoreError::Adapter(format!(
                                "failed to create snapshot record: {error}"
                            )));
                        }
                    }
                }
                unreachable!("snapshot allocation retry loop always returns");
            }

            """
        ),
        "    ",
    )
    lines[start:end] = function.splitlines(keepends=True)
    path.write_text("".join(lines))

    helper = textwrap.dedent(
        """\
        fn is_retryable_counter_conflict(message: &str) -> bool {
            let message = message.to_ascii_lowercase();
            message.contains("read or write conflict") || message.contains("can be retried")
        }
        """
    )
    replacement = helper + textwrap.dedent(
        """\

        fn is_snapshot_record_collision(message: &str) -> bool {
            let message = message.to_ascii_lowercase();
            message.contains("database record") && message.contains("already exists")
        }
        """
    )
    replace_once(str(path), helper, replacement)


def patch_plan() -> None:
    path = Path("athanor_implementation_plan_ru.md")
    text = path.read_text()
    anchor = (
        "- [ ] Полная `athanor/verification-matrix` должна стать successful на опубликованном "
        "architecture commit до повышения пакетов до `[x] verified`.\n\n"
        "## 4. Следующие активные пакеты"
    )
    run_id = os.environ["GITHUB_RUN_ID"]
    section = textwrap.dedent(
        f"""\
        - [ ] Полная `athanor/verification-matrix` должна стать successful на опубликованном architecture commit до повышения пакетов до `[x] verified`.

        ### 3.12 `VERIFY-001G` — full workspace и cross-platform remediation

        - [x] Run `29770365670` (V21) подтвердил полный workspace suite, `--all-features` check и оба Clippy; source commit `2b38618e4b53e3c5cbd3fc2d7c2eb2cc2cd16c43` опубликован.
        - [x] Run `29772014063` (V22) локализовал path aliases и executable mode; run `29775664727` (V24) подтвердил fixes на Linux, macOS и Windows, source commit `c7d3fb541304d3c9c308192aff0c2d6736114a1f` опубликован.
        - [x] Run `29777905429` (V26) локализовал CRLF-sensitive inventory, stale incremental state без canonical snapshot и Surreal snapshot-id collision.
        - [x] Run `{run_id}` (V28) подтвердил quality chain на трёх ОС и exact `store-surreal` feature slice перед публикацией этого source commit.
        - [x] Временные V25–V28 workflows и remediation script физически удалены validated commit.
        - [ ] Финальная стандартная `athanor/verification-matrix` должна подтвердить exact source/plan HEAD.

        ## 4. Следующие активные пакеты
        """
    ).rstrip()
    count = text.count(anchor)
    if count != 1:
        raise SystemExit(f"plan: expected one section anchor, found {count}")
    text = text.replace(anchor, section, 1)

    stale = "- [ ] разобрать remaining tests/Clippy/coverage/smoke failures, если они останутся;"
    done = (
        "- [x] known tests/Clippy/coverage/installer/feature failures разобраны "
        "и исправлены по exact diagnostics;"
    )
    if text.count(stale) != 1:
        raise SystemExit(f"plan: expected one active failure item, found {text.count(stale)}")
    text = text.replace(stale, done, 1)

    table = (
        "| `VERIFY-001F` | P1 | `[x] implemented` | Structural MCP and execution blockers "
        "closed by validated V10 |\n"
        "| `VERIFY-001` | P1 | `[!] blocked` | Exact successful status or JSON evidence "
        "identifies one commit |"
    )
    table_new = (
        "| `VERIFY-001F` | P1 | `[x] implemented` | Structural MCP and execution blockers "
        "closed by validated V10 |\n"
        "| `VERIFY-001G` | P1 | `[x] implemented` | Full workspace and cross-platform blockers "
        "closed by validated V21/V24/V28 |\n"
        "| `VERIFY-001` | P1 | `[!] blocked` | Exact successful status or JSON evidence "
        "identifies one commit |"
    )
    if text.count(table) != 1:
        raise SystemExit(f"plan: expected one table anchor, found {text.count(table)}")
    path.write_text(text.replace(table, table_new, 1))


def remove_temporary_files() -> None:
    for path in [
        ".github/workflows/plan-evidence-v25.yml",
        ".github/workflows/matrix-diagnostics-v26.yml",
        ".github/workflows/matrix-remediation-v27.yml",
        ".github/workflows/matrix-remediation-v28.yml",
        ".github/scripts/matrix_remediation_v28.py",
    ]:
        Path(path).unlink()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--publish", action="store_true")
    args = parser.parse_args()

    patch_api_inventory()
    patch_process_tests()
    patch_index_runtime()
    patch_index_runtime_tests()
    patch_surreal_backend()

    if args.publish:
        patch_plan()
        remove_temporary_files()


if __name__ == "__main__":
    main()
