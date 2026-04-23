#!/usr/bin/env bash

ffhn_die() {
    printf 'error: %s\n' "$1" >&2
    exit 1
}

ffhn_resolve_script_dir() {
    local source_path="$1"

    while [[ -h "${source_path}" ]]; do
        local source_dir
        source_dir="$(cd -P -- "$(dirname -- "${source_path}")" && pwd)"
        source_path="$(readlink "${source_path}")"
        if [[ "${source_path}" != /* ]]; then
            source_path="${source_dir}/${source_path}"
        fi
    done

    cd -P -- "$(dirname -- "${source_path}")" && pwd
}

ffhn_repo_root_from_script_dir() {
    local helper_script_dir="$1"

    cd -P -- "${helper_script_dir}/.." && pwd
}

ffhn_workspace_version() {
    local helper_script_dir="$1"
    local helper_repo_root="$2"

    "${helper_script_dir}/workspace-version.sh" "${helper_repo_root}/Cargo.toml"
}

ffhn_temp_root() {
    local candidate="${RUNNER_TEMP:-${TMPDIR:-${TEMP:-${TMP:-/tmp}}}}"

    if command -v cygpath >/dev/null 2>&1; then
        case "${candidate}" in
            [A-Za-z]:\\*|[A-Za-z]:/*)
                candidate="$(cygpath -u "${candidate}")"
                ;;
        esac
    fi

    printf '%s\n' "${candidate}"
}
