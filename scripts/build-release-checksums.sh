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

checksum_line() {
    local file_path="$1"
    local asset_basename="$2"

    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "${file_path}" | awk -v name="${asset_basename}" '{print $1 "  " name}'
        return
    fi

    if command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "${file_path}" | awk -v name="${asset_basename}" '{print $1 "  " name}'
        return
    fi

    die "no SHA-256 checksum tool found (expected sha256sum or shasum)"
}

script_dir="$(resolve_script_dir)"
readonly script_dir
repo_root="$(cd -P -- "${script_dir}/.." && pwd)"
readonly repo_root
# shellcheck disable=SC1091
. "${script_dir}/release-targets.sh"

version="$(release_version)"
readonly version
output_dir="${repo_root}/dist"
readonly output_dir
manifest_name="$(release_checksum_manifest_name_for_version "${version}")"
readonly manifest_name
manifest_path="${output_dir}/${manifest_name}"
readonly manifest_path

mkdir -p "${output_dir}"
: > "${manifest_path}"

mapfile -t expected_assets < <(release_asset_names_for_version "${version}")
(( ${#expected_assets[@]} > 0 )) || die "release asset inventory is empty"

for asset_name in "${expected_assets[@]}"; do
    local_path="${output_dir}/${asset_name}"

    if [[ "${asset_name}" == "${manifest_name}" ]]; then
        continue
    fi

    [[ -f "${local_path}" ]] || die "missing asset ${local_path}"
    checksum_line "${local_path}" "${asset_name}" >> "${manifest_path}"
done

printf 'Wrote %s\n' "${manifest_path}"
