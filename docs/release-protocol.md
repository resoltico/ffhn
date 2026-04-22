---
afad: "3.5"
version: "2.0.1"
domain: RELEASE
updated: "2026-04-22"
route:
  keywords: [release protocol, gh cli, tag push, release workflow, semver baseline, verification]
  questions: ["how do I release ffhn?", "what must be verified before tagging a release?", "when do I refresh the ffhn semver baseline?"]
---

# Release Protocol

The release flow is driven by the GitHub CLI (`gh`). Every step that touches GitHub uses `gh`,
not the GitHub web UI.

Release choreography lives here. Contract-versioning policy lives in
[versioning-policy.md](versioning-policy.md).

## 0. GitHub CLI gate

Before doing anything else:

```bash
gh --version
gh auth status
```

If either command fails, stop immediately.

## 1. Pre-flight

Before any quality gate or version edit, identify the checkout the user will keep using after the
release. Call it the primary checkout.

Run:

```bash
git rev-parse --show-toplevel
git branch --show-current
git status --short
git fetch origin --prune --tags
git rev-list --left-right --count HEAD...origin/main
```

Requirements:

- the primary checkout path is known explicitly
- the primary checkout must not be left behind `origin/main` at release closeout
- if the primary checkout is already clean and current, release from it directly
- if the primary checkout has local work, is intentionally dirty, or lives on a problematic or slow
  filesystem, create a clean release worktree from the same repository and do the release there:

```bash
PRIMARY_CHECKOUT=$(git rev-parse --show-toplevel)
git fetch origin --prune --tags
RELEASE_WORKTREE="$(mktemp -d -t ffhn-release-XXXXXX)"
git worktree add "$RELEASE_WORKTREE" origin/main
cd "$RELEASE_WORKTREE"
```

Use a Git worktree, not a disconnected clone, whenever possible. A worktree shares refs with the
primary checkout and makes post-release reconciliation mechanically obvious. A separate clone is a
last resort and, if used, must still be reconciled back into the primary checkout before the
release session ends.

If the primary checkout has unpublished local work, decide before the release whether that work is
real or stale. Real work must move onto a named branch or exported patch before closeout. Stale
work must be dropped. Never leave the primary checkout on stale `main` plus unpublished overlays.

Install the local maintainer toolchain if it is not already available by following
[developer-setup.md](developer-setup.md). That guide owns the maintained bootstrap commands for
`rustup`, the Cargo QA tools, `shellcheck`, and `gh`.

Stable remains the default FFHN toolchain. Nightly is installed alongside stable only for the
coverage gate and manual sanitizer-backed fuzz runs.

Run the single local quality gate first:

```bash
./check.sh
```

or equivalently:

```bash
cargo xtask check
```

That gate must succeed before any release commit or tag. The maintained definition of that gate
lives in [quality-gates.md](quality-gates.md).

Then verify:

- `Cargo.toml` `[workspace.package] version` equals the target release version exactly. This is the single version source of truth for both crates and for `ffhn --version`.
- `Cargo.toml` `[workspace.package] description` still reflects the current product in task-facing language.
- `changelog.md` has a `## [X.Y.Z] - YYYY-MM-DD` section with at least one entry.
- `README.md` still documents the current install flow, CLI model, durable runtime model, and the exact public release asset names.
- `CONTRIBUTING.md` still matches the maintained contributor and release workflow.
- `docs/README.md` still points at the maintained developer and maintainer docs.
- `docs/versioning-policy.md` still matches the shipped contract policy, frozen HTMLCut interop model, and semver-baseline rules.
- `docs/cli.md`, `docs/core.md`, `docs/contracts.md`, `docs/reports.md`, and `docs/targets.md` still match the shipped surfaces.
- `docs/quality-gates.md` still matches `cargo xtask`.
- `docs/operations.md` still matches the release scripts, asset matrix, checksum-manifest flow, and workflow structure.
- `docs/platform-support.md` still matches the shipped release target matrix, package contents, and deployment floors.
- `README.md` still documents the release asset names:
  - `ffhn-source-X.Y.Z.zip`
  - `ffhn-source-X.Y.Z.tar.gz`
  - `ffhn-X.Y.Z-aarch64-apple-darwin.tar.gz`
  - `ffhn-X.Y.Z-x86_64-apple-darwin.tar.gz`
  - `ffhn-X.Y.Z-x86_64-unknown-linux-musl.tar.gz`
  - `ffhn-X.Y.Z-x86_64-pc-windows-msvc.zip`
  - `ffhn-X.Y.Z-checksums.txt`
- `Cargo.toml` still defines the `dist` Cargo profile used for shipped public binaries.
- repository settings are aligned with this protocol:
  - default branch is `main`
  - `delete_branch_on_merge` is enabled
  - `main` is protected
  - `main` does not require approving reviews
  - `main` does not enforce branch protection for admins
  - required status checks are exactly:
    - `Check`

Before cutting the release branch, enumerate open PRs so dependency-automation work is never
surprise-discovered after publication:

```bash
gh pr list --state open \
  --json number,title,url,headRefName,mergeStateStatus,isDraft,author,statusCheckRollup
```

If any open PR is authored by `dependabot[bot]`, decide up front whether it changes release
machinery, release assets, or release-critical dependencies. If it does, land or reject it before
cutting the release branch. If it does not, carry that decision forward and complete Step 10
before ending the release session.

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
- `git diff --cached --stat` must reflect versioning, changelog, docs, workflow, and release-script updates only.

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

`Check` is the aggregate branch-protection gate. It must reflect both the Rust maintainer gate and
the release-target smoke matrix.

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

If a green PR is blocked by review requirements or admin-enforced branch protection, repository
settings have drifted away from this protocol and must be corrected before the release proceeds.

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

The release workflow now follows a draft-first publication model: it creates or reuses a draft
release, uploads the full maintained asset inventory, writes the checksum manifest, and only then
publishes the release. A rerun may repair an in-progress draft release. It must not backfill
missing assets into an already-published release.

The rerun path executes the maintained publication scripts from `main`, but it passes the selected
tag version into those scripts explicitly through `RELEASE_VERSION`. That keeps the expected asset
inventory pinned to `vX.Y.Z` even if `main` has already advanced to a newer workspace version.

## 6. Branch hygiene

After the merge and tag push, clean up stale remote-tracking refs and verify that no historical
release branches remain on GitHub.

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

Open maintenance branches such as Dependabot are handled separately in Step 10. Do not treat a
non-`release/` branch as automatically acceptable just because Step 6 only hard-fails
`release/*` leftovers.

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

Never treat one failed run as authoritative if another sibling run for the same tag already
converged the release object onto the required state. The authoritative state is the GitHub
release object and its assets, not the first workflow run you happen to inspect.

## 8. Verify the GitHub release object

The release workflow is expected to create or converge the release object idempotently. Verify it
directly:

```bash
gh release view vX.Y.Z --json tagName,isDraft,isPrerelease,publishedAt,url,assets
```

Requirements:

- the release exists for tag `vX.Y.Z`
- `isDraft` is `false`
- `isPrerelease` is `false`
- assets include:
  - `ffhn-source-X.Y.Z.zip`
  - `ffhn-source-X.Y.Z.tar.gz`
  - `ffhn-X.Y.Z-aarch64-apple-darwin.tar.gz`
  - `ffhn-X.Y.Z-x86_64-apple-darwin.tar.gz`
  - `ffhn-X.Y.Z-x86_64-unknown-linux-musl.tar.gz`
  - `ffhn-X.Y.Z-x86_64-pc-windows-msvc.zip`
  - `ffhn-X.Y.Z-checksums.txt`

Workflow success is not authoritative. The release object and its assets are authoritative.

GitHub will also render `Source code (zip)` and `Source code (tar.gz)` links on the release page.
Those links are GitHub-generated convenience downloads and are not part of FFHN's maintained
asset inventory.

## 9. Verify the public binary

Download the maintained release assets, verify the checksum manifest, and execute the host-native
binary from its extracted package:

```bash
TMP_DIR="$(mktemp -d)"
gh release download vX.Y.Z \
  -p 'ffhn-source-X.Y.Z.zip' \
  -p 'ffhn-source-X.Y.Z.tar.gz' \
  -p 'ffhn-X.Y.Z-aarch64-apple-darwin.tar.gz' \
  -p 'ffhn-X.Y.Z-x86_64-apple-darwin.tar.gz' \
  -p 'ffhn-X.Y.Z-x86_64-unknown-linux-musl.tar.gz' \
  -p 'ffhn-X.Y.Z-x86_64-pc-windows-msvc.zip' \
  -p 'ffhn-X.Y.Z-checksums.txt' \
  -D "$TMP_DIR"

(
  cd "$TMP_DIR"
  shasum -a 256 -c ffhn-X.Y.Z-checksums.txt
  tar -xzf ./ffhn-X.Y.Z-aarch64-apple-darwin.tar.gz
  ./ffhn-X.Y.Z-aarch64-apple-darwin/ffhn --version | grep "^ffhn X.Y.Z$"
  ./ffhn-X.Y.Z-aarch64-apple-darwin/ffhn --help | grep "status"
)

rm -rf "$TMP_DIR"
```

Do not declare the release complete until the checksum manifest validates and the downloaded
host-native binary reports the target version.

The release workflow itself already performs runtime smoke on each target's native runner. The
local post-release command above is an additional asset-integrity check plus a host-native runtime
verification step.

## 10. Triage Dependabot PRs and clear dependency-automation leftovers

After the public release is verified, do not end the release session while open Dependabot PRs are
still sitting untriaged. Release hygiene includes dependency-automation hygiene.

Re-enumerate all open PRs and identify Dependabot-owned entries directly from GitHub metadata:

```bash
gh pr list --state open \
  --json number,title,url,headRefName,mergeStateStatus,isDraft,author,statusCheckRollup
```

Treat any PR whose `author.login` is `dependabot[bot]` as in scope for this step, even if it was
already reviewed during Step 1. Step 1 creates the release-time decision; Step 10 closes the loop
before the release session is allowed to end.

For each open Dependabot PR, inspect the exact payload and its current gate status:

```bash
gh pr diff <N> --name-only
gh pr view <N> --json number,title,state,mergeStateStatus,statusCheckRollup,url
```

Rules:

- If the PR is wanted, mergeable, and already green on the required `CI` checks, merge it
  immediately and delete its branch:

```bash
gh pr merge <N> --merge --delete-branch --subject "<title> (#<N>)"
```

- If the PR is stale, superseded by `main`, intentionally rejected, or replaced by a different
  change path, close it explicitly and delete its branch:

```bash
gh pr close <N> --comment "Superseded or intentionally rejected during release hygiene." --delete-branch
```

- If the PR needs follow-up work before it is acceptable, do that work as a normal post-release
  change on `main` and then land or replace the Dependabot PR. Do not leave a green but
  unattended Dependabot PR parked indefinitely just because the release itself already shipped.

- Never retag, amend, or move the just-published release tag to absorb a Dependabot change. The
  published release remains immutable. Dependabot resolution is post-release `main` hygiene.

- There is no "ignore it and leave the branch there" option. Every open Dependabot PR must end
  this step in exactly one of these states:
  - merged and branch deleted
  - closed and branch deleted
  - consciously kept open with an explicit still-valid reason

After each merge or close, resync and re-check GitHub branch state:

```bash
git checkout main
git pull
git remote prune origin
REPO=$(gh repo view --json nameWithOwner -q .nameWithOwner)
gh api "repos/$REPO/branches" --paginate --jq '.[].name'
```

Requirements before declaring the release session complete:

- No stale Dependabot PR may remain open without an explicit keep-open decision.
- No merged or closed Dependabot branch may remain on GitHub.
- Any remaining non-`main` branch on GitHub must correspond to an intentional still-open PR that
  was reviewed during this step and deliberately kept alive.

## 11. Refresh the semver baseline

After the release is complete, refresh the checked-in semver baseline so future minor-version
checks compare against the latest published API:

```bash
git checkout main
git pull
cargo xtask refresh-semver-baseline --git-ref vX.Y.Z
git add semver-baseline/ffhn-core
git commit -m "chore: refresh ffhn-core semver baseline"
```

That command repackages the published Git ref into `semver-baseline/ffhn-core`, so the baseline
cannot silently drift to unreleased local worktree state.

Treat that baseline refresh as an ordinary post-release change, not as an exception to branch
protection. If `main` is protected against direct pushes, move the commit onto a short-lived
follow-up branch and land it through the normal PR path:

```bash
git switch -c chore/refresh-semver-baseline-vX.Y.Z
git push -u origin chore/refresh-semver-baseline-vX.Y.Z
gh pr create \
  --title "chore: refresh ffhn-core semver baseline" \
  --body "Refresh the checked-in ffhn-core semver baseline to vX.Y.Z after the public release."
gh pr merge --merge --delete-branch
git checkout main
git pull --ff-only
```

Only bypass that PR flow if the repository explicitly allows trusted maintainers to push this
post-release housekeeping commit directly to `main`.

## 12. Reconcile the Primary Checkout

If the release used a dedicated release worktree or any checkout other than the primary checkout,
the session is not complete until the primary checkout is truthful again. This is a blocking
release closeout gate, not an advisory cleanup reminder.

If unpublished local work from the primary checkout is still needed, move it onto a named branch
based on current `main` first, then return the primary checkout itself to `main`.

Run:

```bash
git -C "$PRIMARY_CHECKOUT" fetch origin --prune --tags
git -C "$PRIMARY_CHECKOUT" checkout main
git -C "$PRIMARY_CHECKOUT" rev-list --left-right --count HEAD...origin/main
git -C "$PRIMARY_CHECKOUT" merge --ff-only origin/main
git -C "$PRIMARY_CHECKOUT" rev-parse HEAD
git -C "$PRIMARY_CHECKOUT" status --short
```

Requirements before declaring the release session complete:

- the primary checkout `HEAD` equals `origin/main`
- the primary checkout `Cargo.toml` and `changelog.md` reflect the released version
- no stale release-only checkout may be left behind with the appearance of being authoritative
- if unpublished local work from the primary checkout is still needed, replay it deliberately onto a
  named branch based on current `main`; do not leave it only in a stash or mixed back into `main`
- if that unpublished local work is stale, superseded, or regresses the shipped release state,
  delete it instead of preserving misleading debris

Do not declare the release complete until every condition above is true at the same time.

If a disposable release worktree was created and is no longer needed:

```bash
git worktree remove "$RELEASE_WORKTREE"
```
