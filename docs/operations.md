---
afad: "3.5"
version: "2.0.0"
domain: OPERATIONS
updated: "2026-04-20"
route:
  keywords: [operations, check.sh, release scripts, ci workflow, dist profile, github release, supported targets, release assets]
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
cargo xtask refresh-semver-baseline
```

## CI Workflows

`/.github/workflows/ci.yml` runs two lanes:

1. `rust-gate`: installs toolchains and QA tools, then runs `./check.sh`
2. `release-target-smoke`: builds and smoke-tests the standalone CLI for every supported release target

`/.github/workflows/release.yml` runs four stages:

1. compute the standalone target matrix
2. build source archives
3. build standalone binaries plus `.sha256` files
4. publish and verify the GitHub release idempotently

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

1. one source zip: `ffhn-<version>.zip`
2. one source tarball: `ffhn-<version>.tar.gz`
3. one standalone binary per supported target: `ffhn-<target-triple>[.exe]`
4. one `.sha256` checksum file per standalone binary

Local standalone builds land under `dist/` through [`scripts/build-release-artifact.sh`](../scripts/build-release-artifact.sh).

## Release Scripts

The maintained release scripts are:

1. [`build-release-artifact.sh`](../scripts/build-release-artifact.sh): build one standalone `ffhn` binary plus checksum into `dist/`
2. [`publish-github-release.sh`](../scripts/publish-github-release.sh): create or converge the GitHub release and upload missing assets
3. [`qa-gate.sh`](../scripts/qa-gate.sh): wrapper around `./check.sh`
4. [`release-targets.sh`](../scripts/release-targets.sh): target inventory and asset-name helpers used by CI and scripts
5. [`verify-github-release.sh`](../scripts/verify-github-release.sh): assert the published release is non-draft, non-prerelease, and asset-complete

## Release Preconditions

The release scripts and workflow enforce:

1. the release tag must be `v<workspace-version>`
2. the published tag version must match `Cargo.toml`
3. GitHub publication requires `GH_TOKEN`
4. the release step may safely rerun because publication is convergent rather than append-only

FFHN uses the dedicated `dist` profile for shipped binaries. Normal `cargo build --release` is not the maintained publication path.
