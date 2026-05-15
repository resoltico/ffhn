#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
readonly script_dir
readonly repo_root

readonly tooling_env_path="${FFHN_RUST_TOOLING_ENV:-${repo_root}/tooling/rust-tooling.env}"

# shellcheck disable=SC1090,SC1091
source "${tooling_env_path}"

# This entrypoint is copied into the contributor image without the rest of scripts/.
# Keep the bootstrap contract self-contained.
ffhn_scrub_ambient_native_toolchain_env() {
    unset CC CXX CLANG_BIN CPPFLAGS LDFLAGS
}

usage() {
    cat <<'EOF'
Usage:
  ./scripts/bootstrap-rust-tools.sh install-stable-toolchain
  ./scripts/bootstrap-rust-tools.sh install-coverage-toolchain
  ./scripts/bootstrap-rust-tools.sh install-toolchains
  ./scripts/bootstrap-rust-tools.sh install-cross-platform-qa-tools
  ./scripts/bootstrap-rust-tools.sh install-qa-tools
  ./scripts/bootstrap-rust-tools.sh install-all

Install the pinned Rust toolchains and Cargo QA tools owned by tooling/rust-tooling.env.
EOF
}

verify_stable_toolchain_entrypoints() {
    cargo --version >/dev/null
    rustc --version >/dev/null
}

retry() {
    local attempts="$1"
    shift
    local attempt
    for attempt in $(seq 1 "${attempts}"); do
        if "$@"; then
            return 0
        fi
        if [[ "${attempt}" -lt "${attempts}" ]]; then
            sleep 5
        fi
    done
    return 1
}

install_stable_toolchain() {
    retry 3 rustup toolchain install "${RUST_STABLE_TOOLCHAIN}" --profile minimal --component clippy --component rustfmt --component llvm-tools-preview
    retry 3 rustup default "${RUST_STABLE_TOOLCHAIN}"
    verify_stable_toolchain_entrypoints
}

install_coverage_toolchain() {
    retry 3 rustup toolchain install "${RUST_COVERAGE_TOOLCHAIN}" --profile minimal --component llvm-tools-preview
}

install_toolchains() {
    install_stable_toolchain
    install_coverage_toolchain
}

install_cross_platform_qa_tools() {
    cargo install cargo-audit --version "${CARGO_AUDIT_VERSION}" --locked
    cargo install cargo-deny --version "${CARGO_DENY_VERSION}" --locked
    cargo install cargo-nextest --version "${CARGO_NEXTEST_VERSION}" --locked
    cargo install cargo-semver-checks --version "${CARGO_SEMVER_CHECKS_VERSION}" --locked
}

install_qa_tools() {
    install_cross_platform_qa_tools
    cargo install cargo-fuzz --version "${CARGO_FUZZ_VERSION}" --locked
    cargo install cargo-llvm-cov --version "${CARGO_LLVM_COV_VERSION}" --locked
    cargo install cargo-outdated --version "${CARGO_OUTDATED_VERSION}" --locked
    retry 3 rustup default "${RUST_STABLE_TOOLCHAIN}"
}

main() {
    ffhn_scrub_ambient_native_toolchain_env

    if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
        usage
        exit 0
    fi

    case "${1:-}" in
        install-stable-toolchain)
            install_stable_toolchain
            ;;
        install-coverage-toolchain)
            install_coverage_toolchain
            ;;
        install-toolchains)
            install_toolchains
            ;;
        install-cross-platform-qa-tools)
            install_cross_platform_qa_tools
            ;;
        install-qa-tools)
            install_qa_tools
            ;;
        install-all)
            install_toolchains
            install_qa_tools
            ;;
        *)
            usage >&2
            exit 64
            ;;
    esac
}

main "${@:-}"
