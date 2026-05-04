#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=scripts/common.sh
. "$(cd -- "$(dirname -- "$(printf '%s\n' "${BASH_SOURCE[0]}" | sed 's#\\#/#g')")" && pwd)/common.sh"

print_usage() {
    local command_name="$1"

    cat <<EOF
Usage: ${command_name}

Run FFHN's maintained local quality gate through the stable shell wrapper.
EOF
}

main() {
    local command_name="${BASH_SOURCE[0]}"

    if ffhn_is_help_flag "${1:-}"; then
        print_usage "${command_name}"
        return 0
    fi
    [[ $# -eq 0 ]] || ffhn_usage_error "${command_name}" "this script does not accept arguments"

    local script_dir
    script_dir="$(ffhn_resolve_script_dir "${BASH_SOURCE[0]}")"
    local repo_root
    repo_root="$(ffhn_repo_root_from_script_dir "${script_dir}")"

    cd "${repo_root}"
    exec "${repo_root}/check.sh"
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
    main "$@"
fi
