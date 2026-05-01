#!/bin/sh
set -eu

if [ "$#" -ne 1 ]; then
  printf 'usage: %s /path/to/log.jsonl\n' "$0" >&2
  exit 2
fi

cat >>"$1"
