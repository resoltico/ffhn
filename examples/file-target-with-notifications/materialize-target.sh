#!/bin/sh
set -eu

if [ "$#" -ne 1 ]; then
  printf 'usage: %s /path/to/watch-root/release_notes/target.toml\n' "$0" >&2
  exit 2
fi

destination=$1
destination_dir=$(dirname "$destination")
example_dir=$(CDPATH= cd -- "$(dirname "$0")" && pwd)
release_notes_path="${example_dir}/release-notes.html"
hook_script_path="${example_dir}/append-notification.sh"
hook_log_path="${destination_dir}/ffhn-release-notes-report.jsonl"
escaped_release_notes_path=$(printf '%s' "$release_notes_path" | sed 's/\\/\\\\/g; s/"/\\"/g')
escaped_hook_script_path=$(printf '%s' "$hook_script_path" | sed 's/\\/\\\\/g; s/"/\\"/g')
escaped_hook_log_path=$(printf '%s' "$hook_log_path" | sed 's/\\/\\\\/g; s/"/\\"/g')

mkdir -p "$destination_dir"

cat >"$destination" <<EOF
schema_name = "ffhn.target"
schema_version = 1
target_id = "release_notes"
display_name = "Local Release Notes Example"
enabled = true

[target]
kind = "file"
file_path = "${escaped_release_notes_path}"

[fetch]
engine = "file"
max_bytes = 2000000

[selection]
kind = "css_selector"
selector = "main"
match = "single"
output = "outer_html"
whitespace = "normalize"
rewrite_urls = false

[compare]
basis = "canonical_text_sha256"

[[compare.canonicalization]]
kind = "trim"

[[compare.canonicalization]]
kind = "collapse_whitespace"

[storage]
history_limit = 8

[[notifications]]
name = "log-json"
on = ["changed", "failed_transient", "failed_permanent"]
program = "/bin/sh"
args = ["${escaped_hook_script_path}", "${escaped_hook_log_path}"]
timeout_ms = 1000
EOF
