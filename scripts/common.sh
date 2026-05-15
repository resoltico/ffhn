#!/usr/bin/env bash

ffhn_die() {
    printf 'error: %s\n' "$1" >&2
    exit 1
}

ffhn_is_help_flag() {
    local candidate="${1:-}"

    [[ "${candidate}" == "-h" || "${candidate}" == "--help" ]]
}

ffhn_usage_error() {
    local command_name="$1"
    local message="$2"

    printf 'error: %s\n' "${message}" >&2
    printf 'Run %s --help for usage.\n' "${command_name}" >&2
    exit 1
}

ffhn_normalize_bash_path() {
    local candidate="$1"

    candidate="${candidate//\\//}"
    if [[ "${candidate}" =~ ^([A-Za-z]):/(.*)$ ]]; then
        local drive_letter="${BASH_REMATCH[1],,}"
        local remainder="${BASH_REMATCH[2]}"
        if [[ -n "${remainder}" ]]; then
            candidate="/${drive_letter}/${remainder}"
        else
            candidate="/${drive_letter}"
        fi
    fi

    printf '%s\n' "${candidate}"
}

ffhn_join_and_normalize_bash_path() {
    local base_path="$1"
    local relative_path="$2"

    python3 - <<'PY' "${base_path}" "${relative_path}"
import posixpath
import sys

base_path = sys.argv[1]
relative_path = sys.argv[2]

print(posixpath.normpath(posixpath.join(base_path, relative_path)))
PY
}

ffhn_path_for_host_python() {
    local candidate="$1"

    if command -v cygpath >/dev/null 2>&1; then
        case "${candidate}" in
            /[A-Za-z]/*)
                candidate="$(cygpath -w "${candidate}")"
                ;;
        esac
    fi

    printf '%s\n' "${candidate}"
}

ffhn_resolve_script_dir() {
    local source_path="$1"

    source_path="$(ffhn_normalize_bash_path "${source_path}")"
    while [[ -h "${source_path}" ]]; do
        local source_dir
        source_dir="$(cd -P -- "$(dirname -- "${source_path}")" && pwd)"
        source_path="$(readlink "${source_path}")"
        if [[ "${source_path}" != /* ]]; then
            source_path="${source_dir}/${source_path}"
        fi
        source_path="$(ffhn_normalize_bash_path "${source_path}")"
    done

    cd -P -- "$(dirname -- "${source_path}")" && pwd
}

ffhn_repo_root_from_script_dir() {
    local helper_script_dir="$1"

    cd -P -- "${helper_script_dir}/.." && pwd
}

ffhn_workspace_version() {
    local helper_script_dir="$1"
    local helper_repo_root="$2"

    "${helper_script_dir}/workspace-package-field.sh" version "${helper_repo_root}/Cargo.toml"
}

ffhn_workspace_description() {
    local helper_script_dir="$1"
    local helper_repo_root="$2"

    "${helper_script_dir}/workspace-package-field.sh" description "${helper_repo_root}/Cargo.toml"
}

ffhn_temp_root() {
    local candidate="${RUNNER_TEMP:-${TMPDIR:-${TEMP:-${TMP:-/tmp}}}}"

    if command -v cygpath >/dev/null 2>&1; then
        case "${candidate}" in
            [A-Za-z]:\\*|[A-Za-z]:/*)
                candidate="$(cygpath -u "${candidate}")"
                ;;
        esac
    fi

    printf '%s\n' "${candidate}"
}

ffhn_cargo_build_config_value() {
    local repo_root="$1"
    local field_name="$2"
    local python_repo_root

    python_repo_root="$(ffhn_path_for_host_python "${repo_root}")"

    python3 - <<'PY' "${python_repo_root}" "${field_name}"
import ast
import pathlib
import sys

try:
    import tomllib  # type: ignore[attr-defined]
except ModuleNotFoundError:
    tomllib = None

repo_root = pathlib.Path(sys.argv[1])
field_name = sys.argv[2]
config_path = repo_root / ".cargo" / "config.toml"

if not config_path.is_file():
    raise SystemExit(0)

text = config_path.read_text()

if tomllib is not None:
    document = tomllib.loads(text)
else:
    document = {"build": {}}
    in_build = False
    for raw_line in text.splitlines():
        line = raw_line.split("#", 1)[0].strip()
        if not line:
            continue
        if line.startswith("[") and line.endswith("]"):
            in_build = line[1:-1].strip() == "build"
            continue
        if not in_build or "=" not in line:
            continue
        key, raw_value = line.split("=", 1)
        if key.strip() != field_name:
            continue
        raw_value = raw_value.strip()
        if len(raw_value) >= 2 and raw_value[0] == raw_value[-1] == '"':
            document["build"][field_name] = ast.literal_eval(raw_value)
        elif len(raw_value) >= 2 and raw_value[0] == raw_value[-1] == "'":
            document["build"][field_name] = raw_value[1:-1]
        break

value = document.get("build", {}).get(field_name)
if isinstance(value, str):
    print(value)
PY
}

ffhn_cargo_target_dir() {
    local repo_root="$1"
    local candidate="${CARGO_TARGET_DIR:-}"

    if [[ -z "${candidate}" ]]; then
        candidate="$(ffhn_cargo_build_config_value "${repo_root}" "target-dir")"
    fi
    if [[ -z "${candidate}" ]]; then
        candidate="${repo_root}/target"
    fi

    candidate="$(ffhn_normalize_bash_path "${candidate}")"
    if [[ "${candidate}" == /* ]]; then
        printf '%s\n' "${candidate}"
        return
    fi

    printf '%s\n' "$(ffhn_join_and_normalize_bash_path "${repo_root}" "${candidate}")"
}

ffhn_require_clean_tracked_checkout() {
    local repo_root="$1"

    git -C "${repo_root}" rev-parse --verify HEAD >/dev/null || ffhn_die \
        "release scripts require a git checkout with a valid HEAD commit"

    local tracked_status
    tracked_status="$(git -C "${repo_root}" status --porcelain --untracked-files=no)"
    [[ -z "${tracked_status}" ]] || ffhn_die \
        "release scripts require a clean tracked checkout because source archives ship HEAD while local packages and checksums use the current checkout; commit or stash tracked changes first"
}

ffhn_docker_build() {
    local progress="${FFHN_DOCKER_BUILD_PROGRESS:-plain}"

    docker build --progress "${progress}" "$@"
}

ffhn_scrub_ambient_native_toolchain_env() {
    unset CC CXX CLANG_BIN CPPFLAGS LDFLAGS
}
