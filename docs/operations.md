---
afad: "4.0"
domain: OPERATIONS
updated: "2026-05-15"
route:
  keywords: [operations, check.sh, release scripts, ci workflow, dist profile, github release, supported targets, release packages, checksum manifest]
  questions: ["how do I operate ffhn locally?", "how do the ffhn release scripts work?", "which standalone targets does ffhn publish?", "which FFHN release assets are published?"]
---

# Operations

This page covers the maintained operational surface around FFHN, not the end-user CLI contract or release-session choreography.

Release choreography lives in [release-protocol.md](release-protocol.md). Contract-versioning policy lives in [versioning-policy.md](versioning-policy.md).

## Local Maintainer Commands

Canonical maintainer entrypoint:

```bash
./check.sh
```

Equivalent direct command:

```bash
cargo xtask check
```

Compatibility wrapper:

```bash
./scripts/qa-gate.sh
```

Targeted maintainer commands:

```bash
cargo xtask semver-check
cargo xtask coverage
cargo xtask hygiene report
cargo xtask hygiene clean --mode safe
cargo xtask refresh-semver-baseline --git-ref vX.Y.Z
./scripts/validate-devcontainer.sh
./scripts/run-devcontainer-check.sh
```

## CI Workflows

`/.github/workflows/ci.yml` uses two helper jobs, four work lanes, and one aggregate required-check job:

1. `release-target-matrix`: computes the standalone release-target matrix
2. `rust-gate`: installs toolchains and QA tools, then runs `./check.sh`
3. `devcontainer-changes`: detects whether any devcontainer-relevant path changed in the PR or push; outputs a boolean that gates the next job
4. `contributor-devcontainer-gate`: validates the committed contributor container and runs the full headless `./check.sh` maintainer gate through `./scripts/validate-devcontainer.sh` plus `./scripts/run-devcontainer-check.sh`; fires only when `devcontainer-changes` reports a relevant path was touched — specifically `.github/workflows/ci.yml`, `.devcontainer/`, `tooling/rust-tooling.env`, `scripts/bootstrap-rust-tools.sh`, the devcontainer helper scripts, or `check.sh`; skipped otherwise because the rust-gate already proves the code
5. `cross-platform-rust-gate`: runs formatting, Clippy, tests, dependency-policy checks, and the maintained semver gate on macOS arm64 and Windows x64; excludes the managed Cargo artifact roots from Windows Defender before Cargo operations begin
6. `release-target-smoke`: builds, extracts, and smoke-tests the packaged CLI for every supported release target
7. `check`: aggregate required-status job; uses `if: always()` with explicit failure detection so a skipped `contributor-devcontainer-gate` — the correct outcome when no devcontainer-relevant files changed — does not block merge; only a failed or cancelled job prevents success

`CI` also exposes `workflow_dispatch` so maintainers can rerun the exact aggregate `Check` against a branch when GitHub fails to attach the `pull_request` workflow on the initial PR open.
The Rust-cache steps in both `ci.yml` and `release.yml` intentionally exclude `~/.cargo/bin`, so pinned Rust toolchains and Cargo QA tools stay owned by the repo bootstrap contract rather than by restored runner cache state.

`/.github/workflows/release.yml` uses one helper job, two build jobs, and one publication job. The effective publication flow is:

1. compute the standalone target matrix
2. build source archives and generate GitHub build provenance attestations for them
3. build standalone packages, smoke them, and generate one attestation per package
4. build one checksum manifest for the full asset inventory and attest it too
5. publish and verify the GitHub release idempotently from the maintained default branch

## Supported Standalone Release Targets

The maintained target inventory comes from [`scripts/release-targets.sh`](../scripts/release-targets.sh).

## Contributor Container

FFHN now maintains one contributor container under [../.devcontainer/](../.devcontainer/).

That surface is for repository work only:

1. edit, lint, test, fuzz, and run `./check.sh` inside one pinned Linux environment
2. keep the public release contract on native standalone binaries rather than on a runtime image
3. validate the contributor container through [`validate-devcontainer.sh`](../scripts/validate-devcontainer.sh)
4. run the full maintainer gate headlessly through [`run-devcontainer-check.sh`](../scripts/run-devcontainer-check.sh)

That contributor workflow keeps Cargo caches and the managed artifact roots under the mounted user
cache volume rather than writing heavy build output through the repository bind mount.

The contributor container is not a published artifact and not part of the release-asset inventory.

| Target triple | Notes |
| --- | --- |
| `aarch64-apple-darwin` | macOS arm64, deployment target pinned to `12.0` |
| `x86_64-apple-darwin` | macOS x64, deployment target pinned to `12.0` |
| `x86_64-unknown-linux-musl` | static Linux x64 build |
| `x86_64-pc-windows-msvc` | Windows x64 build |

## Release Artifacts

The release workflow publishes:

1. one source zip: `ffhn-source-<version>.zip`
2. one source tarball: `ffhn-source-<version>.tar.gz`
3. one standalone package per supported target:
   `ffhn-<version>-<target-triple>.tar.gz` on macOS and Linux, `ffhn-<version>-<target-triple>.zip` on Windows
4. one checksum manifest: `ffhn-<version>-checksums.txt`

macOS and Linux ship as `.tar.gz` packages so the executable bit survives extraction. Windows ships as `.zip`.

Each standalone package contains:

1. the platform `ffhn` binary
2. `README.md`
3. `LICENSE`
4. `NOTICE`
5. `PATENTS.md`
6. `changelog.md`

Local source archives land under `dist/` through [`scripts/build-release-source-archives.sh`](../scripts/build-release-source-archives.sh). Local standalone packages land under `dist/` through [`scripts/build-release-artifact.sh`](../scripts/build-release-artifact.sh). The checksum manifest is assembled by [`scripts/build-release-checksums.sh`](../scripts/build-release-checksums.sh) after the full maintained asset inventory is present.
GitHub Actions also emits build provenance attestations for the source archives, standalone packages, and checksum manifest. Those attestations are workflow metadata, not additional FFHN-owned release assets.

## Release Scripts

The maintained release scripts are:

1. [`validate-devcontainer.sh`](../scripts/validate-devcontainer.sh): build and smoke the committed contributor container, then prove the actual Dev Container client path against the committed `devcontainer.json`
2. [`run-devcontainer-check.sh`](../scripts/run-devcontainer-check.sh): build or reuse the committed contributor container image and run the full `./check.sh` gate inside it with the canonical persistent cache volumes
3. [`build-release-source-archives.sh`](../scripts/build-release-source-archives.sh): build the maintained source zip and source tarball into `dist/`
4. [`build-release-artifact.sh`](../scripts/build-release-artifact.sh): build one packaged `ffhn` standalone artifact into `dist/`
5. [`build-release-checksums.sh`](../scripts/build-release-checksums.sh): assemble the single checksum manifest for the maintained asset inventory once `dist/` is complete
6. [`smoke-release-artifact.sh`](../scripts/smoke-release-artifact.sh): extract a packaged artifact and execute the packaged binary
7. [`publish-github-release.sh`](../scripts/publish-github-release.sh): create or reuse a draft release, upload missing assets, and publish only after the full asset set exists
8. [`qa-gate.sh`](../scripts/qa-gate.sh): wrapper around `./check.sh`
9. [`release-targets.sh`](../scripts/release-targets.sh): target inventory and asset-name helpers used by CI and scripts
10. [`verify-github-release.sh`](../scripts/verify-github-release.sh): assert the published release is non-draft, non-prerelease, and asset-complete
11. [`workspace-package-field.sh`](../scripts/workspace-package-field.sh): robustly extract one string field from `[workspace.package]` in `Cargo.toml`

Each maintained release helper supports `--help` for local inspection instead of requiring source reading first.

The three local asset builders refuse tracked checkout drift. Use a clean checkout or a clean
release worktree before building source archives, standalone packages, or the checksum manifest.

## Release Preconditions

The release scripts and workflow enforce:

1. the release tag must be `v<workspace-version>`
2. the published tag version must match the selected release tag version exactly
3. GitHub publication requires `GH_TOKEN`
4. workflow reruns pass `RELEASE_VERSION` from the selected tag so default-branch publication and verification scripts stay pinned to the intended asset inventory even if `main` has moved on
5. the public release path is draft-first: reruns may repair an in-progress draft release, but the scripts refuse to backfill missing assets into an already-published incomplete release

FFHN uses the dedicated `dist` profile for shipped binaries. Normal `cargo build --release` is not the maintained publication path.
