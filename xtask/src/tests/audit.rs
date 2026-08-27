#[cfg(unix)]
use super::app::{with_test_environment, write_executable};
#[cfg(unix)]
use super::*;
#[cfg(unix)]
use crate::app::run_audit;

#[cfg(unix)]
#[test]
fn audit_exception_is_gated_by_feature_reachability() {
    let repo_root = tempdir().expect("tempdir");
    let bin_dir = repo_root.path().join("bin");
    fs::create_dir_all(&bin_dir).expect("create bin dir");
    write_repo_scaffold(repo_root.path());
    fs::write(
        repo_root.path().join("Cargo.lock"),
        "version = 4\n[[package]]\nname = \"rkyv\"\nversion = \"0.7.46\"\n",
    )
    .expect("advisory lockfile");
    let tooling = sample_tooling();
    let script = format!(
        "#!/bin/sh\nif [ \"$1\" = \"audit\" ] && [ \"$2\" = \"--version\" ]; then printf 'cargo-audit {version}\\n'; exit 0; fi\nif [ \"$1\" = \"tree\" ]; then printf 'rkyv v0.7.46\\n'; exit 0; fi\nexit 0\n",
        version = tooling.cargo_audit_version,
    );
    write_executable(&bin_dir.join("cargo"), &script);
    let error = with_test_environment(&bin_dir, Some(repo_root.path()), || {
        run_audit(repo_root.path(), None).expect_err("reachable advisory must fail closed")
    });
    assert!(error.to_string().contains("is reachable"));

    let script = format!(
        "#!/bin/sh\nif [ \"$1\" = \"audit\" ] && [ \"$2\" = \"--version\" ]; then printf 'cargo-audit {version}\\n'; exit 0; fi\nif [ \"$1\" = \"tree\" ]; then exit 0; fi\nif [ \"$1\" = \"audit\" ]; then case \" $* \" in *' --ignore RUSTSEC-2026-0235 '*) exit 0 ;; *) exit 9 ;; esac; fi\nexit 0\n",
        version = tooling.cargo_audit_version,
    );
    write_executable(&bin_dir.join("cargo"), &script);
    with_test_environment(&bin_dir, Some(repo_root.path()), || {
        run_audit(repo_root.path(), None).expect("unreachable advisory exception")
    });

    fs::create_dir_all(repo_root.path().join("fuzz")).expect("fuzz dir");
    fs::write(
        repo_root.path().join("fuzz/Cargo.lock"),
        "version = 4\n[[package]]\nname = \"rkyv\"\nversion = \"0.7.46\"\n",
    )
    .expect("fuzz lockfile");
    fs::write(
        repo_root.path().join("fuzz/Cargo.toml"),
        "[package]\nname = \"fuzz\"\nversion = \"0.0.0\"\n",
    )
    .expect("fuzz manifest");
    let script = format!(
        "#!/bin/sh\nif [ \"$1\" = \"audit\" ] && [ \"$2\" = \"--version\" ]; then printf 'cargo-audit {version}\\n'; exit 0; fi\nif [ \"$1\" = \"tree\" ]; then exit 9; fi\nexit 0\n",
        version = tooling.cargo_audit_version,
    );
    write_executable(&bin_dir.join("cargo"), &script);
    let error = with_test_environment(&bin_dir, Some(repo_root.path()), || {
        run_audit(
            repo_root.path(),
            Some(std::path::Path::new("fuzz/Cargo.lock")),
        )
        .expect_err("failed proof")
    });
    assert!(error.to_string().contains("could not prove"));

    let script = format!(
        "#!/bin/sh\nif [ \"$1\" = \"audit\" ] && [ \"$2\" = \"--version\" ]; then printf 'cargo-audit {version}\\n'; exit 0; fi\nif [ \"$1\" = \"tree\" ]; then case \" $* \" in *' --manifest-path fuzz/Cargo.toml '*) exit 0 ;; *) exit 8 ;; esac; fi\nif [ \"$1\" = \"audit\" ]; then case \" $* \" in *' --ignore RUSTSEC-2026-0235 '*) exit 0 ;; *) exit 9 ;; esac; fi\nexit 0\n",
        version = tooling.cargo_audit_version,
    );
    write_executable(&bin_dir.join("cargo"), &script);
    with_test_environment(&bin_dir, Some(repo_root.path()), || {
        run_audit(
            repo_root.path(),
            Some(std::path::Path::new("fuzz/Cargo.lock")),
        )
        .expect("nested advisory proof uses the nested manifest")
    });

    fs::write(
        repo_root.path().join("fuzz/Cargo.lock"),
        "version = 4\n[[package]]\nname = \"rkyv\"\nversion = \"0.7.45\"\n[[package]]\nname = \"serde\"\nversion = \"0.7.46\"\n",
    )
    .expect("clean lockfile");
    let script = format!(
        "#!/bin/sh\nif [ \"$1\" = \"audit\" ] && [ \"$2\" = \"--version\" ]; then printf 'cargo-audit {version}\\n'; exit 0; fi\nif [ \"$1\" = \"audit\" ]; then exit 0; fi\nexit 9\n",
        version = tooling.cargo_audit_version,
    );
    write_executable(&bin_dir.join("cargo"), &script);
    with_test_environment(&bin_dir, Some(repo_root.path()), || {
        run_audit(
            repo_root.path(),
            Some(std::path::Path::new("fuzz/Cargo.lock")),
        )
        .expect("no advisory exception needed")
    });
}
