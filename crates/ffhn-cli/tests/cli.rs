use std::fs;

use assert_cmd::Command;
use tempfile::tempdir;

fn write_target(target_dir: &std::path::Path) {
    fs::create_dir_all(target_dir).expect("create target directory");
    fs::write(target_dir.join("source.json"), r#"{"price":"1.00"}"#).expect("write source");
    fs::write(
        target_dir.join("target.toml"),
        format!(
            "schema_name = \"ffhn.target\"\nschema_version = 9\ntarget_id = \"demo\"\ndisplay_name = \"Demo\"\nenabled = true\nescalate_after = 3\ndeclared_type = \"decimal\"\nconditions = []\n\n[target]\nkind = \"file\"\nfile_path = \"{}\"\n\n[fetch]\nengine = \"file\"\nmax_bytes = 1024\n\n[projection]\nkind = \"json_pointer\"\npointer = \"/price\"\n",
            target_dir.join("source.json").display()
        ),
    )
    .expect("write target");
}

#[test]
fn run_refuse_reset_cycle_is_machine_readable() {
    let temporary = tempdir().expect("temporary directory");
    let watch_root = temporary.path().join("watchlist");
    let target_dir = watch_root.join("demo");
    write_target(&target_dir);
    let root = watch_root.to_string_lossy().into_owned();

    Command::cargo_bin("ffhn")
        .expect("ffhn")
        .args(["run", "--watch-root", &root, "--target", "demo"])
        .assert()
        .success()
        .stdout(predicates::str::contains("\"outcome\":\"initialized\""));

    let changed_target = fs::read_to_string(target_dir.join("target.toml"))
        .expect("read target")
        .replace("pointer = \"/price\"", "pointer = \"/other\"");
    fs::write(target_dir.join("target.toml"), changed_target).expect("change target");
    Command::cargo_bin("ffhn")
        .expect("ffhn")
        .args(["run", "--watch-root", &root, "--target", "demo"])
        .assert()
        .code(1)
        .stdout(predicates::str::contains(
            "\"outcome\":\"refused_contract_digest\"",
        ));

    Command::cargo_bin("ffhn")
        .expect("ffhn")
        .args(["reset", "--watch-root", &root, "--target", "demo"])
        .assert()
        .success()
        .stdout(predicates::str::contains("\"storage_cleared\":true"));
}

#[test]
fn reset_is_registered_in_command_help() {
    Command::cargo_bin("ffhn")
        .expect("ffhn")
        .args(["reset", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "Blindly delete one target's isolated v2 storage root.",
        ));
}
