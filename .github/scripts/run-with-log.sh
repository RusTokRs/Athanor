#!/usr/bin/env bash
set -uo pipefail

if (($# < 3)); then
  echo "usage: run-with-log.sh LOG_FILE TITLE COMMAND [ARG ...]" >&2
  exit 2
fi

log_file=$1
title=$2
shift 2

"$@" 2>&1 | tee "$log_file"
command_status=${PIPESTATUS[0]}
if ((command_status != 0)); then
  bash .github/scripts/annotate-log-tail.sh "$log_file" "$title"
fi
exit "$command_status"
