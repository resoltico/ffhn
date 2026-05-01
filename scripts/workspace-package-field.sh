#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=scripts/common.sh
. "$(cd -- "$(dirname -- "$(printf '%s\n' "${BASH_SOURCE[0]}" | sed 's#\\#/#g')")" && pwd)/common.sh"

print_usage() {
    local command_name="$1"

    cat <<EOF
Usage: ${command_name} <field-name> [manifest-path]

Print one string field from [workspace.package] in a Cargo manifest.

Inputs:
  field-name           Required field name such as version or description.
  manifest-path        Optional path to a Cargo.toml file. Defaults to ./Cargo.toml at the
                       repository root.
EOF
}

main() {
    local command_name="${BASH_SOURCE[0]}"

    if ffhn_is_help_flag "${1:-}"; then
        print_usage "${command_name}"
        return 0
    fi

    local script_dir
    script_dir="$(ffhn_resolve_script_dir "${BASH_SOURCE[0]}")"
    local repo_root
    repo_root="$(ffhn_repo_root_from_script_dir "${script_dir}")"
    local field_name="${1:-}"
    local manifest_path="${2:-${repo_root}/Cargo.toml}"

    [[ -n "${field_name}" ]] || ffhn_usage_error \
        "${command_name}" \
        "workspace.package field name is required"

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
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
    main "$@"
fi
