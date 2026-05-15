#!/usr/bin/env bash

set -euo pipefail

main() {
    local user_uid
    local user_gid
    user_uid="$(id -u)"
    user_gid="$(id -g)"

    local candidate
    for candidate in \
        "${HOME}/.cargo" \
        "${HOME}/.cargo/git" \
        "${HOME}/.cargo/registry" \
        "${HOME}/.cache"; do
        mkdir -p "${candidate}"
        if [[ ! -w "${candidate}" ]]; then
            sudo chown -R "${user_uid}:${user_gid}" "${candidate}"
        fi
    done
}

main "$@"
