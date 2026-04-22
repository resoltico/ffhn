#!/usr/bin/env bash

set -euo pipefail

resolve_script_dir() {
    local source_path="${BASH_SOURCE[0]}"
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

die() {
    printf 'error: %s\n' "$1" >&2
    exit 1
}

workspace_version() {
    "${script_dir}/workspace-version.sh" "${repo_root}/Cargo.toml"
}

release_version() {
    if [[ -n "${RELEASE_VERSION:-}" ]]; then
        printf '%s\n' "${RELEASE_VERSION}"
        return
    fi

    workspace_version
}

is_windows_environment() {
    [[ "${OS:-}" == "Windows_NT" ]] || command -v cygpath >/dev/null 2>&1
}

native_path() {
    local path="$1"

    if command -v cygpath >/dev/null 2>&1; then
        cygpath -w "${path}"
        return
    fi

    printf '%s\n' "${path}"
}

extract_release_archive() {
    local release_archive_path="$1"
    local release_archive_extension="$2"
    local release_extract_dir="$3"

    case "${release_archive_extension}" in
        tar.gz)
            tar -xzf "${release_archive_path}" -C "${release_extract_dir}"
            ;;
        zip)
            if is_windows_environment && command -v powershell.exe >/dev/null 2>&1; then
                powershell.exe -NoLogo -NoProfile -Command \
                    "Expand-Archive -Path '$(native_path "${release_archive_path}")' -DestinationPath '$(native_path "${release_extract_dir}")' -Force"
                return
            fi

            if command -v unzip >/dev/null 2>&1; then
                unzip -q "${release_archive_path}" -d "${release_extract_dir}"
                return
            fi

            if command -v powershell.exe >/dev/null 2>&1; then
                powershell.exe -NoLogo -NoProfile -Command \
                    "Expand-Archive -Path '$(native_path "${release_archive_path}")' -DestinationPath '$(native_path "${release_extract_dir}")' -Force"
                return
            fi

            die "no ZIP extractor found (expected unzip or powershell.exe)"
            ;;
        *)
            die "unsupported release archive extension: ${release_archive_extension}"
            ;;
    esac
}

script_dir="$(resolve_script_dir)"
readonly script_dir
repo_root="$(cd -P -- "${script_dir}/.." && pwd)"
readonly repo_root
# shellcheck disable=SC1091
. "${script_dir}/release-targets.sh"

target_triple="${1:-}"
readonly target_triple
[[ -n "${target_triple}" ]] || die "target triple is required"
is_supported_release_target "${target_triple}" || die "unsupported release target triple: ${target_triple}"

version="$(release_version)"
readonly version
package_name="$(release_package_name_for_target "${version}" "${target_triple}")"
readonly package_name
package_dir_name="$(release_package_basename_for_target "${version}" "${target_triple}")"
readonly package_dir_name
archive_extension="$(release_archive_extension_for_target "${target_triple}")"
readonly archive_extension
binary_name="$(binary_name_for_target "${target_triple}")"
readonly binary_name
package_path="${repo_root}/dist/${package_name}"
readonly package_path
extract_root="$(mktemp -d "${TMPDIR:-/tmp}/ffhn-smoke-${target_triple}.XXXXXX")"
readonly extract_root
extracted_package_dir="${extract_root}/${package_dir_name}"
readonly extracted_package_dir
binary_path="${extracted_package_dir}/${binary_name}"
readonly binary_path

cleanup() {
    rm -rf "${extract_root}"
}

trap cleanup EXIT

[[ -f "${package_path}" ]] || die "missing package ${package_path}"

extract_release_archive "${package_path}" "${archive_extension}" "${extract_root}"

[[ -f "${binary_path}" ]] || die "missing packaged binary ${binary_path}"
[[ -f "${extracted_package_dir}/README.md" ]] || die "missing packaged README.md"
[[ -f "${extracted_package_dir}/LICENSE" ]] || die "missing packaged LICENSE"

if [[ "${target_triple}" != x86_64-pc-windows-msvc ]]; then
    [[ -x "${binary_path}" ]] || die "packaged binary is not executable: ${binary_path}"
fi

"${binary_path}" --version | tr -d '\r' | grep "^ffhn ${version}$"
"${binary_path}" --help | tr -d '\r' | grep "status"

printf 'Smoke-tested %s\n' "${package_name}"
