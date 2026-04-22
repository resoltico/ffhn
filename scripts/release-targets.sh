#!/usr/bin/env bash

release_target_triples() {
    cat <<'EOF'
aarch64-apple-darwin
x86_64-apple-darwin
x86_64-unknown-linux-musl
x86_64-pc-windows-msvc
EOF
}

is_supported_release_target() {
    local candidate="$1"
    local listed_target

    while IFS= read -r listed_target; do
        if [[ "${listed_target}" == "${candidate}" ]]; then
            return 0
        fi
    done < <(release_target_triples)

    return 1
}

release_matrix_json() {
    cat <<'EOF'
{"include":[
  {
    "id":"macos-arm64",
    "runs_on":"macos-15",
    "target_triple":"aarch64-apple-darwin",
    "artifact_bundle_name":"standalone-macos-arm64",
    "needs_musl_tools":false
  },
  {
    "id":"macos-x64",
    "runs_on":"macos-15-intel",
    "target_triple":"x86_64-apple-darwin",
    "artifact_bundle_name":"standalone-macos-x64",
    "needs_musl_tools":false
  },
  {
    "id":"linux-x64-musl",
    "runs_on":"ubuntu-24.04",
    "target_triple":"x86_64-unknown-linux-musl",
    "artifact_bundle_name":"standalone-linux-x64-musl",
    "needs_musl_tools":true
  },
  {
    "id":"windows-x64",
    "runs_on":"windows-2022",
    "target_triple":"x86_64-pc-windows-msvc",
    "artifact_bundle_name":"standalone-windows-x64",
    "needs_musl_tools":false
  }
]}
EOF
}

binary_suffix_for_target() {
    local requested_target="$1"

    case "${requested_target}" in
        x86_64-pc-windows-msvc)
            printf '.exe\n'
            ;;
        *)
            printf '\n'
            ;;
    esac
}

binary_name_for_target() {
    local requested_target="$1"

    printf 'ffhn%s\n' "$(binary_suffix_for_target "${requested_target}")"
}

release_archive_extension_for_target() {
    local requested_target="$1"

    case "${requested_target}" in
        x86_64-pc-windows-msvc)
            printf 'zip\n'
            ;;
        *)
            printf 'tar.gz\n'
            ;;
    esac
}

release_package_basename_for_target() {
    local release_version="$1"
    local requested_target="$2"

    printf 'ffhn-%s-%s\n' "${release_version}" "${requested_target}"
}

release_package_name_for_target() {
    local release_version="$1"
    local requested_target="$2"

    printf '%s.%s\n' \
        "$(release_package_basename_for_target "${release_version}" "${requested_target}")" \
        "$(release_archive_extension_for_target "${requested_target}")"
}

release_source_archive_basename_for_version() {
    local release_version="$1"

    printf 'ffhn-source-%s\n' "${release_version}"
}

release_source_archive_names_for_version() {
    local release_version="$1"
    local basename

    basename="$(release_source_archive_basename_for_version "${release_version}")"
    printf '%s.zip\n' "${basename}"
    printf '%s.tar.gz\n' "${basename}"
}

release_checksum_manifest_name_for_version() {
    local release_version="$1"

    printf 'ffhn-%s-checksums.txt\n' "${release_version}"
}

macos_deployment_target_for_target() {
    local requested_target="$1"

    case "${requested_target}" in
        aarch64-apple-darwin|x86_64-apple-darwin)
            printf '12.0\n'
            ;;
        *)
            printf '\n'
            ;;
    esac
}

release_asset_names_for_version() {
    local release_version="$1"
    local listed_target

    release_source_archive_names_for_version "${release_version}"

    while IFS= read -r listed_target; do
        printf '%s\n' "$(release_package_name_for_target "${release_version}" "${listed_target}")"
    done < <(release_target_triples)

    printf '%s\n' "$(release_checksum_manifest_name_for_version "${release_version}")"
}
