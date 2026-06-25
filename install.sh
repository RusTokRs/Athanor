#!/usr/bin/env sh
set -eu

prefix="${ATHANOR_INSTALL_DIR:-$HOME/.local/bin}"
mkdir -p "$prefix"
install -m 0755 ath "$prefix/ath"
install -m 0755 athd "$prefix/athd"
printf 'Installed Athanor binaries to %s\n' "$prefix"
printf 'Ensure %s is on PATH, then register a project and run athd service install.\n' "$prefix"
