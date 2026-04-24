use super::*;

#[test]
fn agent_instruction_parity_docs_redirect_every_agent_to_the_root_entrypoint() {
    let repo_root = repo_root();
    let root_agents = repo_root.join("AGENTS.md");
    assert!(root_agents.is_file(), "missing {}", root_agents.display());

    let expected = "MANDATORY: Before performing any analysis, edits, commands, or file reads, every agent MUST load and follow AGENTS.md at the repository root as the sole authoritative instruction source, and if that file has not been read the agent MUST stop immediately and do no work.\n";

    for path in [
        repo_root.join(".codex/AGENTS.md"),
        repo_root.join(".claude/CLAUDE.md"),
        repo_root.join(".gemini/GEMINI.md"),
    ] {
        let text = fs::read_to_string(&path).expect("read agent parity doc");
        assert_eq!(text, expected, "{}", path.display());
    }
}

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
        readme.contains("(docs/getting-started.md#quick-start)"),
        "README.md must route readers to the quick-start entrypoint"
    );
    assert!(
        readme.contains("(docs/README.md)"),
        "README.md must route readers to the full docs index"
    );
    assert!(
        docs_index.contains("[getting-started.md](getting-started.md)"),
        "docs/README.md must list getting-started.md"
    );
}

#[test]
fn public_markdown_links_and_repo_file_mentions_resolve() {
    let repo_root = repo_root();

    for path in public_markdown_paths(&repo_root).expect("markdown paths") {
        let text = fs::read_to_string(&path).expect("read markdown");
        let path_display = path.display().to_string();

        for target in markdown_link_targets(&text) {
            if target.starts_with('#')
                || target.starts_with("http://")
                || target.starts_with("https://")
                || target.starts_with("mailto:")
            {
                continue;
            }

            assert!(
                resolve_repo_path_exists(&repo_root, &path, &target),
                "{path_display} links to missing local path `{target}`"
            );
        }

        for mention in repo_file_mentions(&text) {
            assert!(
                resolve_repo_path_exists(&repo_root, &path, &mention),
                "{path_display} mentions missing repo file `{mention}`"
            );
        }
    }
}

#[test]
fn public_release_docs_match_the_canonical_release_target_script() {
    let repo_root = repo_root();
    let platform_support = fs::read_to_string(repo_root.join("docs/platform-support.md"))
        .expect("read docs/platform-support.md");
    let operations =
        fs::read_to_string(repo_root.join("docs/operations.md")).expect("read docs/operations");
    let release_protocol = fs::read_to_string(repo_root.join("docs/release-protocol.md"))
        .expect("read docs/release-protocol");

    let target_triples = bash_stdout_lines(
        &repo_root,
        "source scripts/release-targets.sh && release_target_triples",
    );
    assert!(
        !target_triples.is_empty(),
        "release target inventory is empty"
    );
    for target in &target_triples {
        assert!(
            platform_support.contains(target),
            "docs/platform-support.md missing `{target}`"
        );
        assert!(
            operations.contains(target),
            "docs/operations.md missing `{target}`"
        );
    }

    let public_assets = bash_stdout_lines(
        &repo_root,
        "source scripts/release-targets.sh && release_asset_names_for_version 'X.Y.Z'",
    );
    assert!(
        !public_assets.is_empty(),
        "release asset inventory is empty"
    );
    for asset in &public_assets {
        assert!(
            platform_support.contains(asset),
            "docs/platform-support.md missing `{asset}`"
        );
    }

    let protocol_assets = bash_stdout_lines(
        &repo_root,
        "source scripts/release-targets.sh && release_asset_names_for_version '${VERSION}'",
    );
    for asset in &protocol_assets {
        assert!(
            release_protocol.contains(asset),
            "docs/release-protocol.md missing `{asset}`"
        );
    }
}

#[test]
fn release_protocol_documents_verified_release_closeout_invariants() {
    let repo_root = repo_root();
    let release_protocol = fs::read_to_string(repo_root.join("docs/release-protocol.md"))
        .expect("read docs/release-protocol");
    let operations =
        fs::read_to_string(repo_root.join("docs/operations.md")).expect("read docs/operations");
    let ci_workflow =
        fs::read_to_string(repo_root.join(".github/workflows/ci.yml")).expect("read ci workflow");

    assert!(
        release_protocol.contains("required conversation resolution before merge"),
        "docs/release-protocol.md must document required conversation resolution"
    );
    assert!(
        release_protocol.contains(
            "gh api \"repos/$REPO/pulls/$PR_NUMBER/files\" --paginate --jq '.[].filename'"
        ),
        "docs/release-protocol.md must document the large-PR file-list fallback"
    );
    assert!(
        release_protocol
            .contains("gh api \"repos/$REPO/pulls/<N>/files\" --paginate --jq '.[].filename'"),
        "docs/release-protocol.md must document the Dependabot large-PR file-list fallback"
    );
    assert!(
        release_protocol.contains("release-prep/${VERSION}"),
        "docs/release-protocol.md must document dirty release-candidate capture via release-prep/"
    );
    assert!(
        release_protocol.contains("git -C \"$RELEASE_WORKTREE\" checkout --detach"),
        "docs/release-protocol.md must document detaching a disposable release worktree before reclaiming main"
    );
    assert!(
        release_protocol.contains("git fetch origin --prune --tags"),
        "docs/release-protocol.md must use explicit fetch steps during release sync"
    );
    assert!(
        release_protocol.contains("git merge --ff-only origin/main"),
        "docs/release-protocol.md must use explicit fast-forward merges during release sync"
    );
    assert!(
        !release_protocol.contains("git pull --ff-only"),
        "docs/release-protocol.md must not rely on implicit git pull --ff-only"
    );
    assert!(
        release_protocol.contains("required `Check` status"),
        "docs/release-protocol.md must name the aggregate required status correctly"
    );
    assert!(
        release_protocol.contains("gh workflow run ci.yml --ref \"$RELEASE_BRANCH\""),
        "docs/release-protocol.md must document the CI workflow_dispatch recovery path"
    );
    assert!(
        release_protocol.contains("app/dependabot"),
        "docs/release-protocol.md must document GitHub's Dependabot app identity"
    );
    assert!(
        release_protocol.contains("failed or cancelled sibling run"),
        "docs/release-protocol.md must document cancelled sibling release runs"
    );
    assert!(
        operations.contains("workflow_dispatch"),
        "docs/operations.md must mention the CI workflow_dispatch recovery path"
    );
    assert!(
        ci_workflow.contains("workflow_dispatch:"),
        ".github/workflows/ci.yml must expose workflow_dispatch for maintainer recovery"
    );
}

#[test]
fn semver_baseline_refresh_docs_always_include_git_ref() {
    let repo_root = repo_root();

    for path in public_markdown_paths(&repo_root).expect("markdown paths") {
        let text = fs::read_to_string(&path).expect("read markdown");
        for line in text
            .lines()
            .filter(|line| line.contains("cargo xtask refresh-semver-baseline"))
        {
            let path_display = path.display().to_string();
            let trimmed_line = line.trim().to_owned();
            assert!(
                line.contains("--git-ref"),
                "{path_display} contains `cargo xtask refresh-semver-baseline` without `--git-ref`: {trimmed_line}"
            );
        }
    }
}

#[test]
fn public_markdown_mentions_only_registered_operations_and_documents() {
    let repo_root = repo_root();
    let registered_operations = cli_contract()
        .operations
        .iter()
        .map(|operation| operation.id)
        .collect::<BTreeSet<_>>();
    let registered_documents = cli_contract()
        .documents
        .iter()
        .map(|document| document.id)
        .collect::<BTreeSet<_>>();

    for path in public_markdown_paths(&repo_root).expect("markdown paths") {
        let text = fs::read_to_string(&path).expect("read markdown");
        let path_display = path.display().to_string();
        assert_registered_operation_ids(
            &path_display,
            extract_cli_operation_ids(&text),
            &registered_operations,
        );
        assert_registered_document_ids(
            &path_display,
            extract_document_ids(&text),
            &registered_documents,
        );
    }
}

#[test]
fn user_facing_source_literals_mention_only_registered_operations_and_documents() {
    let repo_root = repo_root();
    let registered_operations = cli_contract()
        .operations
        .iter()
        .map(|operation| operation.id)
        .collect::<BTreeSet<_>>();
    let registered_documents = cli_contract()
        .documents
        .iter()
        .map(|document| document.id)
        .collect::<BTreeSet<_>>();

    assert_registered_operation_ids(
        "inline literal",
        extract_cli_operation_ids("`ffhn run --target demo`"),
        &registered_operations,
    );
    assert_registered_document_ids(
        "inline literal",
        extract_document_ids("ffhn.run_report"),
        &registered_documents,
    );

    for path in user_facing_source_paths(&repo_root).expect("source paths") {
        let text = fs::read_to_string(&path).expect("read source");
        for literal in string_literals(production_source_text(&text)) {
            let path_display = path.display().to_string();
            assert_registered_operation_ids(
                &path_display,
                extract_cli_operation_ids(&literal),
                &registered_operations,
            );
            assert_registered_document_ids(
                &path_display,
                extract_document_ids(&literal),
                &registered_documents,
            );
        }
    }
}
