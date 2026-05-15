use super::super::*;

#[cfg(unix)]
#[test]
fn public_release_docs_match_the_canonical_release_target_script() {
    let repo_root = repo_root();
    let platform_support = fs::read_to_string(repo_root.join("docs/platform-support.md"))
        .expect("read docs/platform-support.md");
    let operations =
        fs::read_to_string(repo_root.join("docs/operations.md")).expect("read docs/operations");
    let release_protocol = fs::read_to_string(repo_root.join("docs/release-protocol.md"))
        .expect("read docs/release-protocol");

    let target_triples = crate::release::release_target_triples(&repo_root)
        .expect("release target triples from canonical script");
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

    let public_assets = crate::release::release_asset_names(&repo_root, "X.Y.Z")
        .expect("release asset names from canonical script");
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

    let protocol_assets = crate::release::release_asset_names(&repo_root, "${VERSION}")
        .expect("release protocol asset names from canonical script");
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
        release_protocol.contains("./scripts/validate-devcontainer.sh"),
        "docs/release-protocol.md must route release operators to the dedicated devcontainer validator when contributor-environment paths change"
    );
    assert!(
        release_protocol
            .contains("FFHN_DEVCONTAINER_SKIP_BUILD=1 ./scripts/run-devcontainer-check.sh"),
        "docs/release-protocol.md must route release operators to the cached-image full devcontainer gate when contributor-environment paths change"
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
