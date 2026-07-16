---
afad: "4.0"
domain: RELEASE
updated: "2026-05-19"
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

Before any quality gate or version edit, identify the checkout the user will keep using after the release. Call it the primary checkout.

Run:

```bash
PRIMARY_CHECKOUT="$(git rev-parse --show-toplevel)"
CURRENT_VERSION="$(./scripts/workspace-package-field.sh version)"
RELEASE_VERSION="<set the intended release version, for example 8.1.0>"
TAG="v${RELEASE_VERSION}"
RELEASE_BRANCH="release/${RELEASE_VERSION}"
REPO="$(gh repo view --json nameWithOwner -q .nameWithOwner)"

git branch --show-current
git status --short
git fetch origin --prune --tags
git rev-list --left-right --count HEAD...origin/main
```

Requirements:

- the primary checkout path is known explicitly
- the intended public release version is named explicitly as `${RELEASE_VERSION}` before any branch or tag is created
- the primary checkout must not be left behind `origin/main` at release closeout
- if the primary checkout is already clean and current, release from it directly
- if the primary checkout has local work, is intentionally dirty, or lives on a problematic or slow filesystem, create a clean release worktree from the same repository and do the release there:

```bash
RELEASE_WORKTREE="$(mktemp -d -t ffhn-release-XXXXXX)"
git worktree add "$RELEASE_WORKTREE" origin/main
cd "$RELEASE_WORKTREE"
```

Use a Git worktree, not a disconnected clone, whenever possible. A worktree shares refs with the primary checkout and makes post-release reconciliation mechanically obvious. A separate clone is a last resort and, if used, must still be reconciled back into the primary checkout before the release session ends.

The maintained local release entrypoints fail closed on tracked checkout drift.
`build-release-source-archives.sh`, `build-release-artifact.sh`, `build-release-checksums.sh`,
`publish-github-release.sh`, and `verify-github-release.sh` therefore must run from a clean
tracked checkout or a clean release worktree.
Those maintained Bash entrypoints intentionally avoid Bash-4-only builtins, so stock macOS
`/bin/bash` is a supported host shell for the local release protocol.

If the primary checkout has unpublished local work, decide before the release whether that work is real or stale. Real work must move onto a named branch or exported patch before closeout. Stale work must be dropped. Never leave the primary checkout on stale `main` plus unpublished overlays.

The workspace version read into `${CURRENT_VERSION}` at this point may still be the previously released line. That is expected whenever the narrow `release/${RELEASE_VERSION}` branch itself owns the final version bump. Do not derive the release branch or tag from `${CURRENT_VERSION}` unless the tree already contains the intended release version.

If that real unpublished work changes shipped code, tests, docs, workflows, or release machinery beyond the narrow release-version delta itself, land it on `main` through the normal PR path before cutting `release/${RELEASE_VERSION}`. The release branch is for the final changelog dating, tag-triggering, and release-metadata convergence step, not for first publication of substantive product changes that still need ordinary review and CI on their own merits.

When that substantive pre-release work already includes the intended release notes, keep those entries under `## [Unreleased]` while landing the normal PR to `main`. The final `release/${RELEASE_VERSION}` branch must still carry a real narrow diff, typically converting the accumulated `Unreleased` entries into `## [${RELEASE_VERSION}] - YYYY-MM-DD` immediately before the release PR. If the working tree already has a dated release section before the substantive PR is merged, move those entries back under `Unreleased` first so the final release branch remains mechanically meaningful.

If the primary checkout is dirty because it already contains the intended release-candidate work, capture that state explicitly before creating the clean release worktree:

```bash
PREP_BRANCH="release-prep/${RELEASE_VERSION}"
git switch -c "$PREP_BRANCH"
git add <every already-intended release file>
git commit -m "chore: prepare ${RELEASE_VERSION} release candidate"
git fetch origin --prune --tags
RELEASE_WORKTREE="$(mktemp -d -t ffhn-release-XXXXXX)"
git worktree add -b "$RELEASE_BRANCH" "$RELEASE_WORKTREE" "$PREP_BRANCH"
cd "$RELEASE_WORKTREE"
```

That keeps the release worktree clean without discarding the real unpublished state that must ship. Do not hand-copy a dirty diff into a temporary checkout and hope it still matches later.

If the captured prep branch still contains substantive unpublished product changes rather than only the final release delta, treat that prep branch as the input to the normal pre-release PR to `main`, not as the final release branch itself. After that normal PR merges, cut `release/${RELEASE_VERSION}` from the updated `origin/main`.

Install the local maintainer toolchain if it is not already available by following [developer-setup.md](developer-setup.md). The stable workspace toolchain is owned by [../rust-toolchain.toml](../rust-toolchain.toml), and the exact maintainer toolchain plus QA-tool versions are owned by [../tooling/rust-tooling.env](../tooling/rust-tooling.env). The pinned QA nightly toolchain exists for the maintained Miri proof, the coverage gate, and optional manual sanitizer-backed fuzz runs.

Run the single local quality gate first:

```bash
./check.sh
```

or equivalently:

```bash
cargo xtask check
```

That gate must succeed before any final release commit or tag. The maintained definition of that gate lives in [quality-gates.md](quality-gates.md).

If the current substantive pre-release work or release-prep diff touches the contributor-devcontainer
surface, run the dedicated contributor-environment proof before you push the branch for review. The
trigger set is the same one that drives CI's `contributor-devcontainer-gate`:

- `.github/workflows/ci.yml`
- `.devcontainer/`
- `tooling/rust-tooling.env`
- `scripts/bootstrap-rust-tools.sh`
- `scripts/validate-devcontainer.sh`
- `scripts/run-devcontainer-check.sh`
- `scripts/devcontainer-prepare-user-home.sh`
- `scripts/devcontainer-cli-helper.Dockerfile`
- `scripts/common.sh`
- `check.sh`

When any of those paths are in scope, run:

```bash
./scripts/validate-devcontainer.sh
FFHN_DEVCONTAINER_SKIP_BUILD=1 ./scripts/run-devcontainer-check.sh
```

`./check.sh` proves the shipped Rust/product contract. The two devcontainer entrypoints prove the
committed contributor environment, the raw Docker image contract, the Dev Container client path, and
the full headless maintainer gate through that environment. The validator also promotes the
validated contributor image into the canonical local tag `ffhn-devcontainer:local`, so the
`FFHN_DEVCONTAINER_SKIP_BUILD=1` path reuses the exact image that just passed validation instead of
silently drifting onto an older local tag.

FFHN keeps normal Cargo output out of the repository tree by default through
[../.cargo/config.toml](../.cargo/config.toml), which points the maintained host-native path at the
managed sibling artifact roots documented in [hygiene.md](hygiene.md). If the release session needs
a different location, override both `CARGO_TARGET_DIR` and `CARGO_BUILD_BUILD_DIR` for the session
instead of reintroducing repo-local build output.

Then verify:

- `Cargo.toml` `[workspace.package] version` equals `${RELEASE_VERSION}` exactly. This is the single version source of truth for both crates and for `ffhn --version`.
- `Cargo.toml` `[workspace.package] description` still reflects the current product in task-facing language.
- `changelog.md` has a `## [${RELEASE_VERSION}] - YYYY-MM-DD` section with at least one entry.
- `README.md` still documents the current install flow, CLI model, durable runtime model, and the exact public release asset names.
- `CONTRIBUTING.md` still matches the maintained contributor and release workflow.
- `docs/README.md` still points at the maintained developer and maintainer docs.
- `docs/versioning-policy.md` still matches the shipped typed-observation contract policy and semver-baseline rules.
- `docs/cli.md`, `docs/core.md`, `docs/contracts.md`, `docs/reports.md`, `docs/run-reports.md`, and `docs/targets.md` still match the shipped surfaces.
- `docs/quality-gates.md` still matches `cargo xtask`.
- `docs/operations.md` still matches the release scripts, asset matrix, checksum-manifest flow, and workflow structure.
- `docs/platform-support.md` still matches the shipped release target matrix, package contents, and deployment floors.
- `Cargo.toml` still defines the `dist` Cargo profile used for shipped public binaries.
- GitHub settings still satisfy the release assumptions used by this protocol:
  - default branch `main`
  - `delete_branch_on_merge` enabled
  - `main` protected
  - no required approving reviews on `main`
  - no admin-enforced branch protection override requirement
  - required conversation resolution before merge
  - required status checks exactly:
    - `Check`

Before cutting the release branch, enumerate open PRs so dependency-automation work is never surprise-discovered after publication:

```bash
gh pr list --state open \
  --json number,title,url,headRefName,mergeStateStatus,isDraft,author,statusCheckRollup
```

If any open PR is authored by Dependabot, usually surfaced as `app/dependabot` and sometimes as `dependabot[bot]` in `author.login`, decide up front whether it changes release machinery, release assets, or release-critical dependencies. If it does, land or reject it before cutting the release branch. If it does not, carry that decision forward and complete Step 10 before ending the release session.

## 2. Release branch

Do release commits on a release branch, not directly on `main`.

```bash
if [ "$(git branch --show-current)" != "$RELEASE_BRANCH" ]; then
  git switch -c "$RELEASE_BRANCH"
fi
git add -A
git status --short
git diff --cached --name-status
git diff --cached --stat
git commit -m "release: bump version to ${RELEASE_VERSION}"
git push -u origin "$RELEASE_BRANCH"
```

Before committing:

- `git status --short` must show no intended release file left unstaged
- `git diff --cached --name-status` must show the exact release file set
- `git diff --cached --stat` must reflect versioning, changelog dating, docs, workflow, and release-script updates only
- if the branch carries broad product changes instead of the final release delta, stop and land those changes on `main` first through the normal PR path

## 3. Pull request and CI

```bash
PR_URL="$(gh pr create \
  --title "release: bump version to ${RELEASE_VERSION}" \
  --base main \
  --head "$RELEASE_BRANCH" \
  --body "Release ${RELEASE_VERSION}")"
PR_NUMBER="$(gh pr view "$PR_URL" --json number --jq '.number')"

gh pr diff "$PR_NUMBER" --name-only
gh pr view "$PR_NUMBER" --json number,state,mergeStateStatus,statusCheckRollup,url
gh pr checks "$PR_NUMBER"
```

If `gh pr diff "$PR_NUMBER" --name-only` fails with HTTP `406` because the diff exceeds GitHub's line-limit for that endpoint, enumerate the changed file set through the pull-files API instead:

```bash
gh api "repos/$REPO/pulls/$PR_NUMBER/files" --paginate --jq '.[].filename'
```

Do not continue until the required job in workflow `CI` is green:

- `Check`

`Check` is the aggregate branch-protection gate. It must reflect the Linux maintainer gate, the cross-platform Rust gate, and the release-target smoke matrix.

`gh pr checks` is the maintained first-line gate view, but it is not a reliable step-progress
monitor for FFHN's longest jobs. If long-running checks remain generic `pending` with no useful
elapsed detail, inspect the underlying Actions jobs directly before deciding whether the workflow is
healthy or hung:

```bash
gh pr view "$PR_NUMBER" --json statusCheckRollup
gh api "repos/$REPO/actions/jobs/<JOB_ID>"
```

Use the `detailsUrl` or check-run names from `statusCheckRollup` to identify the relevant job ids.
Treat the job API as the authoritative live progress view when `gh pr checks` lags or omits step
detail.

If the PR is open and mergeable but `gh pr checks "$PR_NUMBER"` still reports no checks and the `CI` workflow has no `pull_request` run for `${RELEASE_BRANCH}` after a short wait, treat that as a delivery failure, not as permission to merge without CI.

Recover in this order:

1. Close and reopen the PR once to retrigger the `pull_request` workflow without changing release contents.
2. Re-check:

```bash
gh pr close "$PR_NUMBER"
gh pr reopen "$PR_NUMBER"
gh pr checks "$PR_NUMBER"
gh run list --workflow=ci.yml --branch "$RELEASE_BRANCH" --limit 10
```

3. If `Check` is still absent, push one more commit to the release branch to force a `pull_request` synchronize event. Prefer a real corrective follow-up commit when the protocol or release docs genuinely need refinement; use an empty retrigger commit only as the last resort.
4. If the synchronize event still does not produce `Check`, dispatch the `CI` workflow manually against the release branch and wait for the resulting `Check` status:

```bash
gh workflow run ci.yml --ref "$RELEASE_BRANCH"
gh run list --workflow=ci.yml --branch "$RELEASE_BRANCH" --limit 10
gh pr checks "$PR_NUMBER"
```

`CI` intentionally exposes `workflow_dispatch` for this maintainer recovery path. If `Check` still never materializes after the manual dispatch, stop and investigate repository or GitHub-side drift instead of merging blind.

## 4. Merge handoff

```bash
gh pr merge "$PR_NUMBER" --merge --delete-branch --subject "release: bump version to ${RELEASE_VERSION} (#${PR_NUMBER})"
git fetch origin --prune --tags
git checkout --detach origin/main
gh pr view "$PR_NUMBER" --json number,state,mergedAt,headRefName,baseRefName,url
```

Verify:

- PR state is `MERGED`
- `mergedAt` is populated
- the synced release checkout now reflects `origin/main` at the merged release commit
- the remote release branch is deleted

If a green PR is blocked only because conversations are unresolved, resolve or close those threads and then merge normally.

If a green PR is blocked by review requirements or admin-enforced branch protection, repository settings have drifted away from this protocol and must be corrected before the release proceeds.

If the local release branch still exists:

```bash
git branch -d "$RELEASE_BRANCH"
```

## 5. Tag and push

```bash
git tag "$TAG"
git push origin "$TAG"
gh api "repos/$REPO/git/ref/tags/$TAG"
```

Do not continue until the remote tag ref exists.

The tag push triggers `release.yml`. The PR merge alone does not publish anything.

If the release workflow later needs a targeted rerun against the existing tag:

```bash
gh workflow run release.yml -f release_tag="$TAG"
```

Never create a second tag or move an existing release tag just to retry publication.

The release workflow follows a draft-first publication model: it creates or reuses a draft release, uploads the full maintained asset inventory, writes the checksum manifest, and only then publishes the release. A rerun may repair an in-progress draft release. It must not backfill missing assets into an already-published release.

The rerun path executes the maintained publication scripts from `main`, but it passes the selected tag version into those scripts explicitly through `RELEASE_VERSION`. That keeps the expected asset inventory pinned to `${TAG}` even if `main` has already advanced to a newer workspace version.

## 6. Branch hygiene

After the merge and tag push, clean up stale remote-tracking refs and verify that no historical release branches remain on GitHub.

```bash
git remote prune origin
gh api "repos/$REPO/branches" --paginate --jq '.[].name'
```

Requirements:

- no `${RELEASE_BRANCH}` branch may remain on GitHub after the merge
- no historical `release/` branches may remain on GitHub
- no fully merged local `release/` branches may remain

If you find stale release branches, remove them explicitly with `git push origin --delete release/...` and `git branch -d release/...`.

Open maintenance branches such as Dependabot are handled separately in Step 10. Do not treat a non-`release/` branch as automatically acceptable just because Step 6 only hard-fails `release/*` leftovers.

## 7. Monitor workflow runs

```bash
TAG_SHA="$(git rev-list -n 1 "$TAG")"
gh run list --workflow=release.yml --event=push --commit "$TAG_SHA" --limit=20
gh run list --workflow=release.yml --event=workflow_dispatch --commit "$TAG_SHA" --limit=20
```

To inspect the latest run for that tag in detail:

```bash
RUN_ID="$(gh run list --workflow=release.yml --commit "$TAG_SHA" --limit 1 --json databaseId --jq '.[0].databaseId')"
gh run view "$RUN_ID" --log-failed
```

Never treat one failed or cancelled sibling run as authoritative if another run for the same tag already converged the release object onto the required state. The authoritative state is the GitHub release object and its assets, not the first workflow run you happen to inspect.

## 8. Verify the GitHub release object

The release workflow is expected to create or converge the release object idempotently. Verify it directly:

```bash
gh release view "$TAG" --json tagName,isDraft,isPrerelease,publishedAt,url,assets
```

Requirements:

- the release exists for tag `${TAG}`
- `isDraft` is `false`
- `isPrerelease` is `false`
- assets include:
  - `ffhn-source-${RELEASE_VERSION}.zip`
  - `ffhn-source-${RELEASE_VERSION}.tar.gz`
  - `ffhn-${RELEASE_VERSION}-aarch64-apple-darwin.tar.gz`
  - `ffhn-${RELEASE_VERSION}-x86_64-apple-darwin.tar.gz`
  - `ffhn-${RELEASE_VERSION}-x86_64-unknown-linux-musl.tar.gz`
  - `ffhn-${RELEASE_VERSION}-x86_64-pc-windows-msvc.zip`
  - `ffhn-${RELEASE_VERSION}-checksums.txt`

Workflow success is not authoritative. The release object and its assets are authoritative.
GitHub Actions also emits build provenance attestations for the source archives, standalone packages, and checksum manifest, but this protocol's blocking verification keys on the release object and the maintained asset inventory rather than those separate attestation records.

GitHub will also render `Source code (zip)` and `Source code (tar.gz)` links on the release page. Those links are GitHub-generated convenience downloads and are not part of FFHN's maintained asset inventory.
The maintained `ffhn-source-*` assets are built through [`scripts/build-release-source-archives.sh`](../scripts/build-release-source-archives.sh), which uses `git archive` so `.gitattributes export-ignore` keeps maintainer-only agent and automation configuration out of those shipped source archives even when that configuration is committed in the repository.

## 9. Verify the public binary

Download the maintained host-native release package plus the checksum manifest, validate the checksum, and execute the packaged binary:

```bash
case "$(uname -s)/$(uname -m)" in
  Darwin/arm64)
    HOST_TARGET="aarch64-apple-darwin"
    HOST_ARCHIVE="ffhn-${RELEASE_VERSION}-${HOST_TARGET}.tar.gz"
    ;;
  Darwin/x86_64)
    HOST_TARGET="x86_64-apple-darwin"
    HOST_ARCHIVE="ffhn-${RELEASE_VERSION}-${HOST_TARGET}.tar.gz"
    ;;
  Linux/x86_64)
    HOST_TARGET="x86_64-unknown-linux-musl"
    HOST_ARCHIVE="ffhn-${RELEASE_VERSION}-${HOST_TARGET}.tar.gz"
    ;;
  *)
    printf 'unsupported host for local post-release binary verification: %s/%s\n' "$(uname -s)" "$(uname -m)" >&2
    exit 1
    ;;
esac

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

gh release download "$TAG" \
  -p "$HOST_ARCHIVE" \
  -p "ffhn-${RELEASE_VERSION}-checksums.txt" \
  -D "$TMP_DIR"

(
  cd "$TMP_DIR"
  if command -v shasum >/dev/null 2>&1; then
    grep "  ${HOST_ARCHIVE}$" "ffhn-${RELEASE_VERSION}-checksums.txt" | shasum -a 256 -c
  else
    grep "  ${HOST_ARCHIVE}$" "ffhn-${RELEASE_VERSION}-checksums.txt" | sha256sum -c
  fi

  tar -xzf "$HOST_ARCHIVE"
  VERSION_OUTPUT="$("./ffhn-${RELEASE_VERSION}-${HOST_TARGET}/ffhn" --version | tr -d '\r')"
  [ "$VERSION_OUTPUT" = "ffhn ${RELEASE_VERSION}" ]
  "./ffhn-${RELEASE_VERSION}-${HOST_TARGET}/ffhn" --help | grep "status"
)

trap - EXIT
rm -rf "$TMP_DIR"
```

Do not declare the release complete until the checksum manifest validates and the downloaded host-native binary reports the target version.

The release workflow already performs packaged runtime smoke on each target's native runner. The local command above is an additional checksum plus host-native runtime verification step.

## 10. Triage Dependabot PRs and clear dependency-automation leftovers

After the public release is verified, do not end the release session while open Dependabot PRs are still sitting untriaged. Release hygiene includes dependency-automation hygiene.

Re-enumerate all open PRs and identify Dependabot-owned entries directly from GitHub metadata:

```bash
gh pr list --state open \
  --json number,title,url,headRefName,mergeStateStatus,isDraft,author,statusCheckRollup
```

Treat any PR whose `author.login` is `app/dependabot` or `dependabot[bot]` as in scope for this step, even if it was already reviewed during Step 1. Step 1 creates the release-time decision; Step 10 closes the loop before the release session is allowed to end.

For each open Dependabot PR, inspect the exact payload and its current gate status:

```bash
gh pr diff <N> --name-only
gh pr view <N> --json number,title,state,mergeStateStatus,statusCheckRollup,url
```

If `gh pr diff <N> --name-only` fails with HTTP `406` because the diff exceeds GitHub's line-limit for that endpoint, enumerate the changed file set through the pull-files API instead:

```bash
gh api "repos/$REPO/pulls/<N>/files" --paginate --jq '.[].filename'
```

Rules:

- If the PR is wanted, mergeable, and already green on the required `Check` status, merge it immediately with `gh pr merge <N> --merge --delete-branch`.
- If the PR is stale, superseded by `main`, intentionally rejected, or replaced by a different change path, close it explicitly and delete its branch with `gh pr close <N> --comment "Superseded or intentionally rejected during release hygiene." --delete-branch`.
- If the PR needs follow-up work before it is acceptable, do that work as a normal post-release change on `main` and then land or replace the Dependabot PR. Do not leave a green but unattended Dependabot PR parked indefinitely just because the release itself already shipped.
- Never retag, amend, or move the just-published release tag to absorb a Dependabot change. The published release remains immutable. Dependabot resolution is post-release `main` hygiene.
- There is no "ignore it and leave the branch there" option. Every open Dependabot PR must end this step in exactly one of these states:
  - merged and branch deleted
  - closed and branch deleted
  - consciously kept open with an explicit still-valid reason

After each merge or close, resync and re-check GitHub branch state:

```bash
git fetch origin --prune --tags
git checkout --detach origin/main
gh api "repos/$REPO/branches" --paginate --jq '.[].name'
```

Requirements before declaring the release session complete:

- No stale Dependabot PR may remain open without an explicit keep-open decision.
- No merged or closed Dependabot branch may remain on GitHub.
- Any remaining non-`main` branch on GitHub must correspond to an intentional still-open PR that was reviewed during this step and deliberately kept alive.

## 11. Refresh the semver baseline

After the release is complete, refresh the checked-in semver baseline so future semver checks compare against the latest published API.

The maintained path is a short follow-up branch and PR rooted at the released `origin/main` state. That avoids relying on direct pushes to protected `main`, and it still works if the release itself was cut from a dedicated worktree.

```bash
git fetch origin --prune --tags
git checkout --detach origin/main
cargo xtask refresh-semver-baseline --git-ref "$TAG"
FOLLOWUP_BRANCH="chore/refresh-semver-baseline-${TAG}"
git switch -c "$FOLLOWUP_BRANCH"
git add semver-baseline/ffhn-core
git commit -m "chore: refresh ffhn-core semver baseline"
git push -u origin "$FOLLOWUP_BRANCH"
BASELINE_PR_URL="$(gh pr create \
  --title "chore: refresh ffhn-core semver baseline" \
  --body "Refresh the checked-in ffhn-core semver baseline to ${TAG} after the public release.")"
gh pr checks "$BASELINE_PR_URL" --watch
gh pr merge "$BASELINE_PR_URL" --merge --delete-branch
git fetch origin --prune --tags
git checkout --detach origin/main
```

That command repackages the published Git ref into `semver-baseline/ffhn-core`, so the baseline cannot silently drift to unreleased local worktree state.
The follow-up PR is still a normal protected change against `main`, so wait for the required `Check` status before merging it instead of assuming the short generated diff can skip branch protection.

When the current checkout can safely hold `main`, the equivalent branch-bound sync remains:

```bash
git fetch origin --prune --tags
git merge --ff-only origin/main
```

## 12. Reconcile the primary checkout

If the release used a dedicated release worktree or any checkout other than the primary checkout, the session is not complete until the primary checkout is truthful again. This is a blocking release closeout gate, not an advisory cleanup reminder.

If unpublished local work from the primary checkout is still needed, move it onto a named branch based on current `main` first, then return the primary checkout itself to `main`.

If the disposable release worktree still has `main` checked out, Git will refuse `git -C "$PRIMARY_CHECKOUT" checkout main`. Detach that disposable worktree first:

```bash
git -C "$RELEASE_WORKTREE" checkout --detach
```

Run:

```bash
git -C "$PRIMARY_CHECKOUT" fetch origin --prune --tags
git -C "$PRIMARY_CHECKOUT" checkout main
git -C "$PRIMARY_CHECKOUT" rev-list --left-right --count HEAD...origin/main
git -C "$PRIMARY_CHECKOUT" merge --ff-only origin/main
git -C "$PRIMARY_CHECKOUT" rev-parse HEAD
git -C "$PRIMARY_CHECKOUT" status --short
```

If a temporary `release-prep/${RELEASE_VERSION}` branch was created only to capture dirty release-candidate state and the shipped `main` history has absorbed it, delete that stale prep branch explicitly:

```bash
git -C "$PRIMARY_CHECKOUT" branch -d "release-prep/${RELEASE_VERSION}"
```

Requirements before declaring the release session complete:

- the primary checkout `HEAD` equals `origin/main`
- the primary checkout `Cargo.toml` and `changelog.md` reflect the released version
- no stale release-only checkout may be left behind with the appearance of being authoritative
- no stale local `release-prep/` branch may remain once the shipped `main` history fully absorbs it
- if unpublished local work from the primary checkout is still needed, replay it deliberately onto a named branch based on current `main`; do not leave it only in a stash or mixed back into `main`
- if that unpublished local work is stale, superseded, or regresses the shipped release state, delete it instead of preserving misleading debris

Do not declare the release complete until every condition above is true at the same time.

If a disposable release worktree was created and is no longer needed:

```bash
if [ -n "${RELEASE_WORKTREE:-}" ] && [ -d "${RELEASE_WORKTREE}" ]; then
  git worktree remove "$RELEASE_WORKTREE"
fi
```
