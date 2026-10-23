#!/usr/bin/env bash
set -euo pipefail

log_file=${1:?usage: annotate-log-tail.sh LOG_FILE TITLE}
title=${2:?usage: annotate-log-tail.sh LOG_FILE TITLE}

if [[ ! -f "$log_file" ]]; then
  printf '::error title=%s::Diagnostic log was not created: %s\n' "$title" "$log_file"
  exit 0
fi

log_tail="$(tail -n 30 "$log_file")"
failure_summary="$(grep -E -B 1 -A 1 'panicked at|assertion failed:|test result: FAILED|error: test failed|failures:|error(\[|:)' "$log_file" | tail -n 120 || true)"
if [[ -n "$failure_summary" ]]; then
  message="$log_tail"$'\n--- failure summary ---\n'"$failure_summary"
else
  message="$log_tail"
fi
message=${message//$'\r'/ }
message=${message//$'\n'/ }
message=${message//%/%25}
if ((${#message} > 12000)); then
  message=${message: -12000}
fi
if [[ -z "$message" ]]; then
  message="The failing command produced no captured output."
fi

printf '::error title=%s::%s\n' "$title" "$message"
