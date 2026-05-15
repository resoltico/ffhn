use crate::repo_files::maintained_repo_owned_paths;

use super::super::*;

#[test]
fn agent_instruction_contract_uses_the_root_entrypoint_only() {
    let repo_root = repo_root();
    let root_agents = repo_root.join("AGENTS.md");
    assert!(root_agents.is_file(), "missing {}", root_agents.display());

    {
        let path = repo_root.join(".codex/AGENTS.md");
        assert!(
            !path.exists(),
            "{} should not shadow the repository-root AGENTS.md entrypoint",
            path.display()
        );
    }
}

#[cfg(unix)]
#[test]
fn generated_cli_sections_match_the_core_owned_contract() {
    let repo_root = repo_root();
    let cli_doc = fs::read_to_string(repo_root.join("docs/cli.md")).expect("read docs/cli.md");

    assert_eq!(
        marked_section(&cli_doc, "cli-catalog").expect("CLI catalog section"),
        render_cli_catalog_section()
    );
}

#[test]
fn storefront_readme_and_docs_index_route_readers_to_getting_started() {
    let repo_root = repo_root();
    let readme = fs::read_to_string(repo_root.join("README.md")).expect("read README");
    let docs_index = fs::read_to_string(repo_root.join("docs/README.md")).expect("read docs index");

    assert!(
        readme.contains(
            "(https://github.com/resoltico/ffhn/blob/main/docs/getting-started.md#portable-quick-start)"
        ),
        "README.md must route readers to the quick-start entrypoint"
    );
    assert!(
        readme.contains("(https://github.com/resoltico/ffhn/blob/main/docs/README.md)"),
        "README.md must route readers to the full docs index"
    );
    assert!(
        !readme.contains("examples/file-target-with-notifications/materialize-target.sh"),
        "README.md must not depend on repo-only example materializers because the packaged README ships without examples/"
    );
    assert!(
        docs_index.contains("[getting-started.md](getting-started.md)"),
        "docs/README.md must list getting-started.md"
    );
}

#[test]
fn docs_index_lists_every_maintained_top_level_doc() {
    let repo_root = repo_root();
    let docs_root = repo_root.join("docs");
    let docs_index = fs::read_to_string(docs_root.join("README.md")).expect("read docs index");
    let mut missing = Vec::new();

    for entry in fs::read_dir(&docs_root).expect("read docs directory") {
        let entry = entry.expect("docs entry");
        let path = entry.path();
        let is_markdown = path.extension().is_some_and(|extension| extension == "md");
        if !is_markdown || path.file_name().is_some_and(|name| name == "README.md") {
            continue;
        }

        let file_name = path
            .file_name()
            .expect("docs markdown filename")
            .to_string_lossy()
            .into_owned();
        let link = format!("[{file_name}]({file_name})");
        if !docs_index.contains(&link) {
            missing.push(file_name);
        }
    }

    assert!(
        missing.is_empty(),
        "docs/README.md must list every maintained top-level docs markdown file:\n{}",
        missing.join("\n")
    );
}

#[test]
fn contributor_docs_route_maintainers_to_the_committed_devcontainer() {
    let repo_root = repo_root();
    let docs_index = fs::read_to_string(repo_root.join("docs/README.md")).expect("read docs index");
    let developer_setup = fs::read_to_string(repo_root.join("docs/developer-setup.md"))
        .expect("read developer setup");
    let quality_gates =
        fs::read_to_string(repo_root.join("docs/quality-gates.md")).expect("read quality gates");
    let developer_devcontainer =
        fs::read_to_string(repo_root.join("docs/developer-devcontainer.md"))
            .expect("read developer devcontainer doc");
    let operations =
        fs::read_to_string(repo_root.join("docs/operations.md")).expect("read operations");

    assert!(
        docs_index.contains("[developer-devcontainer.md](developer-devcontainer.md)"),
        "docs/README.md must list developer-devcontainer.md"
    );
    assert!(
        developer_setup.contains("(developer-devcontainer.md)"),
        "docs/developer-setup.md must route contributors to developer-devcontainer.md"
    );
    assert!(
        quality_gates.contains("./scripts/validate-devcontainer.sh"),
        "docs/quality-gates.md must mention the maintained devcontainer validator"
    );
    assert!(
        quality_gates.contains("./scripts/run-devcontainer-check.sh"),
        "docs/quality-gates.md must mention the maintained headless devcontainer full-gate entrypoint"
    );
    assert!(
        developer_devcontainer.contains("./scripts/run-devcontainer-check.sh"),
        "docs/developer-devcontainer.md must route headless users to the maintained full-gate entrypoint"
    );
    assert!(
        operations.contains("contributor-devcontainer-gate"),
        "docs/operations.md must document the CI devcontainer gate"
    );
    assert!(
        operations.contains(
            "contributor-devcontainer-gate`: validates the committed contributor container and runs the full headless `./check.sh` maintainer gate"
        ),
        "docs/operations.md must describe the CI devcontainer gate as both validation and full-gate proof"
    );
}

#[test]
fn maintained_repo_owned_files_are_tracked_by_git() {
    let repo_root = repo_root();
    let tracked = git_tracked_relative_paths(&repo_root);
    let mut untracked = Vec::new();

    for path in maintained_repo_owned_paths(&repo_root).expect("maintained repo-owned paths") {
        let relative = path
            .strip_prefix(&repo_root)
            .expect("maintained repo-owned file inside repo root")
            .to_path_buf();
        if !tracked.contains(&relative) {
            untracked.push(relative.to_string_lossy().into_owned());
        }
    }

    assert!(
        untracked.is_empty(),
        "maintained repo-owned files must be tracked by git:\n{}",
        untracked.join("\n")
    );
}
