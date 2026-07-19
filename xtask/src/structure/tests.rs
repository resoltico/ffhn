use std::fs;

use tempfile::TempDir;

use super::metrics::{Metrics, direct_struct_construction_count, measured_internal_dependencies};
use super::policy::Policy;
use super::{check, collect_findings, report};

const RULE: &str = r#"
version = 1

[[rules]]
path = "crates/ffhn-core/src/"
match = "prefix"
role = "test-role"
owner = "test owner"
rationale = "test rationale"
split_trigger = "test trigger"
max_physical_lines = 20
max_items = 20
max_public_items = 20
max_imports = 20
max_functions = 20
max_decision_points = 20
max_match_arms = 20
allowed_internal_dependencies = ["model"]

[[rules]]
path = "crates/ffhn-cli/src/"
match = "prefix"
role = "test-role"
owner = "test owner"
rationale = "test rationale"
split_trigger = "test trigger"
max_physical_lines = 20
max_items = 20
max_public_items = 20
max_imports = 20
max_functions = 20
max_decision_points = 20
max_match_arms = 20
allowed_internal_dependencies = []

[[rules]]
path = "xtask/src/"
match = "prefix"
role = "test-role"
owner = "test owner"
rationale = "test rationale"
split_trigger = "test trigger"
max_physical_lines = 20
max_items = 20
max_public_items = 20
max_imports = 20
max_functions = 20
max_decision_points = 20
max_match_arms = 20
allowed_internal_dependencies = []
"#;

#[test]
fn measures_ast_shape_without_counting_comments_as_items() {
    let source = r#"
        // A comment is a physical line only.
        use crate::model::Thing;
        pub struct Public;
        fn private() {
            if true && false {
                match Thing::one() {
                    _ => {}
                }
            }
        }
    "#;

    let metrics = Metrics::from_source(source).expect("metrics");

    assert_eq!(metrics.physical_lines, 12);
    assert_eq!(metrics.item_count, 3);
    assert_eq!(metrics.public_item_count, 1);
    assert_eq!(metrics.import_count, 1);
    assert_eq!(metrics.function_count, 1);
    assert_eq!(metrics.decision_points, 3);
    assert_eq!(metrics.match_arms, 1);
}

#[test]
fn measures_every_declared_shape_dimension_from_the_rust_ast() {
    let source = r#"
        use crate::{model::Thing, Stable as Alias, *};
        use serde::{Deserialize, *};
        use serde::Serialize as Ser;

        pub const CONSTANT: usize = 0;
        pub enum PublicEnum {}
        pub extern crate core;
        pub fn public_function() {}
        pub mod public_module {}
        pub static PUBLIC_STATIC: usize = 0;
        pub struct PublicStruct;
        pub trait PublicTrait { fn trait_method(&self); }
        pub trait PublicAlias = Sized;
        pub type PublicType = usize;
        pub union PublicUnion { field: u8 }
        pub use crate::model::Thing as Reexport;
        impl PublicStruct { pub fn method(&self) {} }
        unsafe extern "C" { fn foreign_function(); }
        macro_rules! local { () => {}; }

        fn decisions(values: &[u8]) {
            if true || false {}
            for _ in values {}
            loop { break; }
            while false {}
            match values.len() {
                0 => {},
                _ => {},
            }
        }
    "#;

    let metrics = Metrics::from_source(source).expect("metrics");

    assert_eq!(metrics.import_count, 4);
    assert!(metrics.item_count >= 16);
    assert!(metrics.public_item_count >= 11);
    assert_eq!(metrics.function_count, 4);
    assert_eq!(metrics.decision_points, 7);
    assert_eq!(metrics.match_arms, 2);
}

#[test]
fn rejects_unparseable_source_for_shape_and_dependency_measurement() {
    assert!(Metrics::from_source("fn incomplete(").is_err());
    assert!(measured_internal_dependencies("use crate::{").is_err());
}

#[test]
fn extracts_only_direct_internal_crate_dependencies() {
    let dependencies = measured_internal_dependencies(
        "use crate::{model::Thing, stable_json}; use serde::Serialize; use super::local;",
    )
    .expect("dependencies");

    assert_eq!(
        dependencies.into_iter().collect::<Vec<_>>(),
        ["crate", "model"]
    );
}

#[test]
fn recognizes_direct_struct_construction_from_the_rust_ast() {
    let source = "fn build() { let _ = DiagnosticDetail {}; }";
    assert_eq!(
        direct_struct_construction_count(source, "DiagnosticDetail").expect("construction count"),
        1
    );
    assert_eq!(
        direct_struct_construction_count(
            "// DiagnosticDetail {}\nfn build() {}",
            "DiagnosticDetail"
        )
        .expect("construction count"),
        0
    );
    assert_eq!(
        direct_struct_construction_count("fn build() { let _ = Other {}; }", "DiagnosticDetail")
            .expect("construction count"),
        0
    );
}

#[test]
fn rejects_diagnostic_construction_outside_its_owned_translation_module() {
    let directory = TempDir::new().expect("temporary directory");
    let source = directory
        .path()
        .join("crates/ffhn-core/src/model/unowned_construction.rs");
    fs::create_dir_all(source.parent().expect("parent")).expect("directories");
    fs::write(source, "fn build() { let _ = DiagnosticDetail {}; }\n").expect("source");
    let policy = write_policy(&directory, &format!("{RULE}\n"));

    let findings = collect_findings(directory.path(), &policy).expect("findings");

    assert!(findings.iter().any(|finding| {
        finding.contains("direct DiagnosticDetail construction is owned only by")
    }));
}

#[test]
fn permits_diagnostic_construction_inside_its_owned_translation_module() {
    let directory = TempDir::new().expect("temporary directory");
    let source = directory.path().join(super::DIAGNOSTIC_CONSTRUCTION_OWNER);
    fs::create_dir_all(source.parent().expect("parent")).expect("directories");
    fs::write(source, "fn build() { let _ = DiagnosticDetail {}; }\n").expect("source");
    let policy = write_policy(&directory, &format!("{RULE}\n"));

    let findings = collect_findings(directory.path(), &policy).expect("findings");

    assert!(
        findings
            .iter()
            .all(|finding| !finding.contains("direct DiagnosticDetail construction"))
    );
}

#[test]
fn dependency_measurement_distinguishes_the_public_facade_from_named_modules() {
    let dependencies = measured_internal_dependencies(
        "use crate::{model::Thing, Stable as Alias, *}; use serde::{Deserialize, *}; use serde::Serialize as Ser;",
    )
    .expect("dependencies");

    assert_eq!(
        dependencies.into_iter().collect::<Vec<_>>(),
        ["crate", "model"]
    );
}

#[test]
fn most_specific_matching_rule_owns_a_file() {
    let directory = TempDir::new().expect("temporary directory");
    let policy_path = directory.path().join("policy.toml");
    fs::write(
        &policy_path,
        format!(
            "{RULE}\n[[rules]]\npath = \"crates/ffhn-core/src/model/item.rs\"\nmatch = \"exact\"\nrole = \"exact\"\nowner = \"test owner\"\nrationale = \"test rationale\"\nsplit_trigger = \"test trigger\"\nmax_physical_lines = 1\nmax_items = 1\nmax_public_items = 1\nmax_imports = 1\nmax_functions = 1\nmax_decision_points = 1\nmax_match_arms = 1\n"
        ),
    )
    .expect("policy");
    let policy = Policy::load(&policy_path).expect("parsed policy");

    assert_eq!(
        policy
            .rule_for("crates/ffhn-core/src/model/item.rs")
            .expect("rule")
            .role(),
        "exact"
    );
    assert!(policy.rule_for("unowned.rs").is_none());
}

#[test]
fn reports_unowned_budget_and_dependency_violations_together() {
    let directory = TempDir::new().expect("temporary directory");
    write_fixture_source_tree(&directory);
    let policy = write_policy(&directory, &format!("{RULE}\n"));

    let findings = collect_findings(directory.path(), &policy).expect("findings");

    assert!(
        findings
            .iter()
            .any(|finding| finding.contains("unowned.rs: has no declared ownership rule"))
    );
    assert!(
        findings
            .iter()
            .any(|finding| finding.contains("model.rs: physical lines 21 exceeds 20"))
    );
    assert!(findings
        .iter()
        .any(|finding| finding.contains("model.rs: internal dependency `runtime` is forbidden")));
}

#[test]
fn reports_every_budget_dimension_and_stale_rules() {
    let directory = TempDir::new().expect("temporary directory");
    let source = directory.path().join("crates/ffhn-core/src/model.rs");
    fs::create_dir_all(source.parent().expect("parent")).expect("directories");
    fs::write(
        source,
        "use crate::model::Thing;\npub fn scenario() { if true { match Thing::one() { _ => {} } } }\n",
    )
    .expect("source");
    let policy = write_policy(
        &directory,
        &format!(
            "version = 1\n{}{}",
            rule_block(
                "crates/ffhn-core/src/model.rs",
                "exact",
                0,
                "allowed_internal_dependencies = []"
            ),
            rule_block("crates/ffhn-core/src/stale.rs", "exact", 1, "")
        ),
    );

    let findings = collect_findings(directory.path(), &policy).expect("findings");

    for dimension in [
        "physical lines",
        "top-level items",
        "public items",
        "imports",
        "functions",
        "decision points",
        "match arms",
    ] {
        assert!(findings.iter().any(|finding| finding.contains(dimension)));
    }
    assert!(findings.iter().any(|finding| {
        finding.contains("stale.rs: ownership rule matches no maintained Rust source file")
    }));
}

#[test]
fn policy_validation_rejects_unreadable_malformed_and_ambiguous_contracts() {
    let directory = TempDir::new().expect("temporary directory");
    let missing = Policy::load(&directory.path().join("missing.toml")).expect_err("missing policy");
    assert!(
        missing
            .to_string()
            .contains("cannot read Rust source-structure policy")
    );

    let cases = vec![
        (
            "malformed",
            "version = [".to_owned(),
            "cannot parse Rust source-structure policy",
        ),
        (
            "unsupported",
            "version = 2".to_owned(),
            "unsupported Rust source-structure policy version 2",
        ),
        (
            "empty",
            "version = 1".to_owned(),
            "must declare at least one rule",
        ),
        (
            "prefix-without-slash",
            format!(
                "version = 1\n{}",
                rule_block("crates/ffhn-core/src", "prefix", 1, "")
            ),
            "must end with `/`",
        ),
        (
            "absolute-path",
            format!(
                "version = 1\n{}",
                rule_block("/crates/ffhn-core/src/", "prefix", 1, "")
            ),
            "must be a normalized workspace-relative path",
        ),
        (
            "non-rust-exact-path",
            format!(
                "version = 1\n{}",
                rule_block("crates/ffhn-core/src", "exact", 1, "")
            ),
            "must name a Rust source file",
        ),
        (
            "invalid-expiry",
            format!(
                "version = 1\n{}",
                rule_block(
                    "crates/ffhn-core/src/",
                    "prefix",
                    1,
                    "review_expires_on = \"not-a-date\""
                )
            ),
            "invalid review_expires_on",
        ),
    ];
    for (name, policy, expected) in cases {
        let path = directory.path().join(format!("{name}.toml"));
        fs::write(&path, policy).expect("policy");
        let error = Policy::load(&path).expect_err(name);
        assert!(error.to_string().contains(expected), "{name}: {error}");
    }

    for (name, field) in [
        ("empty-path", "path = \"\""),
        ("empty-role", "role = \"\""),
        ("empty-owner", "owner = \"\""),
        ("empty-rationale", "rationale = \"\""),
        ("empty-split-trigger", "split_trigger = \"\""),
    ] {
        let expected_field = match name {
            "empty-path" => "path = \"crates/ffhn-core/src/\"",
            "empty-role" => "role = \"test-role\"",
            "empty-owner" => "owner = \"test owner\"",
            "empty-rationale" => "rationale = \"test rationale\"",
            "empty-split-trigger" => "split_trigger = \"test trigger\"",
            _ => unreachable!("test case has a known field"),
        };
        let policy =
            rule_block("crates/ffhn-core/src/", "prefix", 1, "").replacen(expected_field, field, 1);
        let path = directory.path().join(format!("{name}.toml"));
        fs::write(&path, format!("version = 1\n{policy}")).expect("policy");
        let error = Policy::load(&path).expect_err(name);
        assert!(
            error
                .to_string()
                .contains("every Rust source-structure rule must declare"),
            "{name}: {error}"
        );
    }

    for (name, rule_path) in [
        ("backslash-path", "crates\\\\ffhn-core\\\\src/"),
        ("double-slash-path", "crates//ffhn-core/src/"),
        ("current-directory-path", "crates/./ffhn-core/src/"),
        ("parent-directory-path", "crates/ffhn-core/../src/"),
    ] {
        let path = directory.path().join(format!("{name}.toml"));
        fs::write(
            &path,
            format!("version = 1\n{}", rule_block(rule_path, "prefix", 1, "")),
        )
        .expect("policy");
        let error = Policy::load(&path).expect_err(name);
        assert!(
            error
                .to_string()
                .contains("must be a normalized workspace-relative path"),
            "{name}: {error}"
        );
    }

    let duplicate_path = directory.path().join("duplicate.toml");
    fs::write(
        &duplicate_path,
        format!(
            "version = 1\n{}{}",
            rule_block("crates/ffhn-core/src/", "prefix", 1, ""),
            rule_block("crates/ffhn-core/src/", "prefix", 1, "")
        ),
    )
    .expect("duplicate policy");
    let duplicate = Policy::load(&duplicate_path).expect_err("duplicate policy");
    assert!(
        duplicate
            .to_string()
            .contains("duplicate Rust source-structure rule for prefix")
    );

    let duplicate_exact_path = directory.path().join("duplicate-exact.toml");
    fs::write(
        &duplicate_exact_path,
        format!(
            "version = 1\n{}{}",
            rule_block("crates/ffhn-core/src/lib.rs", "exact", 1, ""),
            rule_block("crates/ffhn-core/src/lib.rs", "exact", 1, "")
        ),
    )
    .expect("duplicate exact policy");
    let duplicate_exact = Policy::load(&duplicate_exact_path).expect_err("duplicate exact policy");
    assert!(
        duplicate_exact
            .to_string()
            .contains("duplicate Rust source-structure rule for exact")
    );
}

#[test]
fn policy_expirations_are_enforced_only_when_the_review_date_has_passed() {
    let directory = TempDir::new().expect("temporary directory");
    let policy = write_policy(
        &directory,
        &format!(
            "version = 1\n{}{}",
            rule_block(
                "crates/ffhn-core/src/",
                "prefix",
                1,
                "review_expires_on = \"2000-01-01\""
            ),
            rule_block(
                "xtask/src/",
                "prefix",
                1,
                "review_expires_on = \"2999-01-01\""
            )
        ),
    );

    let findings = policy.expired_rule_findings().expect("expiry findings");

    assert_eq!(findings.len(), 1);
    assert!(findings[0].contains("2000-01-01"));
}

#[test]
fn report_can_inspect_unowned_files_while_check_rejects_them() {
    let directory = TempDir::new().expect("temporary directory");
    write_fixture_source_tree(&directory);
    let tooling = directory.path().join("tooling");
    fs::create_dir_all(&tooling).expect("tooling directory");
    fs::write(tooling.join("rust-source-shape-policy.toml"), RULE).expect("policy");

    report(directory.path()).expect("report accepts unowned sources for diagnosis");
    let error = check(directory.path()).expect_err("check rejects unowned source");
    assert_eq!(error.to_string(), "source-structure contract failed");
}

fn write_policy(directory: &TempDir, policy: &str) -> Policy {
    let policy_path = directory.path().join("policy.toml");
    fs::write(&policy_path, policy).expect("policy");
    Policy::load(&policy_path).expect("parsed policy")
}

fn rule_block(path: &str, match_kind: &str, maximum: usize, extra: &str) -> String {
    format!(
        "\n[[rules]]\npath = \"{path}\"\nmatch = \"{match_kind}\"\nrole = \"test-role\"\nowner = \"test owner\"\nrationale = \"test rationale\"\nsplit_trigger = \"test trigger\"\nmax_physical_lines = {maximum}\nmax_items = {maximum}\nmax_public_items = {maximum}\nmax_imports = {maximum}\nmax_functions = {maximum}\nmax_decision_points = {maximum}\nmax_match_arms = {maximum}\n{extra}\n"
    )
}

fn write_fixture_source_tree(directory: &TempDir) {
    for path in [
        "crates/ffhn-core/src/model.rs",
        "xtask/tests/unowned.rs",
        "crates/ffhn-cli/src/main.rs",
        "xtask/src/main.rs",
    ] {
        let file = directory.path().join(path);
        fs::create_dir_all(file.parent().expect("parent")).expect("directories");
        let source = if path.ends_with("model.rs") {
            format!("use crate::runtime::Runner;\n{}", "// filler\n".repeat(20))
        } else {
            "fn main() {}\n".to_owned()
        };
        fs::write(file, source).expect("source");
    }
}
