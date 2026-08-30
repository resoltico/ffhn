#[cfg(unix)]
use crate::{app::run_spec, model::CommandSpec};
#[cfg(unix)]
use tempfile::tempdir;

#[cfg(unix)]
#[test]
fn explicit_environment_wins_over_a_scrub_request() {
    let repo_root = tempdir().expect("tempdir");

    run_spec(
        repo_root.path(),
        &CommandSpec::new(
            "sh",
            ["-c", "test \"$CARGO_TARGET_DIR\" = explicit-target"],
            false,
        )
        .without_envs(["CARGO_TARGET_DIR"])
        .with_envs([("CARGO_TARGET_DIR", "explicit-target")]),
    )
    .expect("explicit environment should win over an ambient scrub request");
}
