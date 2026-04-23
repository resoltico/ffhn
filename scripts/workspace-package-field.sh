#!/usr/bin/env bash

set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "${script_dir}/.." && pwd)"
field_name="${1:-}"
manifest_path="${2:-${repo_root}/Cargo.toml}"

if [[ -z "${field_name}" ]]; then
    printf 'error: workspace.package field name is required\n' >&2
    exit 1
fi

awk -v field_name="${field_name}" '
BEGIN {
    in_workspace_package = 0
    found = 0
}
/^[[:space:]]*\[workspace\.package\][[:space:]]*$/ {
    in_workspace_package = 1
    next
}
/^[[:space:]]*\[/ {
    if (in_workspace_package) {
        exit
    }
    next
}
/^[[:space:]]*#/ {
    next
}
{
    if (!in_workspace_package) {
        next
    }

    line = $0
    pattern = "^[[:space:]]*" field_name "[[:space:]]*=[[:space:]]*\""
    if (line ~ pattern) {
        sub(pattern, "", line)
        sub(/".*$/, "", line)
        print line
        found = 1
        exit
    }
}
END {
    if (!found) {
        printf "error: [workspace.package] %s not found in %s\n", field_name, FILENAME > "/dev/stderr"
        exit 1
    }
}
' "${manifest_path}"
