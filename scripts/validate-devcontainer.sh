#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=scripts/common.sh
. "$(cd -- "$(dirname -- "$(printf '%s\n' "${BASH_SOURCE[0]}" | sed 's#\\#/#g')")" && pwd)/common.sh"

readonly volume_mode="${FFHN_DEVCONTAINER_VOLUME_MODE:-isolated}"
readonly helper_image_tag="ffhn-devcontainer-cli-helper:local"

select_volume_name() {
    local shared_name="$1"
    local isolated_name="$2"

    case "${volume_mode}" in
        isolated)
            printf '%s\n' "${isolated_name}"
            ;;
        shared-ci)
            printf '%s\n' "${shared_name}"
            ;;
        *)
            ffhn_die \
                "unsupported FFHN_DEVCONTAINER_VOLUME_MODE=${volume_mode}; expected isolated or shared-ci"
            ;;
    esac
}

select_validation_image_tag() {
    case "${volume_mode}" in
        isolated)
            printf 'ffhn-devcontainer-validate:%s\n' "$$"
            ;;
        shared-ci)
            printf 'ffhn-devcontainer:local\n'
            ;;
        *)
            ffhn_die \
                "unsupported FFHN_DEVCONTAINER_VOLUME_MODE=${volume_mode}; expected isolated or shared-ci"
            ;;
    esac
}

validate_inner_runtime() {
    [[ "${FFHN_DEVCONTAINER:-}" == "1" ]] || ffhn_die "inner runtime mode requires FFHN_DEVCONTAINER=1"

    local script_dir
    script_dir="$(ffhn_resolve_script_dir "${BASH_SOURCE[0]}")"
    local repo_root
    repo_root="$(ffhn_repo_root_from_script_dir "${script_dir}")"

    cd "${repo_root}"
    [[ -f Cargo.toml ]] || ffhn_die "inner runtime probe requires the FFHN workspace checkout"

    ./scripts/devcontainer-prepare-user-home.sh
    grep -F 'VERSION_ID="26.04"' /etc/os-release >/dev/null
    [[ "$(id -un)" == "vscode" ]]
    [[ -w "${HOME}/.cargo/registry" ]]
    [[ -w "${HOME}/.cargo/git" ]]
    [[ -w "${HOME}/.cache" ]]
    [[ -w "/workspaces/ffhn/target" ]]
    [[ -w "/workspaces/ffhn/fuzz/target" ]]
    rustc --version | grep -E '^rustc 1\.95\.0 ' >/dev/null
    cargo --version >/dev/null
    cargo +nightly --version >/dev/null
    rustup target list --installed | grep -Fx 'x86_64-unknown-linux-musl' >/dev/null
    cargo nextest --version >/dev/null
    cargo audit --version >/dev/null
    cargo deny --version >/dev/null
    cargo semver-checks --version >/dev/null
    cargo outdated --version >/dev/null
    cargo llvm-cov --version >/dev/null
    cargo fuzz --version >/dev/null
    shellcheck --version >/dev/null
    gh --version >/dev/null
    clang --version >/dev/null
    ./check.sh --help >/dev/null
}

print_usage() {
    local command_name="$1"

    cat <<EOF
Usage: ${command_name}

Build FFHN's committed contributor devcontainer, smoke its raw Docker contract, and verify
the committed devcontainer client path.
EOF
}

main() {
    local command_name="${BASH_SOURCE[0]}"

    if ffhn_is_help_flag "${1:-}"; then
        print_usage "${command_name}"
        return 0
    fi

    if [[ "${FFHN_DEVCONTAINER:-}" == "1" ]]; then
        validate_inner_runtime
        return 0
    fi

    [[ $# -eq 0 ]] || ffhn_usage_error "${command_name}" "this script does not accept arguments"

    command -v docker >/dev/null 2>&1 || ffhn_die \
        "validate-devcontainer.sh requires docker in PATH"
    docker info >/dev/null 2>&1 || ffhn_die \
        "validate-devcontainer.sh requires a running docker daemon"

    local script_dir
    script_dir="$(ffhn_resolve_script_dir "${BASH_SOURCE[0]}")"
    local repo_root
    repo_root="$(ffhn_repo_root_from_script_dir "${script_dir}")"

    local dockerfile_path="${repo_root}/.devcontainer/Dockerfile"
    local helper_dockerfile_path="${repo_root}/scripts/devcontainer-cli-helper.Dockerfile"
    local prepare_script="${repo_root}/scripts/devcontainer-prepare-user-home.sh"
    local image_tag
    image_tag="$(select_validation_image_tag)"
    local cargo_registry_volume
    cargo_registry_volume="$(
        select_volume_name "ffhn-cargo-registry" "ffhn-devcontainer-cargo-registry-$$"
    )"
    local cargo_git_volume
    cargo_git_volume="$(
        select_volume_name "ffhn-cargo-git" "ffhn-devcontainer-cargo-git-$$"
    )"
    local user_cache_volume
    user_cache_volume="$(
        select_volume_name "ffhn-user-cache" "ffhn-devcontainer-user-cache-$$"
    )"
    local target_volume
    target_volume="$(
        select_volume_name "ffhn-target" "ffhn-devcontainer-target-$$"
    )"
    local fuzz_target_volume
    fuzz_target_volume="$(
        select_volume_name "ffhn-fuzz-target" "ffhn-devcontainer-fuzz-target-$$"
    )"

    [[ -f "${dockerfile_path}" ]] || ffhn_die "missing ${dockerfile_path}"
    [[ -f "${helper_dockerfile_path}" ]] || ffhn_die "missing ${helper_dockerfile_path}"
    [[ -f "${prepare_script}" ]] || ffhn_die "missing ${prepare_script}"

    readonly FFHN_VALIDATE_REPO_ROOT="${repo_root}"
    readonly FFHN_VALIDATE_IMAGE_TAG="${image_tag}"
    readonly FFHN_VALIDATE_CARGO_REGISTRY_VOLUME="${cargo_registry_volume}"
    readonly FFHN_VALIDATE_CARGO_GIT_VOLUME="${cargo_git_volume}"
    readonly FFHN_VALIDATE_USER_CACHE_VOLUME="${user_cache_volume}"
    readonly FFHN_VALIDATE_TARGET_VOLUME="${target_volume}"
    readonly FFHN_VALIDATE_FUZZ_TARGET_VOLUME="${fuzz_target_volume}"

    cleanup() {
        local devcontainer_ids=()
        local devcontainer_id

        set +e
        if [[ "${volume_mode}" == "isolated" ]]; then
            docker volume rm -f \
                "${FFHN_VALIDATE_CARGO_REGISTRY_VOLUME}" \
                "${FFHN_VALIDATE_CARGO_GIT_VOLUME}" \
                "${FFHN_VALIDATE_USER_CACHE_VOLUME}" \
                "${FFHN_VALIDATE_TARGET_VOLUME}" \
                "${FFHN_VALIDATE_FUZZ_TARGET_VOLUME}" >/dev/null 2>&1 || true
            docker image rm -f "${FFHN_VALIDATE_IMAGE_TAG}" >/dev/null 2>&1 || true
        fi

        while IFS= read -r devcontainer_id; do
            [[ -n "${devcontainer_id}" ]] || continue
            devcontainer_ids+=("${devcontainer_id}")
        done < <(docker ps -aq --filter "label=devcontainer.local_folder=${FFHN_VALIDATE_REPO_ROOT}")
        if ((${#devcontainer_ids[@]} > 0)); then
            docker rm -f "${devcontainer_ids[@]}" >/dev/null 2>&1 || true
        fi
        set -e
    }
    trap cleanup EXIT

    printf 'devcontainer validation: build raw contributor image\n'
    docker build \
        --tag "${image_tag}" \
        --file "${dockerfile_path}" \
        "${repo_root}/.devcontainer" >/dev/null

    printf 'devcontainer validation: probe raw image package contract\n'
    docker run --rm "${image_tag}" bash -lc '
        set -euo pipefail
        . /etc/os-release
        [[ "${ID}" == "ubuntu" ]]
        [[ "${VERSION_ID}" == "26.04" ]]
        rustc --version | grep -E "^rustc 1\\.95\\.0 " >/dev/null
        cargo +nightly --version >/dev/null
        cargo nextest --version >/dev/null
        cargo audit --version >/dev/null
        cargo deny --version >/dev/null
        cargo semver-checks --version >/dev/null
        cargo outdated --version >/dev/null
        cargo llvm-cov --version >/dev/null
        cargo fuzz --version >/dev/null
        shellcheck --version >/dev/null
        gh --version >/dev/null
        clang --version >/dev/null
        git --version >/dev/null
        jq --version >/dev/null
        python3 --version >/dev/null
    '

    if [[ "${volume_mode}" == "shared-ci" ]]; then
        printf 'devcontainer validation: reset shared contributor volumes for CI reuse\n'
        docker volume rm -f \
            "${cargo_registry_volume}" \
            "${cargo_git_volume}" \
            "${user_cache_volume}" \
            "${target_volume}" \
            "${fuzz_target_volume}" >/dev/null 2>&1 || true
    fi

    printf 'devcontainer validation: create contributor volumes\n'
    docker volume create "${cargo_registry_volume}" >/dev/null
    docker volume create "${cargo_git_volume}" >/dev/null
    docker volume create "${user_cache_volume}" >/dev/null
    docker volume create "${target_volume}" >/dev/null
    docker volume create "${fuzz_target_volume}" >/dev/null

    printf 'devcontainer validation: seed root-owned cache and build-output mounts\n'
    docker run --rm \
        --user root \
        --volume "${cargo_registry_volume}:/home/vscode/.cargo/registry" \
        --volume "${cargo_git_volume}:/home/vscode/.cargo/git" \
        --volume "${user_cache_volume}:/home/vscode/.cache" \
        --volume "${target_volume}:/workspaces/ffhn/target" \
        --volume "${fuzz_target_volume}:/workspaces/ffhn/fuzz/target" \
        "${image_tag}" \
        bash -lc '
            set -euo pipefail
            mkdir -p \
                /home/vscode/.cargo/registry \
                /home/vscode/.cargo/git \
                /home/vscode/.cache \
                /workspaces/ffhn/target \
                /workspaces/ffhn/fuzz/target
            touch \
                /home/vscode/.cargo/registry/root-owned \
                /home/vscode/.cargo/git/root-owned \
                /home/vscode/.cache/root-owned \
                /workspaces/ffhn/target/root-owned \
                /workspaces/ffhn/fuzz/target/root-owned
            chown -R 0:0 \
                /home/vscode/.cargo/registry \
                /home/vscode/.cargo/git \
                /home/vscode/.cache \
                /workspaces/ffhn/target \
                /workspaces/ffhn/fuzz/target
        '

    printf 'devcontainer validation: repair mounts and probe the raw contributor image\n'
    docker run --rm \
        --volume "${repo_root}:/workspaces/ffhn" \
        --volume "${cargo_registry_volume}:/home/vscode/.cargo/registry" \
        --volume "${cargo_git_volume}:/home/vscode/.cargo/git" \
        --volume "${user_cache_volume}:/home/vscode/.cache" \
        --volume "${target_volume}:/workspaces/ffhn/target" \
        --volume "${fuzz_target_volume}:/workspaces/ffhn/fuzz/target" \
        --workdir /workspaces/ffhn \
        "${image_tag}" \
        bash -lc '
            set -euo pipefail
            ./scripts/devcontainer-prepare-user-home.sh
            grep -F '\''VERSION_ID="26.04"'\'' /etc/os-release >/dev/null
            [[ "$(id -un)" == "vscode" ]]
            [[ -w "${HOME}/.cargo/registry" ]]
            [[ -w "${HOME}/.cargo/git" ]]
            [[ -w "${HOME}/.cache" ]]
            [[ -w "/workspaces/ffhn/target" ]]
            [[ -w "/workspaces/ffhn/fuzz/target" ]]
            test -f "${HOME}/.cargo/registry/root-owned"
            test -f "${HOME}/.cargo/git/root-owned"
            test -f "${HOME}/.cache/root-owned"
            test -f "/workspaces/ffhn/target/root-owned"
            test -f "/workspaces/ffhn/fuzz/target/root-owned"
            rustc --version | grep -F '\''1.95.0'\'' >/dev/null
            cargo --version >/dev/null
            cargo +nightly --version >/dev/null
            rustup target list --installed | grep -Fx '\''x86_64-unknown-linux-musl'\'' >/dev/null
            cargo nextest --version >/dev/null
            cargo audit --version >/dev/null
            cargo deny --version >/dev/null
            cargo semver-checks --version >/dev/null
            cargo outdated --version >/dev/null
            cargo llvm-cov --version >/dev/null
            cargo fuzz --version >/dev/null
            shellcheck --version >/dev/null
            gh --version >/dev/null
            clang --version >/dev/null
            ./check.sh --help >/dev/null
        '

    printf 'devcontainer validation: build helper image for devcontainer client coverage\n'
    docker build \
        --build-arg "BASE_IMAGE=${image_tag}" \
        --tag "${helper_image_tag}" \
        --file "${helper_dockerfile_path}" \
        "${repo_root}/scripts" >/dev/null

    printf 'devcontainer validation: bring up the committed devcontainer through the client path\n'
    docker run --rm \
        --volume /var/run/docker.sock:/var/run/docker.sock \
        --volume "${repo_root}:${repo_root}" \
        --workdir "${repo_root}" \
        --env FFHN_REPO_ROOT="${repo_root}" \
        --env HOME=/tmp/devcontainer-home \
        "${helper_image_tag}" bash -lc '
            set -euo pipefail
            cleanup() {
                local ids
                set +e
                ids="$(docker ps -aq --filter "label=devcontainer.local_folder=${FFHN_REPO_ROOT}")"
                if [[ -n "${ids}" ]]; then
                    docker rm -f ${ids} >/dev/null 2>&1 || true
                fi
                set -e
            }
            trap cleanup EXIT
            devcontainer up --remove-existing-container --workspace-folder "${FFHN_REPO_ROOT}" >/dev/null
            printf "devcontainer validation: run inner runtime probe through devcontainer exec\n"
            devcontainer exec --workspace-folder "${FFHN_REPO_ROOT}" ./scripts/validate-devcontainer.sh
        '

    printf 'devcontainer validation: success\n'
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
    main "$@"
fi
