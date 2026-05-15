#!/usr/bin/env bash
set -euo pipefail

# Stable maintainer entrypoint: keep local docs, CI muscle memory, and the Rust gate on one path.
repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
cd "$repo_root"
unset CC CXX CLANG_BIN CPPFLAGS LDFLAGS

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
    cat <<'EOF'
Usage: ./check.sh

Run FFHN's maintained local quality gate through `cargo xtask check`.
EOF
    exit 0
fi

cargo run -p xtask -- check
