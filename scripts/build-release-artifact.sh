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
    awk -F'"' '/^version = "/ {print $2; exit}' "${repo_root}/Cargo.toml"
}

checksum_file() {
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
target_triple="${1:-}"
readonly target_triple

[[ -n "${target_triple}" ]] || die "target triple is required"
is_supported_release_target "${target_triple}" || die "unsupported release target triple: ${target_triple}"

version="$(workspace_version)"
readonly version
artifact_name="$(artifact_name_for_target "${target_triple}")"
readonly artifact_name
readonly output_dir="${repo_root}/dist"
readonly artifact_path="${output_dir}/${artifact_name}"
readonly checksum_path="${artifact_path}.sha256"
readonly cargo_profile="dist"
deployment_target="$(macos_deployment_target_for_target "${target_triple}")"
readonly deployment_target

mkdir -p "${output_dir}"
rm -f "${artifact_path}" "${checksum_path}"

(
    cd "${repo_root}"
    if [[ -n "${deployment_target}" ]]; then
        export MACOSX_DEPLOYMENT_TARGET="${deployment_target}"
    fi
    cargo build --profile "${cargo_profile}" --locked -p ffhn-cli --bin ffhn --target "${target_triple}"
)

cp "${repo_root}/target/${target_triple}/${cargo_profile}/ffhn$(binary_suffix_for_target "${target_triple}")" "${artifact_path}"
chmod +x "${artifact_path}"
checksum_file "${artifact_path}" "${artifact_name}" > "${checksum_path}"

printf 'Built %s for FFHN %s with Cargo profile %s\n' "${artifact_name}" "${version}" "${cargo_profile}"
if [[ -n "${deployment_target}" ]]; then
    printf 'Pinned macOS deployment target to %s\n' "${deployment_target}"
fi
printf 'Wrote %s\n' "${checksum_path}"
