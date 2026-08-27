mod check;
mod paths;
mod semver;

pub(crate) use check::{check_plan, is_semver_check_spec, semver_check_spec};
#[cfg(test)]
pub(crate) use check::{collect_shell_script_paths, shell_script_paths};
#[cfg(test)]
pub(crate) use paths::{binary_name, binary_name_for_windows_for_tests};
pub(crate) use paths::{
    cargo_build_root, cargo_target_root, core_manifest_path, coverage_build_root,
    coverage_cargo_build_dir, coverage_cargo_target_dir, coverage_target_root, fuzz_lockfile_path,
    fuzz_manifest_path, mutation_report_root, normalize_path, release_binary_path,
    semver_baseline_path, semver_baseline_target_dir, semver_build_dir, semver_scratch_dir,
};
#[cfg(test)]
pub(crate) use paths::{
    cargo_build_root_for_tests, cargo_target_root_for_tests, coverage_build_root_for_tests,
    coverage_cargo_build_dir_for_tests, coverage_cargo_target_dir_for_tests,
    coverage_target_root_for_tests, mutation_report_root_for_tests, release_binary_path_for_tests,
    semver_scratch_dir_for_tests, sibling_artifact_dir_for_tests,
    with_cargo_artifact_root_overrides,
};
#[cfg(test)]
pub(crate) use semver::{
    release_tag_exists, semver_release_type_from_git_tag, workspace_version,
    workspace_version_from_manifest,
};
pub(crate) use semver::{semver_release_type, with_workspace_stub};
