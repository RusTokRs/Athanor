#!/usr/bin/env python3
"""Verify a release tag and prepare notes from the matching changelog section."""

from __future__ import annotations

import argparse
import re
import sys
import tomllib
from datetime import date
from pathlib import Path

SEMVER = re.compile(
    r"^(0|[1-9][0-9]*)\."
    r"(0|[1-9][0-9]*)\."
    r"(0|[1-9][0-9]*)"
    r"(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?"
    r"(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$"
)
RELEASE_DATE = re.compile(r"^[0-9]{4}-[0-9]{2}-[0-9]{2}$")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tag", required=True, help="Git tag, for example v0.1.0")
    parser.add_argument("--changelog", required=True, type=Path)
    parser.add_argument("--notes-output", required=True, type=Path)
    parser.add_argument(
        "manifests",
        nargs="+",
        type=Path,
        help="Cargo.toml files for release binaries",
    )
    return parser.parse_args()


def package_version(manifest: Path) -> str:
    try:
        payload = tomllib.loads(manifest.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise ValueError(f"cannot read {manifest}: {error}") from error

    package = payload.get("package")
    version = package.get("version") if isinstance(package, dict) else None
    if not isinstance(version, str) or not version:
        raise ValueError(f"{manifest} does not define package.version")
    if not SEMVER.fullmatch(version):
        raise ValueError(f"{manifest} has non-semver package.version {version!r}")
    return version


def verify_versions(tag: str, manifests: list[Path]) -> str:
    if not tag.startswith("v"):
        raise ValueError(f"release tag must start with 'v': {tag!r}")

    tag_version = tag[1:]
    if not SEMVER.fullmatch(tag_version):
        raise ValueError(f"release tag is not v<semver>: {tag!r}")

    versions = {manifest: package_version(manifest) for manifest in manifests}
    mismatches = {
        manifest: version
        for manifest, version in versions.items()
        if version != tag_version
    }
    if mismatches:
        details = ", ".join(
            f"{manifest}={version}" for manifest, version in mismatches.items()
        )
        raise ValueError(f"tag {tag!r} does not match release packages: {details}")

    unique_versions = set(versions.values())
    if len(unique_versions) != 1:
        details = ", ".join(
            f"{manifest}={version}" for manifest, version in versions.items()
        )
        raise ValueError(f"release package versions disagree: {details}")

    return tag_version


def verify_release_date(release_date: str, version: str) -> None:
    if release_date == "Unreleased":
        raise ValueError(f"changelog section [{version}] must be dated before release")
    if not RELEASE_DATE.fullmatch(release_date):
        raise ValueError(
            f"changelog section [{version}] has invalid release date {release_date!r}"
        )
    try:
        date.fromisoformat(release_date)
    except ValueError as error:
        raise ValueError(
            f"changelog section [{version}] has invalid release date {release_date!r}"
        ) from error


def changelog_notes(changelog: Path, version: str) -> str:
    try:
        lines = changelog.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        raise ValueError(f"cannot read {changelog}: {error}") from error

    heading = re.compile(rf"^## \[{re.escape(version)}\] - (.+)$")
    start = None
    release_date = None
    for index, line in enumerate(lines):
        match = heading.fullmatch(line)
        if match:
            start = index + 1
            release_date = match.group(1)
            break

    if start is None or release_date is None:
        raise ValueError(f"{changelog} omits release section [{version}]")
    verify_release_date(release_date, version)

    end = next(
        (index for index in range(start, len(lines)) if lines[index].startswith("## [")),
        len(lines),
    )
    notes = "\n".join(lines[start:end]).strip()
    if not notes:
        raise ValueError(f"changelog section [{version}] has no release notes")
    return notes + "\n"


def main() -> int:
    args = parse_args()
    try:
        version = verify_versions(args.tag, args.manifests)
        notes = changelog_notes(args.changelog, version)
        args.notes_output.parent.mkdir(parents=True, exist_ok=True)
        args.notes_output.write_text(notes, encoding="utf-8", newline="\n")
    except (OSError, ValueError) as error:
        print(f"release contract verification failed: {error}", file=sys.stderr)
        return 1

    packages = ", ".join(str(path) for path in args.manifests)
    print(f"release version {version} matches {packages}")
    print(f"release notes written to {args.notes_output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
