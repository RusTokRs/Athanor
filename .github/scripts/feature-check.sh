#!/usr/bin/env bash
set -euo pipefail

features=${1:?usage: feature-check.sh FEATURES}
case "$features" in
  default) feature_args=() ;;
  all-features) feature_args=(--all-features) ;;
  *) feature_args=(--features "$features") ;;
esac

cargo check --workspace "${feature_args[@]}" --locked
cargo test --workspace --quiet "${feature_args[@]}" --locked
cargo clippy --workspace --all-targets "${feature_args[@]}" --locked -- -D warnings
