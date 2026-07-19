//! Fail-closed ownership, dependency, and source-shape verification for maintained Rust code.

mod metrics;
mod policy;

use std::collections::BTreeSet;
use std::path::Path;

use crate::model::DynResult;
use crate::repo_files::maintained_rust_source_entries_including_tests;

use self::metrics::{Metrics, direct_struct_construction_count, measured_internal_dependencies};
use self::policy::{Policy, Rule};

const POLICY_PATH: &str = "tooling/rust-source-shape-policy.toml";
const DIAGNOSTIC_CONSTRUCTION_OWNER: &str =
    "crates/ffhn-core/src/model/report/diagnostic/construction.rs";

/// Enforces the repository-owned Rust source-structure contract.
pub(crate) fn check(repo_root: &Path) -> DynResult<()> {
    let policy = Policy::load(&repo_root.join(POLICY_PATH))?;
    let findings = collect_findings(repo_root, &policy)?;

    if findings.is_empty() {
        println!("Rust source-structure gate passed.");
        return Ok(());
    }

    eprintln!("Rust source-structure gate failed:");
    for finding in findings {
        eprintln!("- {finding}");
    }
    Err("source-structure contract failed".into())
}

/// Prints the measured source shape and resolved ownership role for every maintained Rust file.
pub(crate) fn report(repo_root: &Path) -> DynResult<()> {
    let policy = Policy::load(&repo_root.join(POLICY_PATH))?;
    let mut rows = Vec::new();

    for (path, relative_path) in maintained_rust_source_entries_including_tests(repo_root)? {
        let source = std::fs::read_to_string(&path)?;
        let metrics = Metrics::from_source(&source)?;
        let role = policy
            .rule_for(&relative_path)
            .map_or("UNOWNED", Rule::role);
        rows.push((relative_path, role.to_owned(), metrics));
    }

    for (path, role, metrics) in rows {
        println!(
            "{path}\trole={role}\tlines={}\titems={}\tpublic_items={}\timports={}\tfunctions={}\tdecisions={}\tmatch_arms={}",
            metrics.physical_lines,
            metrics.item_count,
            metrics.public_item_count,
            metrics.import_count,
            metrics.function_count,
            metrics.decision_points,
            metrics.match_arms,
        );
    }
    Ok(())
}

fn collect_findings(repo_root: &Path, policy: &Policy) -> DynResult<Vec<String>> {
    let mut findings = policy.expired_rule_findings()?;
    let mut matched_rule_paths = BTreeSet::new();

    for (path, relative_path) in maintained_rust_source_entries_including_tests(repo_root)? {
        let source = std::fs::read_to_string(&path)?;
        let metrics = Metrics::from_source(&source)?;
        let dependencies = measured_internal_dependencies(&source)?;
        let Some(rule) = policy.rule_for(&relative_path) else {
            findings.push(format!(
                "{relative_path}: has no declared ownership rule in {POLICY_PATH}"
            ));
            continue;
        };

        matched_rule_paths.insert(rule.path().to_owned());
        findings.extend(rule.budget_findings(&relative_path, &metrics));
        findings.extend(rule.dependency_findings(&relative_path, &dependencies));
        findings.extend(diagnostic_construction_findings(&relative_path, &source)?);
    }

    findings.extend(policy.unmatched_rule_findings(&matched_rule_paths));
    findings.sort();
    Ok(findings)
}

/// Enforces the architectural invariant that closed diagnostic values have one FFHN-owned
/// construction boundary. Private fields are the type-level guard; this catches a future
/// visibility relaxation before it creates an alternate translation path.
fn diagnostic_construction_findings(path: &str, source: &str) -> DynResult<Vec<String>> {
    let count = direct_struct_construction_count(source, "DiagnosticDetail")?;
    if count == 0 || path == DIAGNOSTIC_CONSTRUCTION_OWNER {
        return Ok(Vec::new());
    }
    Ok(vec![format!(
        "{path}: direct DiagnosticDetail construction is owned only by {DIAGNOSTIC_CONSTRUCTION_OWNER}; use the closed diagnostic translation boundary"
    )])
}

#[cfg(test)]
mod tests;
