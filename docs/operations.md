---
afad: "3.5"
version: "3.0.0"
domain: OPERATIONS
updated: "2026-04-23"
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
cargo xtask coverage
cargo xtask refresh-semver-baseline --git-ref vX.Y.Z
```

## CI Workflows

`/.github/workflows/ci.yml` uses one helper job, two work lanes, and one aggregate required-check job:

1. `release-target-matrix`: computes the standalone release-target matrix
2. `rust-gate`: installs toolchains and QA tools, then runs `./check.sh`
3. `release-target-smoke`: builds, extracts, and smoke-tests the packaged CLI for every supported release target
4. `check`: aggregate success job used for branch protection

`/.github/workflows/release.yml` uses one helper job, two build jobs, and one publication job. The effective publication flow is:

1. compute the standalone target matrix
2. build source archives
3. build standalone packages
4. build one checksum manifest for the full asset inventory
5. publish and verify the GitHub release idempotently from the maintained default branch

## Supported Standalone Release Targets

The maintained target inventory comes from [`scripts/release-targets.sh`](../scripts/release-targets.sh).

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

Local package builds land under `dist/` through [`scripts/build-release-artifact.sh`](../scripts/build-release-artifact.sh). The checksum manifest is assembled by [`scripts/build-release-checksums.sh`](../scripts/build-release-checksums.sh).

## Release Scripts

The maintained release scripts are:

1. [`build-release-artifact.sh`](../scripts/build-release-artifact.sh): build one packaged `ffhn` release artifact into `dist/`
2. [`build-release-checksums.sh`](../scripts/build-release-checksums.sh): assemble the single checksum manifest for the maintained asset inventory
3. [`smoke-release-artifact.sh`](../scripts/smoke-release-artifact.sh): extract a packaged artifact and execute the packaged binary
4. [`publish-github-release.sh`](../scripts/publish-github-release.sh): create or reuse a draft release, upload missing assets, and publish only after the full asset set exists
5. [`qa-gate.sh`](../scripts/qa-gate.sh): wrapper around `./check.sh`
6. [`release-targets.sh`](../scripts/release-targets.sh): target inventory and asset-name helpers used by CI and scripts
7. [`verify-github-release.sh`](../scripts/verify-github-release.sh): assert the published release is non-draft, non-prerelease, and asset-complete
8. [`workspace-version.sh`](../scripts/workspace-version.sh): robustly extract `[workspace.package] version` from `Cargo.toml`

## Release Preconditions

The release scripts and workflow enforce:

1. the release tag must be `v<workspace-version>`
2. the published tag version must match the selected release tag version exactly
3. GitHub publication requires `GH_TOKEN`
4. workflow reruns pass `RELEASE_VERSION` from the selected tag so default-branch publication and verification scripts stay pinned to the intended asset inventory even if `main` has moved on
5. the public release path is draft-first: reruns may repair an in-progress draft release, but the scripts refuse to backfill missing assets into an already-published incomplete release

FFHN uses the dedicated `dist` profile for shipped binaries. Normal `cargo build --release` is not the maintained publication path.
