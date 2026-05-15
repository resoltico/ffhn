#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=scripts/common.sh
. "$(cd -- "$(dirname -- "$(printf '%s\n' "${BASH_SOURCE[0]}" | sed 's#\\#/#g')")" && pwd)/common.sh"

release_version() {
    if [[ -n "${RELEASE_VERSION:-}" ]]; then
        printf '%s\n' "${RELEASE_VERSION}"
        return
    fi

    ffhn_workspace_version "${script_dir}" "${repo_root}"
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

    ffhn_die "no SHA-256 checksum tool found (expected sha256sum or shasum)"
}

print_usage() {
    local command_name="$1"

    cat <<EOF
Usage: ${command_name}

Write the canonical SHA-256 checksum manifest for the full maintained ./dist release inventory.

This script refuses tracked checkout drift so the expected asset names and the already-built
release artifacts are derived from one checkout state.

Expected prerequisites:
  1. ./scripts/build-release-source-archives.sh
  2. ./scripts/build-release-artifact.sh <target-triple> for every triple from
     ./scripts/release-targets.sh triples

Use ./scripts/release-targets.sh assets --version <VERSION> to inspect the expected asset names.
EOF
}

main() {
    local command_name="${BASH_SOURCE[0]}"
    local script_dir
    script_dir="$(ffhn_resolve_script_dir "${BASH_SOURCE[0]}")"
    readonly script_dir
    local repo_root
    repo_root="$(ffhn_repo_root_from_script_dir "${script_dir}")"
    readonly repo_root
    # shellcheck disable=SC1091
    . "${script_dir}/release-targets.sh"

    if ffhn_is_help_flag "${1:-}"; then
        print_usage "${command_name}"
        return 0
    fi
    [[ $# -eq 0 ]] || ffhn_usage_error "${command_name}" "this script does not accept arguments"

    local version
    version="$(release_version)"
    readonly version
    local output_dir="${repo_root}/dist"
    readonly output_dir
    local manifest_name
    manifest_name="$(release_checksum_manifest_name_for_version "${version}")"
    readonly manifest_name
    local manifest_path="${output_dir}/${manifest_name}"
    readonly manifest_path

    local expected_assets=()
    local asset_name
    while IFS= read -r asset_name; do
        expected_assets+=("${asset_name}")
    done < <(release_asset_names_for_version "${version}")
    (( ${#expected_assets[@]} > 0 )) || ffhn_die "release asset inventory is empty"

    local missing_assets=()
    for asset_name in "${expected_assets[@]}"; do
        local local_path="${output_dir}/${asset_name}"

        if [[ "${asset_name}" == "${manifest_name}" ]]; then
            continue
        fi

        if [[ ! -f "${local_path}" ]]; then
            missing_assets+=("${asset_name}")
        fi
    done

    if (( ${#missing_assets[@]} > 0 )); then
        local missing_lines=""
        for asset_name in "${missing_assets[@]}"; do
            missing_lines="${missing_lines}"$'\n'"  - ${asset_name}"
        done

        ffhn_die "$(cat <<EOF
missing maintained release assets under ${output_dir}:${missing_lines}

Populate ./dist with the full inventory first:
  1. ./scripts/build-release-source-archives.sh
  2. ./scripts/build-release-artifact.sh <target-triple> for each target from ./scripts/release-targets.sh triples
  3. ./scripts/build-release-checksums.sh
EOF
)"
    fi

    ffhn_require_clean_tracked_checkout "${repo_root}"

    mkdir -p "${output_dir}"
    : > "${manifest_path}"

    for asset_name in "${expected_assets[@]}"; do
        local local_path="${output_dir}/${asset_name}"

        if [[ "${asset_name}" == "${manifest_name}" ]]; then
            continue
        fi

        checksum_line "${local_path}" "${asset_name}" >> "${manifest_path}"
    done

    printf 'Wrote %s\n' "${manifest_path}"
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
    main "$@"
fi
