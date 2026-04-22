#!/usr/bin/env bash

set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "${script_dir}/.." && pwd)"
manifest_path="${1:-${repo_root}/Cargo.toml}"

awk '
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
    if (line ~ /^[[:space:]]*version[[:space:]]*=[[:space:]]*"/) {
        sub(/^[[:space:]]*version[[:space:]]*=[[:space:]]*"/, "", line)
        sub(/".*$/, "", line)
        print line
        found = 1
        exit
    }
}
END {
    if (!found) {
        print "error: [workspace.package] version not found in " FILENAME > "/dev/stderr"
        exit 1
    }
}
' "${manifest_path}"
