#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=scripts/common.sh
. "$(cd -- "$(dirname -- "$(printf '%s\n' "${BASH_SOURCE[0]}" | sed 's#\\#/#g')")" && pwd)/common.sh"

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

create_zip_with_dotnet() {
    local source_parent_path="$1"
    local package_root_name="$2"
    local archive_output_path="$3"

    # shellcheck disable=SC2016
    env \
        SOURCE_PARENT_PATH="$(native_path "${source_parent_path}")" \
        PACKAGE_ROOT_NAME="${package_root_name}" \
        ARCHIVE_OUTPUT_PATH="$(native_path "${archive_output_path}")" \
        powershell.exe -NoLogo -NoProfile -Command \
        '
        Add-Type -AssemblyName System.IO.Compression
        Add-Type -AssemblyName System.IO.Compression.FileSystem
        $ErrorActionPreference = "Stop"
        $sourceParent = $env:SOURCE_PARENT_PATH
        $packageRoot = $env:PACKAGE_ROOT_NAME
        $archivePath = $env:ARCHIVE_OUTPUT_PATH
        $rootPath = Join-Path $sourceParent $packageRoot

        $archive = [System.IO.Compression.ZipFile]::Open(
            $archivePath,
            [System.IO.Compression.ZipArchiveMode]::Create
        )
        try {
            Get-ChildItem -LiteralPath $rootPath -File -Recurse | ForEach-Object {
                $entryName = $_.FullName.Substring($sourceParent.Length + 1) -replace "\\", "/"
                [System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile(
                    $archive,
                    $_.FullName,
                    $entryName,
                    [System.IO.Compression.CompressionLevel]::Optimal
                ) | Out-Null
            }
        } finally {
            $archive.Dispose()
        }'
}

create_zip_with_7zip() {
    local archiver="$1"
    local source_parent_path="$2"
    local package_root_name="$3"
    local archive_output_path="$4"

    (
        cd "${source_parent_path}"
        "${archiver}" a -tzip -bd -mx=9 "$(native_path "${archive_output_path}")" "${package_root_name}" >/dev/null
    )
}

create_release_archive() {
    local source_parent_path="$1"
    local package_root_name="$2"
    local archive_output_path="$3"
    local archive_output_extension="$4"

    rm -f "${archive_output_path}"

    case "${archive_output_extension}" in
        tar.gz)
            COPYFILE_DISABLE=1 COPY_EXTENDED_ATTRIBUTES_DISABLE=1 \
                tar -C "${source_parent_path}" -czf "${archive_output_path}" "${package_root_name}"
            ;;
        zip)
            if command -v zip >/dev/null 2>&1; then
                (
                    cd "${source_parent_path}"
                    zip -qr "${archive_output_path}" "${package_root_name}"
                )
                return
            fi

            if is_windows_environment && command -v powershell.exe >/dev/null 2>&1; then
                create_zip_with_dotnet "${source_parent_path}" "${package_root_name}" "${archive_output_path}"
                return
            fi

            local seven_zip_archiver
            for seven_zip_archiver in 7z 7zz 7za; do
                if command -v "${seven_zip_archiver}" >/dev/null 2>&1; then
                    create_zip_with_7zip "${seven_zip_archiver}" "${source_parent_path}" "${package_root_name}" "${archive_output_path}"
                    return
                fi
            done

            if command -v powershell.exe >/dev/null 2>&1; then
                create_zip_with_dotnet "${source_parent_path}" "${package_root_name}" "${archive_output_path}"
                return
            fi

            ffhn_die "no ZIP archiver found (expected zip, powershell.exe, or 7z)"
            ;;
        *)
            ffhn_die "unsupported release archive extension: ${archive_output_extension}"
            ;;
    esac
}

print_usage() {
    local command_name="$1"

    cat <<EOF
Usage: ${command_name} <target-triple>

Build one maintained standalone FFHN release artifact under ./dist.

This script refuses tracked checkout drift so the compiled binary, packaged docs, and versioned
asset name all come from the same content snapshot.

Supported target triples:
$(release_target_triples | sed 's/^/  /')

Use ./scripts/release-targets.sh matrix-json to inspect the canonical release matrix.
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

    ffhn_require_clean_tracked_checkout "${repo_root}"

    local target_triple="${1:-}"
    [[ -n "${target_triple}" ]] || ffhn_usage_error "${command_name}" "target triple is required"
    is_supported_release_target "${target_triple}" || ffhn_usage_error \
        "${command_name}" \
        "unsupported release target triple: ${target_triple}"
    readonly target_triple

    local version
    version="$(ffhn_workspace_version "${script_dir}" "${repo_root}")"
    readonly version
    local artifact_name
    artifact_name="$(release_package_name_for_target "${version}" "${target_triple}")"
    readonly artifact_name
    local output_dir="${repo_root}/dist"
    readonly output_dir
    local artifact_path="${output_dir}/${artifact_name}"
    readonly artifact_path
    local cargo_profile="dist"
    readonly cargo_profile
    local deployment_target
    deployment_target="$(macos_deployment_target_for_target "${target_triple}")"
    readonly deployment_target
    local package_dir_name
    package_dir_name="$(release_package_basename_for_target "${version}" "${target_triple}")"
    readonly package_dir_name
    local archive_extension
    archive_extension="$(release_archive_extension_for_target "${target_triple}")"
    readonly archive_extension
    local compiled_binary_name
    compiled_binary_name="$(binary_name_for_target "${target_triple}")"
    readonly compiled_binary_name
    local cargo_target_root
    cargo_target_root="$(ffhn_cargo_target_dir "${repo_root}")"
    readonly cargo_target_root
    local compiled_binary_path="${cargo_target_root}/${target_triple}/${cargo_profile}/${compiled_binary_name}"
    readonly compiled_binary_path
    local temp_root
    temp_root="$(ffhn_temp_root)"
    readonly temp_root
    local staging_root
    staging_root="$(mktemp -d "${temp_root}/ffhn-release-${target_triple}.XXXXXX")"
    readonly staging_root
    local package_dir="${staging_root}/${package_dir_name}"
    readonly package_dir

    FFHN_RELEASE_ARTIFACT_STAGING_ROOT="${staging_root}"
    readonly FFHN_RELEASE_ARTIFACT_STAGING_ROOT
    trap 'rm -rf -- "${FFHN_RELEASE_ARTIFACT_STAGING_ROOT}"' EXIT

    mkdir -p "${output_dir}"
    rm -f "${artifact_path}"

    (
        cd "${repo_root}"
        if [[ -n "${deployment_target}" ]]; then
            export MACOSX_DEPLOYMENT_TARGET="${deployment_target}"
        fi
        cargo build --profile "${cargo_profile}" --locked -p ffhn-cli --bin ffhn --target "${target_triple}"
    )

    mkdir -p "${package_dir}"
    cp "${compiled_binary_path}" "${package_dir}/${compiled_binary_name}"
    if ! is_windows_environment; then
        chmod +x "${package_dir}/${compiled_binary_name}"
    fi
    cp "${repo_root}/LICENSE" "${package_dir}/LICENSE"
    cp "${repo_root}/NOTICE" "${package_dir}/NOTICE"
    cp "${repo_root}/PATENTS.md" "${package_dir}/PATENTS.md"
    cp "${repo_root}/README.md" "${package_dir}/README.md"
    cp "${repo_root}/changelog.md" "${package_dir}/changelog.md"
    create_release_archive "${staging_root}" "${package_dir_name}" "${artifact_path}" "${archive_extension}"

    printf 'Built %s for FFHN %s with Cargo profile %s\n' "${artifact_name}" "${version}" "${cargo_profile}"
    if [[ -n "${deployment_target}" ]]; then
        printf 'Pinned macOS deployment target to %s\n' "${deployment_target}"
    fi
    printf 'Wrote %s\n' "${artifact_path}"
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
    main "$@"
fi
