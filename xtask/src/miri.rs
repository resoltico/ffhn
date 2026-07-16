use crate::model::{CommandArtifactLayout, CommandSpec};
use crate::tooling::RustTooling;

pub(crate) const MIRI_V2_FOUNDATION_TEST_NAME: &str =
    "model::v2::tests::typed_parser_covers_exact_integer_decimal_money_and_semver";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MiriPreflightFailure {
    MissingQaNightlyMiri,
    MissingQaNightlyRustSrc,
    BrokenQaNightlyMiriBinary,
}

pub(crate) fn miri_probe_command(tooling: &RustTooling) -> CommandSpec {
    CommandSpec::new(
        "cargo",
        [
            tooling.qa_nightly_toolchain_arg(),
            "miri".to_owned(),
            "--version".to_owned(),
        ],
        true,
    )
    .with_artifact_layout(CommandArtifactLayout::ManagedWorkspace)
}

pub(crate) fn miri_command(tooling: &RustTooling) -> CommandSpec {
    CommandSpec::new(
        "cargo",
        [
            tooling.qa_nightly_toolchain_arg(),
            "miri".to_owned(),
            "test".to_owned(),
            "-p".to_owned(),
            "ffhn-core".to_owned(),
            "--lib".to_owned(),
            "--locked".to_owned(),
            MIRI_V2_FOUNDATION_TEST_NAME.to_owned(),
            "--".to_owned(),
            "--exact".to_owned(),
        ],
        false,
    )
    .with_envs([("MIRIFLAGS", "-Zmiri-strict-provenance")])
    .with_artifact_layout(CommandArtifactLayout::ManagedWorkspace)
}

pub(crate) fn miri_preflight_failures(
    installed_components_output: &str,
    miri_binary_runs: bool,
) -> Vec<MiriPreflightFailure> {
    let mut failures = Vec::new();
    if !installed_component_present(installed_components_output, "miri") {
        failures.push(MiriPreflightFailure::MissingQaNightlyMiri);
    }
    if !installed_component_present(installed_components_output, "rust-src") {
        failures.push(MiriPreflightFailure::MissingQaNightlyRustSrc);
    }
    if failures.is_empty() && !miri_binary_runs {
        failures.push(MiriPreflightFailure::BrokenQaNightlyMiriBinary);
    }

    failures
}

pub(crate) fn miri_preflight_message(
    tooling: &RustTooling,
    failures: &[MiriPreflightFailure],
) -> String {
    let mut message = String::from(
        "Rust Miri preflight failed. FFHN keeps stable as the default toolchain, but the maintained typed-observation strict-provenance proof runs through `cargo +nightly miri test`.\n",
    );

    let missing_miri = failures.contains(&MiriPreflightFailure::MissingQaNightlyMiri);
    let missing_rust_src = failures.contains(&MiriPreflightFailure::MissingQaNightlyRustSrc);
    let broken_binary = failures.contains(&MiriPreflightFailure::BrokenQaNightlyMiriBinary);

    if missing_miri || missing_rust_src {
        let mut missing_components = Vec::new();
        if missing_miri {
            missing_components.push("miri");
        }
        if missing_rust_src {
            missing_components.push("rust-src");
        }
        message.push_str(&format!(
            "\nInstall the missing nightly Miri components:\n  rustup component add {} --toolchain {}\n",
            missing_components.join(" "),
            tooling.qa_nightly_toolchain
        ));
    }

    if broken_binary {
        message.push_str(&format!(
            "\nNightly reports the Miri components as installed, but `cargo +{} miri --version` still does not run.\nRepair the nightly toolchain cleanly with:\n  rustup toolchain uninstall {}\n  rustup toolchain install {} --profile minimal --component llvm-tools-preview --component miri --component rust-src\n",
            tooling.qa_nightly_toolchain,
            tooling.qa_nightly_toolchain,
            tooling.qa_nightly_toolchain,
        ));
    }

    message
}

fn installed_component_present(output: &str, expected_component: &str) -> bool {
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter_map(|line| line.split_whitespace().next())
        .any(|component| {
            component == expected_component
                || component
                    .strip_prefix(expected_component)
                    .is_some_and(|suffix| suffix.starts_with('-'))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tooling::parse_rust_tooling;

    fn sample_tooling() -> RustTooling {
        parse_rust_tooling(
            "RUST_WORKSPACE_EDITION=2024\n\
RUST_WORKSPACE_RUST_VERSION=1.97\n\
RUST_STABLE_TOOLCHAIN=1.97.0\n\
RUST_QA_NIGHTLY_TOOLCHAIN=nightly-2026-05-11\n\
\n\
CARGO_AUDIT_VERSION=0.22.2\n\
CARGO_DENY_VERSION=0.20.2\n\
CARGO_FUZZ_VERSION=0.13.2\n\
CARGO_LLVM_COV_VERSION=0.8.7\n\
CARGO_NEXTEST_VERSION=0.9.140\n\
CARGO_OUTDATED_VERSION=0.19.0\n\
CARGO_SEMVER_CHECKS_VERSION=0.48.0\n",
        )
        .expect("parse tooling")
    }

    #[test]
    fn miri_commands_target_the_v2_foundation_probe() {
        let tooling = sample_tooling();
        let probe = miri_probe_command(&tooling);
        let command = miri_command(&tooling);

        assert_eq!(
            probe.args,
            vec![
                "+nightly-2026-05-11".to_owned(),
                "miri".to_owned(),
                "--version".to_owned(),
            ]
        );
        assert_eq!(
            command.args,
            vec![
                "+nightly-2026-05-11".to_owned(),
                "miri".to_owned(),
                "test".to_owned(),
                "-p".to_owned(),
                "ffhn-core".to_owned(),
                "--lib".to_owned(),
                "--locked".to_owned(),
                MIRI_V2_FOUNDATION_TEST_NAME.to_owned(),
                "--".to_owned(),
                "--exact".to_owned(),
            ]
        );
        assert_eq!(
            command.env.get("MIRIFLAGS"),
            Some(&"-Zmiri-strict-provenance".to_owned())
        );
        assert_eq!(
            command.artifact_layout,
            CommandArtifactLayout::ManagedWorkspace
        );
    }

    #[test]
    fn miri_preflight_detects_missing_components_and_broken_binaries() {
        assert_eq!(
            miri_preflight_failures("", false),
            vec![
                MiriPreflightFailure::MissingQaNightlyMiri,
                MiriPreflightFailure::MissingQaNightlyRustSrc,
            ]
        );
        assert_eq!(
            miri_preflight_failures("miri-aarch64-apple-darwin\nrust-src\n", false),
            vec![MiriPreflightFailure::BrokenQaNightlyMiriBinary]
        );
        assert!(miri_preflight_failures("miri-aarch64-apple-darwin\nrust-src\n", true).is_empty());
    }

    #[test]
    fn miri_preflight_message_names_the_repair_commands() {
        let tooling = sample_tooling();
        let message = miri_preflight_message(
            &tooling,
            &[
                MiriPreflightFailure::MissingQaNightlyMiri,
                MiriPreflightFailure::MissingQaNightlyRustSrc,
            ],
        );
        assert!(
            message.contains("rustup component add miri rust-src --toolchain nightly-2026-05-11")
        );

        let broken_message =
            miri_preflight_message(&tooling, &[MiriPreflightFailure::BrokenQaNightlyMiriBinary]);
        assert!(broken_message.contains("cargo +nightly-2026-05-11 miri --version"));
        assert!(broken_message.contains("rustup toolchain uninstall nightly-2026-05-11"));
    }

    #[test]
    fn miri_preflight_message_lists_only_the_missing_component_names() {
        let tooling = sample_tooling();
        let missing_miri =
            miri_preflight_message(&tooling, &[MiriPreflightFailure::MissingQaNightlyMiri]);
        assert!(missing_miri.contains("rustup component add miri --toolchain nightly-2026-05-11"));
        assert!(!missing_miri.contains("miri rust-src --toolchain"));

        let missing_rust_src =
            miri_preflight_message(&tooling, &[MiriPreflightFailure::MissingQaNightlyRustSrc]);
        assert!(
            missing_rust_src
                .contains("rustup component add rust-src --toolchain nightly-2026-05-11")
        );
        assert!(!missing_rust_src.contains("miri rust-src --toolchain"));
    }

    #[test]
    fn installed_component_present_accepts_platform_suffixes_but_rejects_lookalikes() {
        assert!(installed_component_present("miri\n", "miri"));
        assert!(installed_component_present(
            "miri-aarch64-apple-darwin\n",
            "miri"
        ));
        assert!(!installed_component_present("mirihelper\n", "miri"));
        assert!(!installed_component_present("cargo-miri\n", "miri"));
    }
}
