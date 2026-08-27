use std::fs;

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use tempfile::tempdir;

#[test]
fn new_then_validate_uses_graph_configuration_and_mints_no_source_lineage() {
    let temporary = tempdir().expect("temporary directory");
    let graph_root = temporary.path().join("graph");
    let root = graph_root.to_string_lossy().into_owned();

    Command::cargo_bin("ffhn")
        .expect("ffhn")
        .args([
            "new",
            "source",
            "--source",
            "shop",
            "--graph-root",
            &root,
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "\"schema_name\":\"ffhn.new_report\"",
        ));
    assert!(graph_root.join(".ffhn-graph.json").is_file());
    assert!(graph_root.join("sources/shop/source.toml").is_file());
    assert!(!graph_root.join("sources/shop/.ffhn-identity.json").exists());
    assert!(!graph_root.join("sources/shop/.ffhn").exists());

    Command::cargo_bin("ffhn")
        .expect("ffhn")
        .args([
            "new",
            "measurement",
            "--source",
            "shop",
            "--measurement",
            "title",
            "--graph-root",
            &root,
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("\"kind\":\"measurement\""));
    assert!(
        graph_root
            .join("sources/shop/measurements/title/measurement.toml")
            .is_file()
    );

    Command::cargo_bin("ffhn")
        .expect("ffhn")
        .args([
            "validate",
            "--all",
            "--graph-root",
            &root,
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "\"schema_name\":\"ffhn.validate_report\"",
        ))
        .stdout(predicates::str::contains("\"valid\":true"));
    Command::cargo_bin("ffhn")
        .expect("ffhn")
        .args(["agent", "status", "--graph-root", &root, "--format", "json"])
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "\"schema_name\":\"ffhn.agent_status_report\"",
        ));
    Command::cargo_bin("ffhn")
        .expect("ffhn")
        .args([
            "list",
            "--sources",
            "--graph-root",
            &root,
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "\"schema_name\":\"ffhn.list_report\"",
        ));
    Command::cargo_bin("ffhn")
        .expect("ffhn")
        .args([
            "status",
            "--source",
            "shop",
            "--graph-root",
            &root,
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "\"schema_name\":\"ffhn.source_status_report\"",
        ));
    Command::cargo_bin("ffhn")
        .expect("ffhn")
        .args(["agent", "tick", "--graph-root", &root, "--format", "json"])
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "\"schema_name\":\"ffhn.agent_tick_report\"",
        ));
    Command::cargo_bin("ffhn")
        .expect("ffhn")
        .args([
            "reset",
            "--source",
            "shop",
            "--graph-root",
            &root,
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "\"schema_name\":\"ffhn.reset_report\"",
        ));
}

#[test]
fn reset_help_is_mint_only_and_run_is_not_a_command() {
    Command::cargo_bin("ffhn")
        .expect("ffhn")
        .args(["reset", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "Mint fresh source or measurement lineage",
        ));
    Command::cargo_bin("ffhn")
        .expect("ffhn")
        .arg("run")
        .assert()
        .code(2);
}

#[test]
fn summary_is_human_text_for_a_graph_command() {
    let temporary = tempdir().expect("temporary directory");
    let graph_root = temporary.path().join("graph");
    fs::create_dir_all(&graph_root).expect("graph root");
    let root = graph_root.to_string_lossy().into_owned();
    Command::cargo_bin("ffhn")
        .expect("ffhn")
        .args([
            "new",
            "source",
            "--source",
            "shop",
            "--graph-root",
            &root,
            "--format",
            "summary",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("Schema Name: ffhn.new_report"))
        .stdout(predicates::str::contains("{").not());
}

#[test]
fn manual_measure_reports_contention_and_exits_busy() {
    let temporary = tempdir().expect("temporary directory");
    let graph_root = temporary.path().join("graph");
    let root = graph_root.to_string_lossy().into_owned();
    Command::cargo_bin("ffhn")
        .expect("ffhn")
        .args(["new", "source", "--source", "shop", "--graph-root", &root])
        .assert()
        .success();
    let graph =
        ffhn_core::graph::TrustedGraphRoot::open(ffhn_core::graph::GraphPaths::new(&graph_root))
            .expect("graph");
    let source = graph
        .open_source(ffhn_core::graph::SourceId::new("shop").expect("source id"))
        .expect("source");
    let _lease = source
        .try_acquire_write_lease()
        .expect("source lock")
        .expect("available source lock");
    Command::cargo_bin("ffhn")
        .expect("ffhn")
        .args([
            "measure",
            "--source",
            "shop",
            "--graph-root",
            &root,
            "--format",
            "json",
        ])
        .assert()
        .code(4)
        .stdout(predicates::str::contains(
            "\"source_status\":\"skipped_locked\"",
        ));
}

#[test]
fn finite_agent_tick_exits_busy_when_the_graph_lease_is_owned() {
    let temporary = tempdir().expect("temporary directory");
    let graph_root = temporary.path().join("graph");
    let root = graph_root.to_string_lossy().into_owned();
    Command::cargo_bin("ffhn")
        .expect("ffhn")
        .args(["new", "source", "--source", "shop", "--graph-root", &root])
        .assert()
        .success();
    let graph =
        ffhn_core::graph::TrustedGraphRoot::open(ffhn_core::graph::GraphPaths::new(&graph_root))
            .expect("graph");
    let _worker = ffhn_core::graph::AgentWorker::try_start(&graph)
        .expect("agent lease")
        .expect("available agent lease");
    Command::cargo_bin("ffhn")
        .expect("ffhn")
        .args(["agent", "tick", "--graph-root", &root])
        .assert()
        .code(4)
        .stderr(predicates::str::contains("agent is already running"));
}

#[test]
fn measure_dry_run_and_live_run_obey_the_graph_lineage_boundary() {
    let temporary = tempdir().expect("temporary directory");
    let graph_root = temporary.path().join("graph");
    let root = graph_root.to_string_lossy().into_owned();
    let value = graph_root.join("value.json");
    let value_path = value.to_string_lossy().into_owned();
    Command::cargo_bin("ffhn")
        .expect("ffhn")
        .args(["new", "source", "--source", "shop", "--graph-root", &root])
        .assert()
        .success();
    Command::cargo_bin("ffhn")
        .expect("ffhn")
        .args([
            "new",
            "measurement",
            "--source",
            "shop",
            "--measurement",
            "price",
            "--graph-root",
            &root,
        ])
        .assert()
        .success();
    Command::cargo_bin("ffhn")
        .expect("ffhn")
        .args([
            "new",
            "measurement",
            "--source",
            "shop",
            "--measurement",
            "price",
            "--graph-root",
            &root,
        ])
        .assert()
        .code(3);
    fs::write(&value, "{\"price\":7}\n").expect("value");
    fs::write(
        graph_root.join("sources/shop/source.toml"),
        format!(
            "schema_name = \"ffhn.source\"\nschema_version = 1\nsource_id = \"shop\"\ndisplay_name = \"Shop\"\nenabled = true\nescalate_after = 2\n[fetch]\nengine = \"file\"\nfile_path = {value_path:?}\nmax_bytes = 1024\n[conditional]\nenabled = false\n[schedule]\ninterval_ms = 1000\nmin_interval_ms = 1000\n"
        ),
    )
    .expect("source config");
    fs::write(
        graph_root.join("sources/shop/measurements/price/measurement.toml"),
        "schema_name = \"ffhn.measurement\"\nschema_version = 1\nmeasurement_id = \"price\"\ndisplay_name = \"Price\"\nenabled = true\nescalate_after = 2\ndeclared_type = \"integer\"\nconditions = []\n[projection]\nkind = \"json_pointer\"\npointer = \"/price\"\n",
    )
    .expect("measurement config");
    Command::cargo_bin("ffhn")
        .expect("ffhn")
        .args([
            "measure",
            "--source",
            "shop",
            "--graph-root",
            &root,
            "--dry-run",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("\"source_status\":\"document\""));
    assert!(!graph_root.join("sources/shop/.ffhn-identity.json").exists());
    Command::cargo_bin("ffhn")
        .expect("ffhn")
        .args([
            "measure",
            "--source",
            "shop",
            "--graph-root",
            &root,
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "\"schema_name\":\"ffhn.measure_report\"",
        ));
    Command::cargo_bin("ffhn")
        .expect("ffhn")
        .args([
            "status",
            "--source",
            "shop",
            "--graph-root",
            &root,
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("\"status\":\"ready\""));
    Command::cargo_bin("ffhn")
        .expect("ffhn")
        .args([
            "status",
            "--source",
            "shop",
            "--measurement",
            "price",
            "--graph-root",
            &root,
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "\"schema_name\":\"ffhn.measurement_status_report\"",
        ));

    fs::write(&value, "{\"other\":8}\n").expect("missing projection value");
    Command::cargo_bin("ffhn")
        .expect("ffhn")
        .args([
            "measure",
            "--source",
            "shop",
            "--graph-root",
            &root,
            "--format",
            "json",
        ])
        .assert()
        .code(1)
        .stdout(predicates::str::contains(
            "\"status\":\"extraction_failed\"",
        ));

    fs::write(
        graph_root.join("sources/shop/measurements/price/measurement.toml"),
        "schema_name = \"ffhn.measurement\"\nschema_version = 1\nmeasurement_id = \"price\"\ndisplay_name = \"Price\"\nenabled = true\nescalate_after = 2\ndeclared_type = \"integer\"\nconditions = []\n[projection]\nkind = \"json_pointer\"\npointer = \"/other\"\n",
    )
    .expect("changed measurement contract");
    Command::cargo_bin("ffhn")
        .expect("ffhn")
        .args([
            "status",
            "--source",
            "shop",
            "--measurement",
            "price",
            "--graph-root",
            &root,
            "--format",
            "json",
        ])
        .assert()
        .code(1)
        .stdout(predicates::str::contains("\"status\":\"quarantined\""));
}

#[cfg(unix)]
#[test]
fn agent_run_handles_sigterm_and_exits_cleanly() {
    use std::{process::Stdio, thread, time::Duration};

    let temporary = tempdir().expect("temporary directory");
    let graph_root = temporary.path().join("graph");
    let root = graph_root.to_string_lossy().into_owned();
    Command::cargo_bin("ffhn")
        .expect("ffhn")
        .args(["new", "source", "--source", "shop", "--graph-root", &root])
        .assert()
        .success();

    let mut child = std::process::Command::new(assert_cmd::cargo::cargo_bin!("ffhn"))
        .args(["agent", "run", "--graph-root", &root, "--format", "json"])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("agent process");
    thread::sleep(Duration::from_millis(250));
    let signal = std::process::Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status()
        .expect("send SIGTERM");
    assert!(signal.success());
    for _ in 0..100 {
        if let Some(status) = child.try_wait().expect("agent status") {
            assert!(
                status.success(),
                "agent did not exit successfully: {status}"
            );
            return;
        }
        thread::sleep(Duration::from_millis(25));
    }
    let _ = child.kill();
    let _ = child.wait();
    panic!("agent did not exit within the bounded SIGTERM window");
}

#[path = "cli/command_matrix.rs"]
mod command_matrix;
