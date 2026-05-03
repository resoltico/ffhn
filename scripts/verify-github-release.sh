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

print_usage() {
    local command_name="$1"

    cat <<EOF
Usage: ${command_name} [tag-name]

Verify the published GitHub release object for one maintained FFHN tag.

This script refuses tracked checkout drift so the expected tag and maintained asset inventory are
derived from one checkout state.

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

    local release_tag
    release_tag="$(gh release view "${tag_name}" --json tagName --jq '.tagName')"
    [[ "${release_tag}" == "${tag_name}" ]] || ffhn_die \
        "expected release tag ${tag_name}, got ${release_tag}"

    local is_draft
    is_draft="$(gh release view "${tag_name}" --json isDraft --jq '.isDraft')"
    [[ "${is_draft}" == "false" ]] || ffhn_die "release ${tag_name} is a draft"

    local is_prerelease
    is_prerelease="$(gh release view "${tag_name}" --json isPrerelease --jq '.isPrerelease')"
    [[ "${is_prerelease}" == "false" ]] || ffhn_die "release ${tag_name} is marked prerelease"

    local expected_assets=()
    local asset_name
    while IFS= read -r asset_name; do
        expected_assets+=("${asset_name}")
    done < <(release_asset_names_for_version "${version}")
    (( ${#expected_assets[@]} > 0 )) || ffhn_die "release asset inventory is empty"

    for asset_name in "${expected_assets[@]}"; do
        local has_asset
        has_asset="$(gh release view "${tag_name}" --json assets --jq \
            ".assets | map(.name) | index(\"${asset_name}\") != null")"
        [[ "${has_asset}" == "true" ]] || ffhn_die \
            "release ${tag_name} is missing required asset ${asset_name}"
    done

    local release_url
    release_url="$(gh release view "${tag_name}" --json url --jq '.url')"
    printf 'Verified GitHub release handoff: %s\n' "${release_url}"
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
    main "$@"
fi
