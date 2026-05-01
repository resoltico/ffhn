#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=scripts/common.sh
. "$(cd -- "$(dirname -- "$(printf '%s\n' "${BASH_SOURCE[0]}" | sed 's#\\#/#g')")" && pwd)/common.sh"

skip_build="${FFHN_DEVCONTAINER_SKIP_BUILD:-0}"

print_usage() {
    local command_name="$1"

    cat <<EOF
Usage: ${command_name}

Build FFHN's committed contributor devcontainer and run ./check.sh inside it using the
same persistent Cargo/cache volumes as the editor-driven devcontainer workflow.
EOF
}

assert_skip_build_mode() {
    case "${skip_build}" in
        0|1) ;;
        *)
            ffhn_die \
                "unsupported FFHN_DEVCONTAINER_SKIP_BUILD=${skip_build}; expected 0 or 1"
            ;;
    esac
}

main() {
    local command_name="${BASH_SOURCE[0]}"

    if ffhn_is_help_flag "${1:-}"; then
        print_usage "${command_name}"
        return 0
    fi
    [[ $# -eq 0 ]] || ffhn_usage_error "${command_name}" "this script does not accept arguments"

    command -v docker >/dev/null 2>&1 || ffhn_die \
        "run-devcontainer-check.sh requires docker in PATH"
    docker info >/dev/null 2>&1 || ffhn_die \
        "run-devcontainer-check.sh requires a running docker daemon"
    assert_skip_build_mode

    local script_dir
    script_dir="$(ffhn_resolve_script_dir "${BASH_SOURCE[0]}")"
    local repo_root
    repo_root="$(ffhn_repo_root_from_script_dir "${script_dir}")"
    local image_tag="ffhn-devcontainer:local"

    if [[ "${skip_build}" == "1" ]]; then
        docker image inspect "${image_tag}" >/dev/null 2>&1 || ffhn_die \
            "run-devcontainer-check.sh requires ${image_tag} to exist when FFHN_DEVCONTAINER_SKIP_BUILD=1"
        printf 'devcontainer gate: reusing contributor image %s\n' "${image_tag}"
    else
        printf 'devcontainer gate: building contributor image\n'
        docker build \
            --tag "${image_tag}" \
            --file "${repo_root}/.devcontainer/Dockerfile" \
            "${repo_root}/.devcontainer" >/dev/null
    fi

    printf 'devcontainer gate: preparing canonical contributor volumes\n'
    docker volume create ffhn-cargo-registry >/dev/null
    docker volume create ffhn-cargo-git >/dev/null
    docker volume create ffhn-user-cache >/dev/null
    docker volume create ffhn-target >/dev/null
    docker volume create ffhn-fuzz-target >/dev/null

    printf 'devcontainer gate: running ./check.sh inside contributor container\n'
    docker run --rm \
        --volume "${repo_root}:/workspaces/ffhn" \
        --volume "ffhn-cargo-registry:/home/vscode/.cargo/registry" \
        --volume "ffhn-cargo-git:/home/vscode/.cargo/git" \
        --volume "ffhn-user-cache:/home/vscode/.cache" \
        --volume "ffhn-target:/workspaces/ffhn/target" \
        --volume "ffhn-fuzz-target:/workspaces/ffhn/fuzz/target" \
        --workdir /workspaces/ffhn \
        --env FFHN_DEVCONTAINER=1 \
        "${image_tag}" \
        bash -lc '
            set -euo pipefail
            ./scripts/devcontainer-prepare-user-home.sh
            git config --global --add safe.directory /workspaces/ffhn
            ./check.sh
        '

    printf 'devcontainer gate: success\n'
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
    main "$@"
fi
