#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=scripts/common.sh
. "$(cd -- "$(dirname -- "$(printf '%s\n' "${BASH_SOURCE[0]}" | sed 's#\\#/#g')")" && pwd)/common.sh"

release_version() {
    if [[ -n "${RELEASE_VERSION:-}" ]]; then
        printf '%s\n' "${RELEASE_VERSION}"
        return
    fi

    "${script_dir}/workspace-version.sh" "${repo_root}/Cargo.toml"
}

release_exists() {
    gh release view "${tag_name}" >/dev/null 2>&1
}

release_has_asset() {
    local asset_name="$1"
    gh release view "${tag_name}" --json assets --jq \
        ".assets | map(.name) | index(\"${asset_name}\") != null"
}

release_is_draft() {
    gh release view "${tag_name}" --json isDraft --jq '.isDraft'
}

publish_release() {
    gh release edit "${tag_name}" \
        --title "${tag_name}" \
        --draft=false \
        --prerelease=false \
        --latest \
        --verify-tag >/dev/null
}

ensure_release_draft_exists() {
    if release_exists; then
        return
    fi

    if gh release create "${tag_name}" \
        --title "${tag_name}" \
        --generate-notes \
        --draft \
        --verify-tag >/dev/null 2>&1; then
        return
    fi

    release_exists || ffhn_die "failed to create draft release ${tag_name}"
}

ensure_release_is_uploadable() {
    local asset_name="$1"

    if [[ "$(release_is_draft)" == "true" ]]; then
        return
    fi

    if [[ "$(release_has_asset "${asset_name}")" == "true" ]]; then
        return
    fi

    ffhn_die \
        "release ${tag_name} is already published and missing ${asset_name}; refusing to mutate a published release"
}

upload_if_missing() {
    local asset_path="$1"
    local asset_name
    asset_name="$(basename -- "${asset_path}")"

    [[ -f "${asset_path}" ]] || ffhn_die "missing asset ${asset_path}"

    if [[ "$(release_has_asset "${asset_name}")" == "true" ]]; then
        return
    fi

    ensure_release_is_uploadable "${asset_name}"

    if gh release upload "${tag_name}" "${asset_path}" >/dev/null 2>&1; then
        return
    fi

    [[ "$(release_has_asset "${asset_name}")" == "true" ]] || ffhn_die \
        "failed to upload ${asset_name} to release ${tag_name}"
}

print_usage() {
    local command_name="$1"

    cat <<EOF
Usage: ${command_name} [tag-name]

Publish or converge the GitHub release object for one maintained FFHN tag.

This script refuses tracked checkout drift so the release tag, expected asset inventory, and
uploaded ./dist artifacts are derived from one checkout state.

Inputs:
  tag-name             Optional release tag such as v${version}. Defaults to
                       RELEASE_TAG, then GITHUB_REF_NAME.

Required environment:
  GH_TOKEN             GitHub token accepted by the gh CLI.
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
    local version
    version="$(release_version)"
    readonly version

    if ffhn_is_help_flag "${1:-}"; then
        print_usage "${command_name}"
        return 0
    fi

    ffhn_require_clean_tracked_checkout "${repo_root}"

    local tag_name="${1:-${RELEASE_TAG:-${GITHUB_REF_NAME:-}}}"
    readonly tag_name
    local expected_tag="v${version}"
    readonly expected_tag

    [[ -n "${GH_TOKEN:-}" ]] || ffhn_die "GH_TOKEN is required"
    [[ -n "${tag_name}" ]] || ffhn_die "tag name is required"
    [[ "${tag_name}" == "${expected_tag}" ]] || ffhn_die \
        "expected tag ${expected_tag}, got ${tag_name}"

    ensure_release_draft_exists

    mapfile -t expected_assets < <(release_asset_names_for_version "${version}")
    (( ${#expected_assets[@]} > 0 )) || ffhn_die "release asset inventory is empty"

    for asset_name in "${expected_assets[@]}"; do
        upload_if_missing "${repo_root}/dist/${asset_name}"
    done

    publish_release

    printf 'GitHub release publication converged for %s\n' "${tag_name}"
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
    main "$@"
fi
