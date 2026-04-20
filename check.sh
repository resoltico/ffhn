#!/usr/bin/env bash
set -euo pipefail

# Stable maintainer entrypoint: keep local docs, CI muscle memory, and the Rust gate on one path.
repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
cd "$repo_root"

cargo run --quiet -p xtask -- check
