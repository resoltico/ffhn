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

artifact_name_for_target() {
    local requested_target="$1"

    printf 'ffhn-%s%s\n' "${requested_target}" "$(binary_suffix_for_target "${requested_target}")"
}

checksum_name_for_target() {
    local requested_target="$1"

    printf '%s.sha256\n' "$(artifact_name_for_target "${requested_target}")"
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

    printf 'ffhn-%s.zip\n' "${release_version}"
    printf 'ffhn-%s.tar.gz\n' "${release_version}"

    while IFS= read -r listed_target; do
        printf '%s\n' "$(artifact_name_for_target "${listed_target}")"
        printf '%s\n' "$(checksum_name_for_target "${listed_target}")"
    done < <(release_target_triples)
}
