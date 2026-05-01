#!/usr/bin/env bash

set -euo pipefail

main() {
    local workspace_root
    workspace_root="$(cd -- "$(dirname -- "$(printf '%s\n' "${BASH_SOURCE[0]}" | sed 's#\\#/#g')")/.." && pwd)"

    local user_uid
    local user_gid
    user_uid="$(id -u)"
    user_gid="$(id -g)"

    local candidate
    for candidate in \
        "${HOME}/.cargo" \
        "${HOME}/.cargo/git" \
        "${HOME}/.cargo/registry" \
        "${HOME}/.cache" \
        "${workspace_root}/target" \
        "${workspace_root}/fuzz/target"; do
        mkdir -p "${candidate}"
        if [[ ! -w "${candidate}" ]]; then
            sudo chown -R "${user_uid}:${user_gid}" "${candidate}"
        fi
    done
}

main "$@"
