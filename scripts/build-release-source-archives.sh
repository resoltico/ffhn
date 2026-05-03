#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=scripts/common.sh
. "$(cd -- "$(dirname -- "$(printf '%s\n' "${BASH_SOURCE[0]}" | sed 's#\\#/#g')")" && pwd)/common.sh"

print_usage() {
    local command_name="$1"

    cat <<EOF
Usage: ${command_name}

Build the maintained FFHN source archives under ./dist from the current committed checkout.

This script refuses tracked checkout drift so the archive contents and versioned asset names come
from one immutable tree state.

Outputs:
  ffhn-source-<version>.zip
  ffhn-source-<version>.tar.gz

This uses git archive, so export-ignore rules apply to the shipped source bundles.
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

    ffhn_require_clean_tracked_checkout "${repo_root}"

    local version
    version="$(ffhn_workspace_version "${script_dir}" "${repo_root}")"
    readonly version
    local archive_basename
    archive_basename="$(release_source_archive_basename_for_version "${version}")"
    readonly archive_basename
    local output_dir="${repo_root}/dist"
    readonly output_dir
    local zip_path="${output_dir}/${archive_basename}.zip"
    readonly zip_path
    local tar_path="${output_dir}/${archive_basename}.tar.gz"
    readonly tar_path

    mkdir -p "${output_dir}"
    rm -f "${zip_path}" "${tar_path}"

    git -C "${repo_root}" archive \
        --format=zip \
        --prefix="${archive_basename}/" \
        -o "${zip_path}" \
        HEAD
    git -C "${repo_root}" archive \
        --format=tar.gz \
        --prefix="${archive_basename}/" \
        -o "${tar_path}" \
        HEAD

    printf 'Built %s.zip and %s.tar.gz from HEAD\n' "${archive_basename}" "${archive_basename}"
    printf 'Wrote %s\n' "${zip_path}"
    printf 'Wrote %s\n' "${tar_path}"
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
    main "$@"
fi
