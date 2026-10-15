#!/usr/bin/env bash
set -euo pipefail

log_file=${1:?usage: annotate-log-tail.sh LOG_FILE TITLE}
title=${2:?usage: annotate-log-tail.sh LOG_FILE TITLE}

if [[ ! -f "$log_file" ]]; then
  printf '::error title=%s::Diagnostic log was not created: %s\n' "$title" "$log_file"
  exit 0
fi

message="$(tail -n 18 "$log_file" | tr '\r\n' ' ')"
message=${message//%/%25}
if ((${#message} > 6000)); then
  message=${message: -6000}
fi
if [[ -z "$message" ]]; then
  message="The failing command produced no captured output."
fi

printf '::error title=%s::%s\n' "$title" "$message"
