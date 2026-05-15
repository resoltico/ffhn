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

is_windows_environment() {
    [[ "${OS:-}" == "Windows_NT" ]] || command -v cygpath >/dev/null 2>&1
}

extract_zip_with_powershell() {
    local release_archive_path="$1"
    local release_extract_dir="$2"

    powershell.exe -NoLogo -NoProfile -Command \
        "Expand-Archive -Path '$(cygpath -w "${release_archive_path}")' -DestinationPath '$(cygpath -w "${release_extract_dir}")' -Force"
}

debug_release_archive_contents() {
    local release_archive_path="$1"

    printf 'Archive contents for %s:\n' "${release_archive_path}" >&2
    if command -v unzip >/dev/null 2>&1; then
        unzip -Z1 "${release_archive_path}" >&2 || true
        return
    fi

    if command -v tar >/dev/null 2>&1; then
        tar -tf "${release_archive_path}" >&2 || true
        return
    fi

    if command -v powershell.exe >/dev/null 2>&1; then
        # shellcheck disable=SC2016
        env \
            ARCHIVE_PATH="$(cygpath -w "${release_archive_path}")" \
            powershell.exe -NoLogo -NoProfile -Command \
            '
            Add-Type -AssemblyName System.IO.Compression
            $archive = [System.IO.Compression.ZipFile]::OpenRead($env:ARCHIVE_PATH)
            try {
                $archive.Entries | ForEach-Object { $_.FullName }
            } finally {
                $archive.Dispose()
            }' >&2 || true
    fi
}

debug_extracted_layout() {
    local extract_root="$1"

    printf 'Extracted layout under %s:\n' "${extract_root}" >&2
    find "${extract_root}" -mindepth 1 -maxdepth 4 -print 2>/dev/null | sort >&2 || true
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
            if command -v unzip >/dev/null 2>&1; then
                unzip -q "${release_archive_path}" -d "${release_extract_dir}"
                return
            fi

            if is_windows_environment && command -v tar >/dev/null 2>&1; then
                tar -xf "${release_archive_path}" -C "${release_extract_dir}"
                return
            fi

            if command -v powershell.exe >/dev/null 2>&1; then
                extract_zip_with_powershell "${release_archive_path}" "${release_extract_dir}"
                return
            fi

            ffhn_die "no ZIP extractor found (expected unzip, tar, or powershell.exe)"
            ;;
        *)
            ffhn_die "unsupported release archive extension: ${release_archive_extension}"
            ;;
    esac
}

assert_no_archive_metadata_sidecars() {
    local extract_root="$1"

    if find "${extract_root}" \( -name '._*' -o -name '__MACOSX' \) -print -quit | grep -q .; then
        debug_extracted_layout "${extract_root}"
        ffhn_die "release archive contains platform metadata sidecars"
    fi
}

print_usage() {
    local command_name="$1"

    cat <<EOF
Usage: ${command_name} <target-triple>

Extract one maintained ./dist release archive and verify the packaged binary and legal files.

Supported target triples:
$(release_target_triples | sed 's/^/  /')
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

    local target_triple="${1:-}"
    [[ -n "${target_triple}" ]] || ffhn_usage_error "${command_name}" "target triple is required"
    is_supported_release_target "${target_triple}" || ffhn_usage_error \
        "${command_name}" \
        "unsupported release target triple: ${target_triple}"
    readonly target_triple

    local version
    version="$(release_version)"
    readonly version
    local package_name
    package_name="$(release_package_name_for_target "${version}" "${target_triple}")"
    readonly package_name
    local package_dir_name
    package_dir_name="$(release_package_basename_for_target "${version}" "${target_triple}")"
    readonly package_dir_name
    local archive_extension
    archive_extension="$(release_archive_extension_for_target "${target_triple}")"
    readonly archive_extension
    local binary_name
    binary_name="$(binary_name_for_target "${target_triple}")"
    readonly binary_name
    local package_path="${repo_root}/dist/${package_name}"
    readonly package_path
    local temp_root
    temp_root="$(ffhn_temp_root)"
    readonly temp_root
    local extract_root
    extract_root="$(mktemp -d "${temp_root}/ffhn-smoke-${target_triple}.XXXXXX")"
    readonly extract_root
    local extracted_package_dir="${extract_root}/${package_dir_name}"
    readonly extracted_package_dir
    local binary_path="${extracted_package_dir}/${binary_name}"
    readonly binary_path

    FFHN_SMOKE_RELEASE_EXTRACT_ROOT="${extract_root}"
    readonly FFHN_SMOKE_RELEASE_EXTRACT_ROOT
    trap 'rm -rf -- "${FFHN_SMOKE_RELEASE_EXTRACT_ROOT}"' EXIT

    [[ -f "${package_path}" ]] || ffhn_die "missing package ${package_path}"

    extract_release_archive "${package_path}" "${archive_extension}" "${extract_root}"
    assert_no_archive_metadata_sidecars "${extract_root}"

    if [[ ! -f "${binary_path}" ]]; then
        debug_release_archive_contents "${package_path}"
        debug_extracted_layout "${extract_root}"
        ffhn_die "missing packaged binary ${binary_path}"
    fi
    [[ -f "${extracted_package_dir}/README.md" ]] || ffhn_die "missing packaged README.md"
    [[ -f "${extracted_package_dir}/LICENSE" ]] || ffhn_die "missing packaged LICENSE"
    [[ -f "${extracted_package_dir}/NOTICE" ]] || ffhn_die "missing packaged NOTICE"
    [[ -f "${extracted_package_dir}/PATENTS.md" ]] || ffhn_die "missing packaged PATENTS.md"
    [[ -f "${extracted_package_dir}/changelog.md" ]] || ffhn_die "missing packaged changelog.md"

    if [[ "${target_triple}" != x86_64-pc-windows-msvc ]]; then
        [[ -x "${binary_path}" ]] || ffhn_die "packaged binary is not executable: ${binary_path}"
    fi

    version_output="$("${binary_path}" --version | tr -d '\r')"
    readonly version_output

    [[ "${version_output}" == "ffhn ${version}" ]] || ffhn_die "packaged version output mismatch: ${version_output}"
    "${binary_path}" --help | tr -d '\r' | grep -q "status"

    printf 'Smoke-tested %s\n' "${package_name}"
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
    main "$@"
fi
