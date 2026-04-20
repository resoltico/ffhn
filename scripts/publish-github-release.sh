#!/usr/bin/env bash

set -euo pipefail

die() {
    printf 'error: %s\n' "$1" >&2
    exit 1
}

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

script_dir="$(resolve_script_dir)"
readonly script_dir
repo_root="$(cd -P -- "${script_dir}/.." && pwd)"
readonly repo_root
# shellcheck disable=SC1091
. "${script_dir}/release-targets.sh"
tag_name="${1:-${RELEASE_TAG:-${GITHUB_REF_NAME:-}}}"
readonly tag_name
version="$(awk -F'"' '/^version = "/ {print $2; exit}' "${repo_root}/Cargo.toml")"
readonly version
readonly expected_tag="v${version}"

[[ -n "${GH_TOKEN:-}" ]] || die "GH_TOKEN is required"
[[ -n "${tag_name}" ]] || die "tag name is required"
[[ "${tag_name}" == "${expected_tag}" ]] || die "expected tag ${expected_tag}, got ${tag_name}"

release_exists() {
    gh release view "${tag_name}" >/dev/null 2>&1
}

release_has_asset() {
    local asset_name="$1"
    gh release view "${tag_name}" --json assets --jq \
        ".assets | map(.name) | index(\"${asset_name}\") != null"
}

converge_release_metadata() {
    gh release edit "${tag_name}" \
        --title "${tag_name}" \
        --draft=false \
        --prerelease=false \
        --latest \
        --verify-tag >/dev/null
}

create_or_converge_release() {
    if release_exists; then
        converge_release_metadata
        return
    fi

    if gh release create "${tag_name}" \
        --title "${tag_name}" \
        --generate-notes \
        --latest \
        --verify-tag >/dev/null 2>&1; then
        return
    fi

    release_exists || die "failed to create release ${tag_name}"
    converge_release_metadata
}

upload_if_missing() {
    local asset_path="$1"
    local asset_name
    asset_name="$(basename -- "${asset_path}")"

    [[ -f "${asset_path}" ]] || die "missing asset ${asset_path}"

    if [[ "$(release_has_asset "${asset_name}")" == "true" ]]; then
        return
    fi

    if gh release upload "${tag_name}" "${asset_path}" >/dev/null 2>&1; then
        return
    fi

    [[ "$(release_has_asset "${asset_name}")" == "true" ]] || die \
        "failed to upload ${asset_name} to release ${tag_name}"
}

create_or_converge_release

mapfile -t expected_assets < <(release_asset_names_for_version "${version}")
(( ${#expected_assets[@]} > 0 )) || die "release asset inventory is empty"

for asset_name in "${expected_assets[@]}"; do
    upload_if_missing "${repo_root}/dist/${asset_name}"
done

printf 'GitHub release publication converged for %s\n' "${tag_name}"
