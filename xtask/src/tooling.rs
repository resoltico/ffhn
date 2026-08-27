use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::model::DynResult;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CargoQaToolSpec<'a> {
    pub(crate) package_name: &'static str,
    pub(crate) subcommand_name: &'static str,
    pub(crate) expected_version: &'a str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RustTooling {
    pub(crate) workspace_edition: String,
    pub(crate) workspace_rust_version: String,
    pub(crate) stable_toolchain: String,
    pub(crate) qa_nightly_toolchain: String,
    pub(crate) cargo_audit_version: String,
    pub(crate) cargo_deny_version: String,
    pub(crate) cargo_fuzz_version: String,
    pub(crate) cargo_llvm_cov_version: String,
    pub(crate) cargo_mutants_version: String,
    pub(crate) cargo_nextest_version: String,
    pub(crate) cargo_outdated_version: String,
    pub(crate) cargo_semver_checks_version: String,
}

impl RustTooling {
    pub(crate) fn qa_nightly_toolchain_arg(&self) -> String {
        format!("+{}", self.qa_nightly_toolchain)
    }

    pub(crate) fn cargo_qa_tools(&self) -> [CargoQaToolSpec<'_>; 6] {
        [
            CargoQaToolSpec {
                package_name: "cargo-audit",
                subcommand_name: "audit",
                expected_version: &self.cargo_audit_version,
            },
            CargoQaToolSpec {
                package_name: "cargo-deny",
                subcommand_name: "deny",
                expected_version: &self.cargo_deny_version,
            },
            CargoQaToolSpec {
                package_name: "cargo-fuzz",
                subcommand_name: "fuzz",
                expected_version: &self.cargo_fuzz_version,
            },
            CargoQaToolSpec {
                package_name: "cargo-llvm-cov",
                subcommand_name: "llvm-cov",
                expected_version: &self.cargo_llvm_cov_version,
            },
            CargoQaToolSpec {
                package_name: "cargo-nextest",
                subcommand_name: "nextest",
                expected_version: &self.cargo_nextest_version,
            },
            CargoQaToolSpec {
                package_name: "cargo-semver-checks",
                subcommand_name: "semver-checks",
                expected_version: &self.cargo_semver_checks_version,
            },
        ]
    }
}

pub(crate) fn rust_tooling_path(repo_root: &Path) -> PathBuf {
    repo_root.join("tooling/rust-tooling.env")
}

pub(crate) fn rust_tooling(repo_root: &Path) -> DynResult<RustTooling> {
    let path = rust_tooling_path(repo_root);
    let text = fs::read_to_string(&path)?;
    parse_rust_tooling(&text)
        .map_err(|error| format!("{} has invalid tooling metadata: {error}", path.display()).into())
}

pub(crate) fn parse_rust_tooling(text: &str) -> Result<RustTooling, String> {
    let mut values = BTreeMap::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let (key, value) = trimmed
            .split_once('=')
            .ok_or_else(|| format!("invalid tooling line: {trimmed}"))?;
        let key = key.trim();
        let value = value.trim();
        if key.is_empty() || value.is_empty() {
            return Err(format!("invalid tooling line: {trimmed}"));
        }
        values.insert(key.to_owned(), value.to_owned());
    }

    Ok(RustTooling {
        workspace_edition: required_tooling_value(&values, "RUST_WORKSPACE_EDITION")?,
        workspace_rust_version: required_tooling_value(&values, "RUST_WORKSPACE_RUST_VERSION")?,
        stable_toolchain: required_tooling_value(&values, "RUST_STABLE_TOOLCHAIN")?,
        qa_nightly_toolchain: required_tooling_value(&values, "RUST_QA_NIGHTLY_TOOLCHAIN")?,
        cargo_audit_version: required_tooling_value(&values, "CARGO_AUDIT_VERSION")?,
        cargo_deny_version: required_tooling_value(&values, "CARGO_DENY_VERSION")?,
        cargo_fuzz_version: required_tooling_value(&values, "CARGO_FUZZ_VERSION")?,
        cargo_llvm_cov_version: required_tooling_value(&values, "CARGO_LLVM_COV_VERSION")?,
        cargo_mutants_version: required_tooling_value(&values, "CARGO_MUTANTS_VERSION")?,
        cargo_nextest_version: required_tooling_value(&values, "CARGO_NEXTEST_VERSION")?,
        cargo_outdated_version: required_tooling_value(&values, "CARGO_OUTDATED_VERSION")?,
        cargo_semver_checks_version: required_tooling_value(
            &values,
            "CARGO_SEMVER_CHECKS_VERSION",
        )?,
    })
}

fn required_tooling_value(values: &BTreeMap<String, String>, key: &str) -> Result<String, String> {
    values
        .get(key)
        .cloned()
        .ok_or_else(|| format!("missing {key}"))
}
