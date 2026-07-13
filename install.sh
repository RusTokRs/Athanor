#!/usr/bin/env sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
cd "$script_dir"

manifest="SHA256SUMS"
if [ ! -f "$manifest" ]; then
  printf 'Missing checksum manifest: %s\n' "$script_dir/$manifest" >&2
  exit 1
fi

for binary in ath athd; do
  if [ ! -f "$binary" ]; then
    printf 'Missing packaged binary: %s\n' "$script_dir/$binary" >&2
    exit 1
  fi
done

if command -v sha256sum >/dev/null 2>&1; then
  sha256sum -c "$manifest"
elif command -v shasum >/dev/null 2>&1; then
  shasum -a 256 -c "$manifest"
else
  printf 'Cannot verify packaged binaries: sha256sum or shasum is required.\n' >&2
  exit 1
fi

prefix="${ATHANOR_INSTALL_DIR:-$HOME/.local/bin}"
mkdir -p "$prefix"
install -m 0755 ath "$prefix/ath"
install -m 0755 athd "$prefix/athd"
printf 'Verified and installed Athanor binaries to %s\n' "$prefix"
printf 'Ensure %s is on PATH, then register a project and run athd service install.\n' "$prefix"
