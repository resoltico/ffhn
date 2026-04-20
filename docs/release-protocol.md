---
afad: "3.5"
version: "2.0.0"
domain: RELEASE
updated: "2026-04-20"
route:
  keywords: [release protocol, gh cli, tag push, release workflow, semver baseline, verification]
  questions: ["how do I release ffhn?", "what must be verified before tagging a release?", "when do I refresh the ffhn semver baseline?"]
---

# Release Protocol

The release flow is driven by the GitHub CLI (`gh`). Every step that touches GitHub uses `gh`, not the GitHub web UI.

Release choreography lives here. Contract-versioning policy lives in [versioning-policy.md](versioning-policy.md).

## 0. GitHub CLI gate

Before doing anything else:

```bash
gh --version
gh auth status
```

If either command fails, stop immediately.

## 1. Pre-flight

Install the local maintainer toolchain if it is not already available by following [developer-setup.md](developer-setup.md). That guide owns the maintained bootstrap commands for `rustup`, the Cargo QA tools, `shellcheck`, and `gh`.

Stable remains the default FFHN toolchain. Nightly is installed alongside stable only for the coverage gate and manual sanitizer-backed fuzz runs.

Run the single local quality gate first:

```bash
./check.sh
```

or equivalently:

```bash
cargo xtask check
```

That gate must succeed before any release commit or tag. The maintained definition of that gate lives in [quality-gates.md](quality-gates.md).

Then verify:

- `Cargo.toml` `[workspace.package] version` equals the target release version exactly. This is the single version source of truth for both crates and for `ffhn --version`.
- `Cargo.toml` `[workspace.package] description` still reflects the current product in task-facing language.
- `README.md` still documents the current install flow, CLI model, durable runtime model, and the exact public release asset names.
- `CONTRIBUTING.md` still matches the maintained contributor and release workflow.
- `docs/README.md` still points at the maintained developer and maintainer docs.
- `docs/versioning-policy.md` still matches the shipped contract policy, frozen HTMLCut interop model, and semver-baseline rules.
- `docs/cli.md`, `docs/core.md`, `docs/contracts.md`, `docs/reports.md`, and `docs/targets.md` still match the shipped surfaces.
- `docs/quality-gates.md` still matches `cargo xtask`.
- `docs/operations.md` still matches the release scripts, asset matrix, and workflow structure.
- `Cargo.toml` still defines the `dist` Cargo profile used for shipped public binaries.
- repository settings are aligned with this protocol:
  - default branch is `main`
  - `delete_branch_on_merge` is enabled
  - `main` is protected
  - `main` does not require approving reviews
  - `main` does not enforce branch protection for admins
  - required status checks are exactly:
    - `Check`

Before cutting the release branch, enumerate open PRs so dependency-automation work is never surprise-discovered after publication:

```bash
gh pr list --state open \
  --json number,title,url,headRefName,mergeStateStatus,isDraft,author,statusCheckRollup
```

If any open PR is authored by `dependabot[bot]`, decide up front whether it changes release machinery, release assets, or release-critical dependencies. If it does, land or reject it before cutting the release branch. If it does not, carry that decision forward and complete Step 10 before ending the release session.

## 2. Release branch

Do release commits on a release branch, not directly on `main`.

```bash
git checkout -b release/X.Y.Z
git add <every intended release file>
git status --short
git diff --cached --name-status
git diff --cached --stat
git commit -m "release: bump version to X.Y.Z"
git push origin release/X.Y.Z
```

Before committing:

- `git status --short` must show no intended release file left unstaged.
- `git diff --cached --name-status` must show the exact release file set.
- `git diff --cached --stat` must reflect the shipped release payload only.

## 3. Pull request and CI

```bash
gh pr create \
  --title "release: bump version to X.Y.Z" \
  --base main \
  --head release/X.Y.Z \
  --body "Release X.Y.Z"
```

Then verify:

```bash
gh pr diff <N> --name-only
gh pr view <N> --json number,state,mergeStateStatus,statusCheckRollup,url
gh pr checks <N>
```

Do not continue until the required job in workflow `CI` is green:

- `Check`

`Check` is the aggregate branch-protection gate. It must reflect both the Rust maintainer gate and the release-target smoke matrix.

## 4. Merge handoff

```bash
gh pr merge <N> --merge --delete-branch --subject "release: bump version to X.Y.Z (#N)"
git checkout main
git pull
gh pr view <N> --json number,state,mergedAt,headRefName,baseRefName,url
```

Verify:

- PR state is `MERGED`
- `mergedAt` is populated
- local `main` contains the merge you expect
- the remote release branch is deleted

If a green PR is blocked by review requirements or admin-enforced branch protection, repository settings have drifted away from this protocol and must be corrected before the release proceeds.

If the local `release/X.Y.Z` branch still exists:

```bash
git branch -d release/X.Y.Z
```

## 5. Tag and push

```bash
git tag vX.Y.Z
git push origin vX.Y.Z

REPO=$(gh repo view --json nameWithOwner -q .nameWithOwner)
gh api "repos/$REPO/git/ref/tags/vX.Y.Z"
```

Do not continue until the remote tag ref exists.

The tag push triggers `release.yml`. The PR merge alone does not publish anything.

If the release workflow later needs a targeted rerun against the existing tag:

```bash
gh workflow run release.yml -f release_tag=vX.Y.Z
```

Never create a second tag or move an existing release tag just to retry publication.

## 6. Branch hygiene

After the merge and tag push, clean up stale remote-tracking refs and verify that no historical release branches remain on GitHub.

```bash
REPO=$(gh repo view --json nameWithOwner -q .nameWithOwner)
git remote prune origin
gh api "repos/$REPO/branches" --paginate --jq '.[].name'
```

Requirements:

- no `release/X.Y.Z` branch may remain on GitHub after the merge
- no historical `release/` branches may remain on GitHub; if any are present, delete them:

```bash
git push origin --delete release/A.B.C
```

- no fully merged local `release/` branches may remain; delete them:

```bash
git branch -d release/A.B.C
```

## 7. Monitor workflow runs

```bash
TAG_SHA=$(git rev-list -n 1 vX.Y.Z)
gh run list --workflow=release.yml --event=push --commit "$TAG_SHA" --limit=20
gh run list --workflow=release.yml --event=workflow_dispatch --commit "$TAG_SHA" --limit=20
```

Inspect failed runs with:

```bash
gh run view <run-id> --log-failed
```

Never treat one failed run as authoritative if another sibling run for the same tag already converged the release object onto the required state. The authoritative state is the GitHub release object and its assets.

## 8. Verify the GitHub release object

The release workflow is expected to create or converge the release object idempotently. Verify it directly:

```bash
gh release view vX.Y.Z --json tagName,isDraft,isPrerelease,publishedAt,url,assets
```

Requirements:

- the release exists for tag `vX.Y.Z`
- `isDraft` is `false`
- `isPrerelease` is `false`
- assets include:
  - `ffhn-X.Y.Z.zip`
  - `ffhn-X.Y.Z.tar.gz`
  - `ffhn-aarch64-apple-darwin`
  - `ffhn-aarch64-apple-darwin.sha256`
  - `ffhn-x86_64-apple-darwin`
  - `ffhn-x86_64-apple-darwin.sha256`
  - `ffhn-x86_64-unknown-linux-musl`
  - `ffhn-x86_64-unknown-linux-musl.sha256`
  - `ffhn-x86_64-pc-windows-msvc.exe`
  - `ffhn-x86_64-pc-windows-msvc.exe.sha256`

Workflow success is not authoritative. The release object and its assets are authoritative.

## 9. Verify the public binary

Download the published standalone artifacts, verify every checksum, and execute the host-native binary directly:

```bash
TMP_DIR="$(mktemp -d)"
gh release download vX.Y.Z \
  -p 'ffhn-aarch64-apple-darwin' \
  -p 'ffhn-aarch64-apple-darwin.sha256' \
  -p 'ffhn-x86_64-apple-darwin' \
  -p 'ffhn-x86_64-apple-darwin.sha256' \
  -p 'ffhn-x86_64-unknown-linux-musl' \
  -p 'ffhn-x86_64-unknown-linux-musl.sha256' \
  -p 'ffhn-x86_64-pc-windows-msvc.exe' \
  -p 'ffhn-x86_64-pc-windows-msvc.exe.sha256' \
  -D "$TMP_DIR"

(
  cd "$TMP_DIR"
  shasum -a 256 -c ffhn-aarch64-apple-darwin.sha256
  shasum -a 256 -c ffhn-x86_64-apple-darwin.sha256
  shasum -a 256 -c ffhn-x86_64-unknown-linux-musl.sha256
  shasum -a 256 -c ffhn-x86_64-pc-windows-msvc.exe.sha256
  chmod +x ./ffhn-aarch64-apple-darwin
  ./ffhn-aarch64-apple-darwin --version | grep "^ffhn X.Y.Z$"
  ./ffhn-aarch64-apple-darwin --help | grep "status"
)

rm -rf "$TMP_DIR"
```

Do not declare the release complete until every checksum validates and the downloaded host-native binary reports the target version.

The `grep "^ffhn X.Y.Z$"` check intentionally validates only the first line because `ffhn --version` is multi-line: it prints the version line first, then the product description from the workspace manifest.

## 10. Triage Dependabot PRs and clear dependency-automation leftovers

After the public release is verified, do not end the release session while open Dependabot PRs are still sitting untriaged.

Re-enumerate all open PRs:

```bash
gh pr list --state open \
  --json number,title,url,headRefName,mergeStateStatus,isDraft,author,statusCheckRollup
```

For each open Dependabot PR, inspect the exact payload and its current gate status:

```bash
gh pr diff <N> --name-only
gh pr view <N> --json number,title,state,mergeStateStatus,statusCheckRollup,url
```

Rules:

- If the PR is wanted, mergeable, and already green on `Check`, merge it immediately and delete its branch.
- If the PR is stale, superseded, intentionally rejected, or replaced by a different change path, close it explicitly and delete its branch.
- Never retag, amend, or move the just-published release tag to absorb a Dependabot change.
- There is no "ignore it and leave the branch there" option.

After each merge or close, resync and re-check GitHub branch state:

```bash
git checkout main
git pull
git remote prune origin
REPO=$(gh repo view --json nameWithOwner -q .nameWithOwner)
gh api "repos/$REPO/branches" --paginate --jq '.[].name'
```

## 11. Refresh the semver baseline

After the release is complete, refresh the checked-in semver baseline so future minor-version checks compare against the latest published API:

```bash
git checkout main
git pull
cargo xtask refresh-semver-baseline --git-ref vX.Y.Z
git add semver-baseline/ffhn-core
git commit -m "chore: refresh ffhn-core semver baseline"
git push
```

That command repackages the published Git ref into `semver-baseline/ffhn-core`, so the baseline cannot silently drift to unreleased local worktree state.
