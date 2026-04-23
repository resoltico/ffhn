---
afad: "3.5"
version: "3.0.0"
domain: PLATFORM
updated: "2026-04-23"
route:
  keywords: [platform support, release targets, standalone packages, deployment floors, target matrix]
  questions: ["which standalone targets does FFHN release?", "what platforms are maintained for FFHN?", "where is the FFHN release target policy defined?"]
---

# Platform Support

This document defines FFHN's maintained build and release target policy.

## Local Development

Local maintainer builds are host-native.

On the primary maintainer host today, that means:

- `aarch64-apple-darwin`

Local development is not expected to produce every public release artifact. Cross-platform public artifact production belongs to GitHub release automation.

## Public Standalone Release Targets

FFHN publishes versioned standalone release packages for:

- `aarch64-apple-darwin`
- `x86_64-apple-darwin`
- `x86_64-unknown-linux-musl`
- `x86_64-pc-windows-msvc`

The maintained asset names are:

- `ffhn-source-X.Y.Z.zip`
- `ffhn-source-X.Y.Z.tar.gz`
- `ffhn-X.Y.Z-aarch64-apple-darwin.tar.gz`
- `ffhn-X.Y.Z-x86_64-apple-darwin.tar.gz`
- `ffhn-X.Y.Z-x86_64-unknown-linux-musl.tar.gz`
- `ffhn-X.Y.Z-x86_64-pc-windows-msvc.zip`
- `ffhn-X.Y.Z-checksums.txt`

macOS and Linux targets ship as `.tar.gz` packages so the executable bit survives extraction. Windows ships as a `.zip` package.

Each standalone package contains:

- the platform binary
- `README.md`
- `LICENSE`

## Deployment Floors

Apple targets are pinned to:

- macOS 12.0 Monterey

That floor applies to:

- `aarch64-apple-darwin`
- `x86_64-apple-darwin`

Windows standalone artifacts target:

- Windows x64 through the `x86_64-pc-windows-msvc` toolchain

Linux standalone artifacts target:

- Linux x64 through `x86_64-unknown-linux-musl`

## Why These Targets

- `aarch64-apple-darwin` is the primary maintainer and operator path.
- `x86_64-apple-darwin` keeps Intel macOS users first-class instead of silently dropping them.
- `x86_64-unknown-linux-musl` produces a more portable standalone Linux binary than a glibc-bound release.
- `x86_64-pc-windows-msvc` is the mainstream Windows release target and avoids 32-bit or GNU-toolchain drift.

## Workflow Boundary

GitHub release builds currently run on:

- `macos-15` for `aarch64-apple-darwin`
- `macos-15-intel` for `x86_64-apple-darwin`
- `ubuntu-24.04` for `x86_64-unknown-linux-musl`
- `windows-2022` for `x86_64-pc-windows-msvc`

GitHub CI also runs release-target smoke on that same target matrix before the aggregate required check reports success.

GitHub also renders auto-generated `Source code (zip)` and `Source code (tar.gz)` links on release pages. Those links are GitHub-provided convenience downloads and are not part of FFHN's maintained asset inventory.

## Source Of Truth

The target policy is implemented in:

- `scripts/release-targets.sh`
- `.github/workflows/ci.yml`
- `.github/workflows/release.yml`
- `scripts/build-release-artifact.sh`
- `scripts/build-release-checksums.sh`
- `scripts/smoke-release-artifact.sh`
- `scripts/publish-github-release.sh`
- `scripts/verify-github-release.sh`

Do not change the public target matrix in only one of those places. The release system must stay derived from the same target policy.
