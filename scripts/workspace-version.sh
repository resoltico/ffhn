#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=scripts/common.sh
. "$(cd -- "$(dirname -- "$(printf '%s\n' "${BASH_SOURCE[0]}" | sed 's#\\#/#g')")" && pwd)/common.sh"

print_usage() {
    local command_name="$1"

    cat <<EOF
Usage: ${command_name} [manifest-path]

Print the [workspace.package] version from one Cargo manifest.

Inputs:
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
    local manifest_path="${1:-${repo_root}/Cargo.toml}"

    "${script_dir}/workspace-package-field.sh" version "${manifest_path}"
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
    main "$@"
fi
