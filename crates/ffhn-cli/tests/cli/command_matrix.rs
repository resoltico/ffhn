use std::fs;

use assert_cmd::Command;
use tempfile::tempdir;

#[test]
fn command_matrix_covers_selected_measurement_reset_listing_validation_and_failure_routes() {
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
        .args(["new", "source", "--source", "shop", "--graph-root", &root])
        .assert()
        .code(3);
    Command::cargo_bin("ffhn")
        .expect("ffhn")
        .args([
            "new",
            "measurement",
            "--source",
            "missing",
            "--measurement",
            "value",
            "--graph-root",
            &root,
        ])
        .assert()
        .code(3);
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
    .expect("measurement");

    Command::cargo_bin("ffhn")
        .expect("ffhn")
        .args([
            "list",
            "--measurements",
            "--graph-root",
            &root,
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("\"measurement_id\":\"price\""));
    fs::write(
        graph_root.join("sources/shop/.ffhn-lineage.manifest"),
        "not-json",
    )
    .expect("manifest");
    Command::cargo_bin("ffhn")
        .expect("ffhn")
        .args([
            "reset",
            "--source",
            "shop",
            "--measurement",
            "price",
            "--graph-root",
            &root,
        ])
        .assert()
        .code(3);
    fs::remove_file(graph_root.join("sources/shop/.ffhn-lineage.manifest"))
        .expect("remove manifest");
    Command::cargo_bin("ffhn")
        .expect("ffhn")
        .args(["validate", "--source", "shop", "--graph-root", &root])
        .assert()
        .success();
    for dry in [true, false] {
        let mut args = vec![
            "measure",
            "--source",
            "shop",
            "--measurement",
            "price",
            "--graph-root",
            &root,
            "--format",
            "json",
        ];
        if dry {
            args.push("--dry-run");
        }
        Command::cargo_bin("ffhn")
            .expect("ffhn")
            .args(args)
            .assert()
            .success();
    }
    Command::cargo_bin("ffhn")
        .expect("ffhn")
        .args([
            "measure",
            "--source",
            "shop",
            "--measurement",
            "missing",
            "--graph-root",
            &root,
        ])
        .assert()
        .code(3);
    Command::cargo_bin("ffhn")
        .expect("ffhn")
        .args(["validate", "--source", "missing", "--graph-root", &root])
        .assert()
        .code(3);
    Command::cargo_bin("ffhn")
        .expect("ffhn")
        .args([
            "reset",
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
        .stdout(predicates::str::contains("\"measurement_id\":\"price\""));
    Command::cargo_bin("ffhn")
        .expect("ffhn")
        .args(["agent", "tick", "--jobs", "2", "--graph-root", &root])
        .assert()
        .success();
    Command::cargo_bin("ffhn")
        .expect("ffhn")
        .args(["agent", "tick", "--jobs", "0", "--graph-root", &root])
        .assert()
        .code(2);
    Command::cargo_bin("ffhn")
        .expect("ffhn")
        .args(["status", "--source", "missing", "--graph-root", &root])
        .assert()
        .code(3);
    Command::cargo_bin("ffhn")
        .expect("ffhn")
        .args([
            "list",
            "--sources",
            "--graph-root",
            "/path/that/does/not/exist",
        ])
        .assert()
        .code(3);
}
