use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(super) fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

pub(super) fn git_tracked_relative_paths(repo_root: &Path) -> BTreeSet<PathBuf> {
    let output = Command::new("git")
        .current_dir(repo_root)
        .args(["ls-files", "-z"])
        .output()
        .expect("run git ls-files -z");
    assert!(
        output.status.success(),
        "git ls-files -z failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    output
        .stdout
        .split(|byte| *byte == b'\0')
        .filter(|entry| !entry.is_empty())
        .map(|entry| PathBuf::from(std::str::from_utf8(entry).expect("utf8 git path")))
        .collect()
}
