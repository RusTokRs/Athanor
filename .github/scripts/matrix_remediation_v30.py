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


def patch_surreal_create_only() -> None:
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

                    let response = self
                        .db
                        .query(
                            "CREATE ONLY type::thing('snapshot', $snapshot_id) "
                                .to_owned()
                                + "CONTENT $record RETURN AFTER;",
                        )
                        .bind(("snapshot_id", snapshot_id.clone()))
                        .bind(("record", record))
                        .await
                        .map_err(|error| {
                            CoreError::Adapter(format!(
                                "failed to execute snapshot record allocation: {error}"
                            ))
                        })?;
                    let mut response = match response.check() {
                        Ok(response) => response,
                        Err(error)
                            if attempt + 1 < MAX_ATTEMPTS
                                && (is_retryable_counter_conflict(&error.to_string())
                                    || is_snapshot_record_collision(&error.to_string())) =>
                        {
                            sleep(Duration::from_millis(1_u64 << attempt.min(6))).await;
                            continue;
                        }
                        Err(error) => {
                            return Err(CoreError::Adapter(format!(
                                "failed to create snapshot record: {error}"
                            )));
                        }
                    };
                    let created: Option<SnapshotRecord> = response.take(0).map_err(|error| {
                        CoreError::Adapter(format!(
                            "failed to parse created snapshot record: {error}"
                        ))
                    })?;
                    if created.is_some() {
                        return Ok(SnapshotId(snapshot_id));
                    }
                    if attempt + 1 < MAX_ATTEMPTS {
                        sleep(Duration::from_millis(1_u64 << attempt.min(6))).await;
                        continue;
                    }
                    return Err(CoreError::Conflict(format!(
                        "snapshot record {snapshot_id} was not created after {MAX_ATTEMPTS} attempts"
                    )));
                }
                unreachable!("snapshot allocation retry loop always returns");
            }

            """
        ),
        "    ",
    )
    lines[start:end] = function.splitlines(keepends=True)
    path.write_text("".join(lines))


def patch_docs_frontmatter() -> None:
    documents = {
        "docs/development/legacy-runtime-compatibility.md": "developer_guide",
        "docs/development/publication-semantics-inventory.md": "architecture_inventory",
    }
    for path, kind in documents.items():
        file_path = Path(path)
        text = file_path.read_text()
        if text.startswith("---\n"):
            raise SystemExit(f"{path}: frontmatter already exists")
        frontmatter = textwrap.dedent(
            f"""\
            ---
            id: doc://{path}
            kind: {kind}
            language: en
            source_language: en
            status: implemented
            ---
            """
        )
        file_path.write_text(frontmatter + text)


def patch_plan_and_cleanup() -> None:
    path = Path("athanor_implementation_plan_ru.md")
    text = path.read_text()
    run_id = os.environ["GITHUB_RUN_ID"]
    generated = (
        f"- [x] Run `{run_id}` (V28) подтвердил quality chain на трёх ОС "
        "и exact `store-surreal` feature slice перед публикацией этого source commit."
    )
    replacement = (
        "- [x] Run `29779895591` (V28) подтвердил полный `store-surreal` "
        "check/test/Clippy и сохранил cross-platform quality diagnostics.\n"
        "- [x] Run `29781244450` (V29) локализовал два последних blockers: "
        "неатомарное создание Surreal snapshot record и docs completeness.\n"
        f"- [x] Run `{run_id}` (V31) подтвердил полный quality chain на Linux, macOS и Windows, "
        "installer checks и exact `store-surreal` feature slice перед публикацией."
    )
    if text.count(generated) != 1:
        raise SystemExit(
            f"plan: expected one generated remediation line, found {text.count(generated)}"
        )
    text = text.replace(generated, replacement, 1)
    cleanup = (
        "- [x] Временные V25–V28 workflows и remediation script физически удалены "
        "validated commit."
    )
    cleanup_new = (
        "- [x] Временные V25–V31 workflows и remediation scripts физически удалены "
        "validated commit."
    )
    if text.count(cleanup) != 1:
        raise SystemExit(f"plan: expected one cleanup line, found {text.count(cleanup)}")
    path.write_text(text.replace(cleanup, cleanup_new, 1))

    for temporary in [
        ".github/workflows/matrix-remediation-v29.yml",
        ".github/workflows/matrix-diagnostics-v30.yml",
        ".github/workflows/matrix-remediation-v31.yml",
        ".github/scripts/matrix_remediation_v30.py",
    ]:
        Path(temporary).unlink()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--publish", action="store_true")
    args = parser.parse_args()

    patch_surreal_create_only()
    patch_docs_frontmatter()
    if args.publish:
        patch_plan_and_cleanup()


if __name__ == "__main__":
    main()
