#!/usr/bin/env bash

set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "${script_dir}/.." && pwd)"
manifest_path="${1:-${repo_root}/Cargo.toml}"

"${script_dir}/workspace-package-field.sh" version "${manifest_path}"
