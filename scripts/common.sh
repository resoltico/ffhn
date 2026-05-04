#!/usr/bin/env bash

ffhn_die() {
    printf 'error: %s\n' "$1" >&2
    exit 1
}

ffhn_is_help_flag() {
    local candidate="${1:-}"

    [[ "${candidate}" == "-h" || "${candidate}" == "--help" ]]
}

ffhn_usage_error() {
    local command_name="$1"
    local message="$2"

    printf 'error: %s\n' "${message}" >&2
    printf 'Run %s --help for usage.\n' "${command_name}" >&2
    exit 1
}

ffhn_normalize_bash_path() {
    local candidate="$1"

    candidate="${candidate//\\//}"
    if [[ "${candidate}" =~ ^([A-Za-z]):/(.*)$ ]]; then
        local drive_letter="${BASH_REMATCH[1],,}"
        local remainder="${BASH_REMATCH[2]}"
        if [[ -n "${remainder}" ]]; then
            candidate="/${drive_letter}/${remainder}"
        else
            candidate="/${drive_letter}"
        fi
    fi

    printf '%s\n' "${candidate}"
}

ffhn_resolve_script_dir() {
    local source_path="$1"

    source_path="$(ffhn_normalize_bash_path "${source_path}")"
    while [[ -h "${source_path}" ]]; do
        local source_dir
        source_dir="$(cd -P -- "$(dirname -- "${source_path}")" && pwd)"
        source_path="$(readlink "${source_path}")"
        if [[ "${source_path}" != /* ]]; then
            source_path="${source_dir}/${source_path}"
        fi
        source_path="$(ffhn_normalize_bash_path "${source_path}")"
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

    "${helper_script_dir}/workspace-package-field.sh" version "${helper_repo_root}/Cargo.toml"
}

ffhn_workspace_description() {
    local helper_script_dir="$1"
    local helper_repo_root="$2"

    "${helper_script_dir}/workspace-package-field.sh" description "${helper_repo_root}/Cargo.toml"
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

ffhn_cargo_target_dir() {
    local repo_root="$1"
    local candidate="${CARGO_TARGET_DIR:-${repo_root}/target}"

    candidate="$(ffhn_normalize_bash_path "${candidate}")"
    if [[ "${candidate}" == /* ]]; then
        printf '%s\n' "${candidate}"
        return
    fi

    printf '%s\n' "${repo_root}/${candidate}"
}

ffhn_require_clean_tracked_checkout() {
    local repo_root="$1"

    git -C "${repo_root}" rev-parse --verify HEAD >/dev/null || ffhn_die \
        "release scripts require a git checkout with a valid HEAD commit"

    local tracked_status
    tracked_status="$(git -C "${repo_root}" status --porcelain --untracked-files=no)"
    [[ -z "${tracked_status}" ]] || ffhn_die \
        "release scripts require a clean tracked checkout because source archives ship HEAD while local packages and checksums use the current checkout; commit or stash tracked changes first"
}
