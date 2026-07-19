#!/usr/bin/env bash
set -euo pipefail

# Stable maintainer entrypoint: keep local docs, CI muscle memory, and the Rust gate on one path.
repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
cd "$repo_root"
unset CC CXX CLANG_BIN CPPFLAGS LDFLAGS

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
    cat <<'EOF'
Usage: ./check.sh [--format <human|json>] [--verbosity <concise|verbose>] [--log-dir <DIRECTORY>] [--retain-passing-logs]

Run FFHN's maintained local quality gate through `cargo xtask check`. Options are forwarded to
the gate event renderer; concise human output is the default.
EOF
    exit 0
fi

cargo run -p xtask -- check "$@"
