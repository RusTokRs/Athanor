#!/usr/bin/env python3
"""Verify that a release tag exactly matches all release package versions."""

from __future__ import annotations

import argparse
import re
import sys
import tomllib
from pathlib import Path

SEMVER = re.compile(
    r"^(0|[1-9][0-9]*)\."
    r"(0|[1-9][0-9]*)\."
    r"(0|[1-9][0-9]*)"
    r"(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?"
    r"(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$"
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tag", required=True, help="Git tag, for example v0.1.0")
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


def verify(tag: str, manifests: list[Path]) -> str:
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


def main() -> int:
    args = parse_args()
    try:
        version = verify(args.tag, args.manifests)
    except ValueError as error:
        print(f"release version verification failed: {error}", file=sys.stderr)
        return 1

    packages = ", ".join(str(path) for path in args.manifests)
    print(f"release version {version} matches {packages}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
