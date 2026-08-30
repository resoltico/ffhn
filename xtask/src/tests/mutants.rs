use std::fs;

#[cfg(unix)]
use std::process::Command;

#[cfg(unix)]
use serde_json::{Value, json};

#[cfg(unix)]
use super::app::{with_test_environment, write_executable};
#[cfg(unix)]
use super::{with_test_artifact_roots, write_repo_scaffold};
#[cfg(unix)]
use crate::app::{MutantsScope, run_mutants};
#[cfg(unix)]
use crate::plan::mutation_report_root;

fn repo_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn mutation_configurations_partition_all_first_party_production_rust() {
    let runtime = fs::read_to_string(repo_root().join(".cargo/mutants-runtime.toml"))
        .expect("runtime mutation config");
    let tooling = fs::read_to_string(repo_root().join(".cargo/mutants-tooling.toml"))
        .expect("tooling mutation config");
    for config in [&runtime, &tooling] {
        assert!(config.contains("all_features = true"));
        assert!(config.contains("minimum_test_timeout = 120"));
        assert!(config.contains("\"--locked\""));
        assert!(config.contains("build.target-dir='target/cargo-mutants'"));
        assert!(config.contains("build.build-dir='target/cargo-mutants-build'"));
        assert!(config.contains("test_tool = \"cargo\""));
        assert!(config.contains("sharding = \"round-robin\""));
        assert!(config.contains("**/src/**/tests.rs"));
    }
    assert!(runtime.contains("test_package = [\"ffhn-core\", \"ffhn-cli\"]"));
    assert!(runtime.contains("additional_cargo_test_args = [\"--\", \"--test-threads=1\"]"));
    assert!(runtime.contains("crate::CoreError::internal"));
    assert!(runtime.contains("ffhn_core::CoreError::internal"));
    assert!(runtime.contains("crates/ffhn-core/src/**/*.rs"));
    assert!(runtime.contains("crates/ffhn-cli/src/**/*.rs"));
    assert!(!runtime.contains("xtask/src"));
    assert!(tooling.contains("test_package = [\"xtask\"]"));
    assert!(tooling.contains("std::io::Error::other"));
    assert!(tooling.contains("examine_globs = [\"xtask/src/**/*.rs\"]"));
    assert!(!tooling.contains("crates/ffhn-core"));
}

#[test]
fn mutation_workflow_uses_canonical_safe_shards_and_retains_complete_results() {
    let workflow = fs::read_to_string(repo_root().join(".github/workflows/mutants.yml"))
        .expect("mutation workflow")
        .replace("\r\n", "\n");
    assert!(workflow.contains("workflow_dispatch:"));
    assert!(workflow.contains("schedule:"));
    assert!(workflow.contains("pull_request:\n"));
    assert!(workflow.contains("push:\n    branches: [main]"));
    assert!(!workflow.contains("paths:"));
    assert!(workflow.contains("cancel-in-progress: true"));
    assert!(workflow.contains("timeout-minutes: 5"));
    assert!(workflow.contains("shards=\"$(./scripts/mutation-shard-plan.sh)\""));
    assert!(workflow.contains("shard: ${{ fromJSON(needs.mutation-plan.outputs.shards) }}"));
    assert!(workflow.contains("--scope \"${{ matrix.shard.scope }}\""));
    assert!(workflow.contains("--shard \"${{ matrix.shard.selector }}\""));
    assert!(workflow.contains("name: ${{ matrix.shard.artifact_name }}"));
    assert!(!workflow.contains("cargo-mutants-${{ matrix.shard }}"));
    assert!(workflow.contains("CARGO_MUTANTS_VERSION"));
    assert!(!workflow.contains("CARGO_MUTANTS_MINIMUM_TEST_TIMEOUT"));
    assert!(workflow.contains("--in-diff"));
    assert!(workflow.contains("Inspect mutations in changed production code"));
    assert!(workflow.contains("matrix.shard.scope }} pull-request diff"));
    assert!(workflow.contains("--shard \"${{ matrix.shard.selector }}\" --in-diff"));
    assert!(workflow.contains("^crates/(ffhn-cli|ffhn-core)/src/"));
    assert!(workflow.contains("^xtask/src/"));
    assert!(workflow.contains("/(tests/|tests\\\\.rs$)"));
    assert!(!workflow.contains("scope: [runtime, tooling]"));
    assert!(workflow.contains("has_mutants=false"));
    assert!(workflow.contains("steps.diff_mutants.outputs.has_mutants == 'true'"));
    assert!(workflow.contains("path: ${{ runner.temp }}/ffhn-mutation-results\n"));
    assert!(workflow.contains("scope.json"));
    assert!(workflow.contains("pattern: cargo-mutants-*-shard-*-of-*"));
    assert!(workflow.contains("merge-multiple: false"));
    assert!(workflow.contains("./scripts/summarize-mutation-results.sh"));
    assert!(workflow.contains("if: always()"));
    assert!(workflow.contains("mutation-pr-summary:"));
    assert_eq!(
        workflow.matches("\n    name: Mutation testing\n").count(),
        2
    );
    assert!(workflow.contains("needs: [mutation-plan, mutants-diff]"));
}

#[cfg(unix)]
fn generated_shard_plan() -> Value {
    let output = Command::new("bash")
        .arg(repo_root().join("scripts/mutation-shard-plan.sh"))
        .output()
        .expect("run mutation shard plan");
    assert!(
        output.status.success(),
        "mutation shard plan failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("parse shard plan")
}

#[cfg(unix)]
#[test]
fn mutation_shard_plan_covers_runtime_and_tooling_with_safe_unique_identities() {
    let plan = generated_shard_plan();
    let shards = plan.as_array().expect("shard array");
    assert_eq!(shards.len(), 16);
    for (scope, total) in [("runtime", 12), ("tooling", 4)] {
        let scoped = shards
            .iter()
            .filter(|shard| shard["scope"] == scope)
            .collect::<Vec<_>>();
        assert_eq!(scoped.len(), total);
        for (index, shard) in scoped.into_iter().enumerate() {
            assert_eq!(shard["selector"], format!("{index}/{total}"));
            assert_eq!(
                shard["artifact_name"],
                format!("cargo-mutants-{scope}-shard-{index}-of-{total}")
            );
            assert!(!shard["artifact_name"].as_str().expect("name").contains('/'));
        }
    }
}

#[cfg(unix)]
fn two_scope_plan() -> Value {
    json!([
        {
            "scope": "runtime",
            "selector": "0/1",
            "artifact_name": "cargo-mutants-runtime-shard-0-of-1"
        },
        {
            "scope": "tooling",
            "selector": "0/1",
            "artifact_name": "cargo-mutants-tooling-shard-0-of-1"
        }
    ])
}

#[cfg(unix)]
fn write_outcome(
    artifact_root: &std::path::Path,
    artifact_name: &str,
    relative_path: &str,
    counts: [u64; 5],
) {
    let path = artifact_root.join(artifact_name).join(relative_path);
    fs::create_dir_all(path.parent().expect("outcome parent")).expect("outcome parent");
    fs::write(
        path,
        serde_json::to_vec_pretty(&json!({
            "end_time": "2026-08-25T12:00:00Z",
            "total_mutants": counts[0],
            "caught": counts[1],
            "missed": counts[2],
            "timeout": counts[3],
            "unviable": counts[4]
        }))
        .expect("outcome JSON"),
    )
    .expect("write outcome");
}

#[cfg(unix)]
fn write_scope(
    artifact_root: &std::path::Path,
    artifact_name: &str,
    scope: &str,
    selector: &str,
    expected_total: u64,
) {
    let path = artifact_root.join(artifact_name).join("scope.json");
    fs::create_dir_all(path.parent().expect("scope parent")).expect("scope parent");
    fs::write(
        path,
        serde_json::to_vec_pretty(&json!({
            "scope": scope,
            "selector": selector,
            "expected_total": expected_total,
        }))
        .expect("scope JSON"),
    )
    .expect("write scope");
}

#[cfg(unix)]
fn run_summary(
    plan: &Value,
    artifact_root: &std::path::Path,
    summary: &std::path::Path,
) -> std::process::Output {
    Command::new("bash")
        .arg(repo_root().join("scripts/summarize-mutation-results.sh"))
        .arg(plan.to_string())
        .arg(artifact_root)
        .arg(summary)
        .output()
        .expect("run summary")
}

#[cfg(unix)]
#[test]
fn mutation_summary_verifies_exact_scopes_and_aggregates_completed_outcomes_once() {
    let root = tempfile::tempdir().expect("summary fixture");
    let artifacts = root.path().join("artifacts");
    let summary = root.path().join("summary.md");
    write_outcome(
        &artifacts,
        "cargo-mutants-runtime-shard-0-of-1",
        "mutants.out/outcomes.json",
        [3, 2, 1, 0, 0],
    );
    write_scope(
        &artifacts,
        "cargo-mutants-runtime-shard-0-of-1",
        "runtime",
        "0/1",
        3,
    );
    write_outcome(
        &artifacts,
        "cargo-mutants-tooling-shard-0-of-1",
        "mutants.out/outcomes.json",
        [2, 1, 0, 1, 0],
    );
    write_scope(
        &artifacts,
        "cargo-mutants-tooling-shard-0-of-1",
        "tooling",
        "0/1",
        2,
    );
    let output = run_summary(&two_scope_plan(), &artifacts, &summary);
    assert!(
        output.status.success(),
        "summary failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let summary = fs::read_to_string(summary).expect("summary");
    assert!(summary.contains("| `runtime` | `0/1` | 3 | 2 | 1 | 0 | 0 |"));
    assert!(summary.contains("| `tooling` | `0/1` | 2 | 1 | 0 | 1 | 0 |"));
    assert_eq!(summary.matches("Total:").count(), 1);
    assert!(summary.contains("Total: 5 mutants; 3 caught; 1 missed; 1 timed out; 0 unviable."));
}

#[cfg(unix)]
#[test]
fn mutation_summary_rejects_wrong_identity_flattened_layout_and_incomplete_outcome() {
    let plan = two_scope_plan();

    let wrong = tempfile::tempdir().expect("wrong set");
    fs::create_dir_all(wrong.path().join("cargo-mutants-runtime-shard-0-of-1"))
        .expect("expected artifact");
    fs::create_dir_all(wrong.path().join("cargo-mutants-runtime-shard-1-of-1"))
        .expect("unexpected artifact");
    let output = run_summary(&plan, wrong.path(), &wrong.path().join("summary.md"));
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("missing: cargo-mutants-tooling-shard-0-of-1"));
    assert!(stderr.contains("unexpected: cargo-mutants-runtime-shard-1-of-1"));

    let flattened = tempfile::tempdir().expect("flattened");
    for name in [
        "cargo-mutants-runtime-shard-0-of-1",
        "cargo-mutants-tooling-shard-0-of-1",
    ] {
        write_outcome(flattened.path(), name, "outcomes.json", [1, 1, 0, 0, 0]);
    }
    let output = run_summary(
        &plan,
        flattened.path(),
        &flattened.path().join("summary.md"),
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("mutants.out/outcomes.json"));

    let incomplete = tempfile::tempdir().expect("incomplete");
    for name in [
        "cargo-mutants-runtime-shard-0-of-1",
        "cargo-mutants-tooling-shard-0-of-1",
    ] {
        write_outcome(
            incomplete.path(),
            name,
            "mutants.out/outcomes.json",
            [1, 1, 0, 0, 0],
        );
    }
    write_scope(
        incomplete.path(),
        "cargo-mutants-runtime-shard-0-of-1",
        "runtime",
        "0/1",
        1,
    );
    write_scope(
        incomplete.path(),
        "cargo-mutants-tooling-shard-0-of-1",
        "tooling",
        "0/1",
        1,
    );
    let path = incomplete
        .path()
        .join("cargo-mutants-tooling-shard-0-of-1/mutants.out/outcomes.json");
    fs::write(path, r#"{"end_time":null}"#).expect("incomplete outcome");
    let output = run_summary(
        &plan,
        incomplete.path(),
        &incomplete.path().join("summary.md"),
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid or incomplete"));
}

#[cfg(unix)]
#[test]
fn mutation_summary_rejects_empty_mismatched_and_uncaught_scope_evidence() {
    let plan = two_scope_plan();

    let empty = tempfile::tempdir().expect("empty evidence");
    for (scope, name) in [
        ("runtime", "cargo-mutants-runtime-shard-0-of-1"),
        ("tooling", "cargo-mutants-tooling-shard-0-of-1"),
    ] {
        write_outcome(
            empty.path(),
            name,
            "mutants.out/outcomes.json",
            [0, 0, 0, 0, 0],
        );
        write_scope(empty.path(), name, scope, "0/1", 0);
    }
    let output = run_summary(&plan, empty.path(), &empty.path().join("summary.md"));
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("scope evidence"));

    let mismatched = tempfile::tempdir().expect("mismatched evidence");
    for (scope, name) in [
        ("runtime", "cargo-mutants-runtime-shard-0-of-1"),
        ("tooling", "cargo-mutants-tooling-shard-0-of-1"),
    ] {
        write_outcome(
            mismatched.path(),
            name,
            "mutants.out/outcomes.json",
            [1, 1, 0, 0, 0],
        );
        write_scope(mismatched.path(), name, scope, "0/1", 2);
    }
    let output = run_summary(
        &plan,
        mismatched.path(),
        &mismatched.path().join("summary.md"),
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("do not match"));

    let uncaught = tempfile::tempdir().expect("uncaught evidence");
    for (scope, name) in [
        ("runtime", "cargo-mutants-runtime-shard-0-of-1"),
        ("tooling", "cargo-mutants-tooling-shard-0-of-1"),
    ] {
        write_outcome(
            uncaught.path(),
            name,
            "mutants.out/outcomes.json",
            [1, 0, 0, 0, 1],
        );
        write_scope(uncaught.path(), name, scope, "0/1", 1);
    }
    let output = run_summary(&plan, uncaught.path(), &uncaught.path().join("summary.md"));
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("caught no mutants"));
}

#[cfg(unix)]
#[test]
fn mutation_runner_executes_both_safe_scopes_and_preserves_scope_evidence() {
    let root = tempfile::tempdir().expect("runner fixture");
    let bin = root.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    write_repo_scaffold(root.path());
    let calls = root.path().join("mutant-calls.txt");
    write_executable(
        &bin.join("cargo"),
        &format!(
            "#!/bin/sh\nif [ \"$1\" = mutants ] && [ \"$2\" = --version ]; then printf 'cargo-mutants 27.1.0\\n'; exit 0; fi\nprintf 'target=%s build=%s args=%s\\n' \"$CARGO_TARGET_DIR\" \"$CARGO_BUILD_BUILD_DIR\" \"$*\" >> {calls:?}\nwhile [ \"$#\" -gt 0 ]; do if [ \"$1\" = --output ]; then shift; mkdir -p \"$1/mutants.out\"; fi; shift; done\nexit 0\n",
            calls = calls,
        ),
    );

    with_test_artifact_roots(root.path(), || {
        with_test_environment(&bin, Some(root.path()), || {
            run_mutants(root.path(), MutantsScope::All, None, None, false)
        })
        .expect("complete mutation runner");

        let report_root = mutation_report_root(root.path());
        assert!(report_root.join("runtime/mutants.out").is_dir());
        assert!(report_root.join("tooling/mutants.out").is_dir());
    });
    let calls = fs::read_to_string(calls).expect("calls");
    assert!(calls.contains("--config .cargo/mutants-runtime.toml"));
    assert!(calls.contains("--config .cargo/mutants-tooling.toml"));
    assert!(calls.lines().all(|call| call.contains("target= build=")));
    assert!(!calls.contains("--in-place"));
}

#[cfg(unix)]
#[test]
fn mutation_runner_rejects_ambiguous_shards_and_classifies_survivors() {
    let root = tempfile::tempdir().expect("runner fixture");
    let bin = root.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    write_repo_scaffold(root.path());
    write_executable(
        &bin.join("cargo"),
        "#!/bin/sh\nif [ \"$1\" = mutants ] && [ \"$2\" = --version ]; then printf 'cargo-mutants 27.1.0\\n'; exit 0; fi\nexit 2\n",
    );
    let error = with_test_artifact_roots(root.path(), || {
        with_test_environment(&bin, Some(root.path()), || {
            let routed_error =
                crate::run_from(["xtask", "mutants", "--scope", "all", "--shard", "0/12"])
                    .expect_err("routed all-scope shard");
            assert!(
                routed_error
                    .to_string()
                    .contains("requires an explicit --scope")
            );
            assert!(
                run_mutants(root.path(), MutantsScope::All, Some("0/12"), None, false)
                    .expect_err("ambiguous all-scope shard")
                    .to_string()
                    .contains("requires an explicit --scope")
            );
            run_mutants(root.path(), MutantsScope::Runtime, None, None, false)
        })
    })
    .expect_err("surviving mutant");
    assert!(error.to_string().contains("missed mutants"));
    assert!(
        root.path()
            .join(".managed-artifacts/mutation-runs/runtime")
            .is_dir()
    );
}

#[cfg(unix)]
#[test]
fn mutation_runner_iterates_existing_scope_evidence_without_accepting_ci_selectors() {
    let root = tempfile::tempdir().expect("runner fixture");
    let bin = root.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    write_repo_scaffold(root.path());
    let calls = root.path().join("mutant-calls.txt");
    write_executable(
        &bin.join("cargo"),
        &format!(
            "#!/bin/sh\nif [ \"$1\" = mutants ] && [ \"$2\" = --version ]; then printf 'cargo-mutants 27.1.0\\n'; exit 0; fi\nprintf '%s\\n' \"$*\" >> {calls:?}\niterate=false\noutput_dir=\nwhile [ \"$#\" -gt 0 ]; do\n  case \"$1\" in\n    --iterate) iterate=true ;;\n    --output) shift; output_dir=\"$1\" ;;\n  esac\n  shift\ndone\nif [ \"$iterate\" = true ]; then test -f \"$output_dir/mutants.out/caught.txt\"; fi\nmkdir -p \"$output_dir/mutants.out\"\nexit 0\n",
            calls = calls,
        ),
    );

    with_test_artifact_roots(root.path(), || {
        let retained = mutation_report_root(root.path()).join("runtime/mutants.out/caught.txt");
        fs::create_dir_all(retained.parent().expect("retained parent")).expect("retained parent");
        fs::write(&retained, "previously caught mutant\n").expect("retained evidence");
        with_test_environment(&bin, Some(root.path()), || {
            run_mutants(root.path(), MutantsScope::Runtime, None, None, true)
        })
        .expect("iterate selected scope");
        assert!(retained.is_file());
    });
    assert!(
        fs::read_to_string(calls)
            .expect("calls")
            .contains("--iterate")
    );
    assert!(
        run_mutants(root.path(), MutantsScope::Runtime, Some("0/12"), None, true)
            .expect_err("iterate with a CI shard")
            .to_string()
            .contains("cannot be combined")
    );
    assert!(
        run_mutants(
            root.path(),
            MutantsScope::Runtime,
            None,
            Some(std::path::Path::new("changes.diff")),
            true,
        )
        .expect_err("iterate with a pull-request diff")
        .to_string()
        .contains("cannot be combined")
    );
}
