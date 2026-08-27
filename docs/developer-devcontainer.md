---
afad: "4.0"
domain: DEVCONTAINER
updated: "2026-08-25"
route:
  keywords: [devcontainer, docker desktop, contributor container, vscode, dev containers, rust toolchain, cargo xtask, ffhn contributor workflow]
  questions: ["what is the preferred FFHN contributor workflow?", "how do I use the FFHN devcontainer?", "does FFHN have a contributor container?", "how do I validate the FFHN devcontainer?", "how do I run the full FFHN maintainer gate inside the devcontainer?"]
---

# Contributor Devcontainer Workflow

This page documents FFHN's preferred contributor workflow.

The committed contributor container is a Linux development environment for repository work. It is
not a published FFHN runtime surface and it is not part of the native release-asset contract.

## Canonical Stance

Preferred contributor path:

1. keep the Git checkout on the host filesystem
2. open that checkout through the committed devcontainer
3. run `cargo`, `cargo xtask`, `./check.sh`, and release-helper inspection from the container shell

The current committed owner files are:

1. [../.devcontainer/devcontainer.json](../.devcontainer/devcontainer.json)
2. [../.devcontainer/Dockerfile](../.devcontainer/Dockerfile)
3. [../tooling/rust-tooling.env](../tooling/rust-tooling.env)
4. [../scripts/bootstrap-rust-tools.sh](../scripts/bootstrap-rust-tools.sh)
5. [../scripts/devcontainer-prepare-user-home.sh](../scripts/devcontainer-prepare-user-home.sh)
6. [../scripts/validate-devcontainer.sh](../scripts/validate-devcontainer.sh)
7. [../scripts/run-devcontainer-check.sh](../scripts/run-devcontainer-check.sh)

The contributor container is intentionally separate from FFHN's shipped runtime model. FFHN
publishes native standalone binaries, documented in [platform-support.md](platform-support.md), not
container images.

## Current Base Image

The contributor image is pinned to Ubuntu `24.04`.

Why:

1. it gives FFHN one reproducible Linux maintainer environment
2. it bakes the pinned Rust toolchains plus required QA tools into the same place
3. it avoids host-by-host drift in `clang`, `shellcheck`, Cargo subcommands, and release helpers

## What The Contributor Container Owns

The image bakes in:

1. the pinned stable and coverage Rust toolchains from [../tooling/rust-tooling.env](../tooling/rust-tooling.env)
2. `cargo-nextest`, `cargo-audit`, `cargo-deny`, `cargo-semver-checks`, `cargo-outdated`, `cargo-llvm-cov`, and `cargo-fuzz`
3. `clang`, `shellcheck`, `gh`, and Linux build tooling

The optional cargo-mutants tool is not baked into the default image. Install it only for a mutation campaign with `./scripts/bootstrap-rust-tools.sh install-mutation-tool`; its exact version remains owned by [../tooling/rust-tooling.env](../tooling/rust-tooling.env).
4. the Linux `x86_64-unknown-linux-musl` Rust target for local Linux package work

The devcontainer mounts named volumes for Cargo registry/git caches and the user cache directory.
That user cache volume owns FFHN's managed artifact roots, including
`/home/vscode/.cache/ffhn-artifacts/target` and `/home/vscode/.cache/ffhn-artifacts/build`, so
container rebuilds do not force a full dependency redownload and Docker-backed contributor runs do
not write heavy Rust build output back through the host bind mount.

The maintained semver lane uses isolated scratch directories under those managed roots rather than
writing into the repository tree.

## First Open With VS Code

Prerequisites on the host:

1. Docker Desktop running
2. Visual Studio Code
3. the Dev Containers extension

Then:

1. open the repository in VS Code
2. run `Dev Containers: Reopen in Container`
3. wait for the image build to finish
   The first build can take several minutes because the contributor image bakes the pinned Rust
   toolchains and the maintained Cargo QA tools into the container instead of assuming host setup.
4. open a terminal inside the container and run:

```bash
./scripts/devcontainer-prepare-user-home.sh
rustc --version
cargo nextest --version
./check.sh --help
```

Expected shape:

1. `rustc` reports the stable toolchain pinned in [../tooling/rust-tooling.env](../tooling/rust-tooling.env)
2. Cargo QA tools resolve without host setup
3. `./check.sh --help` works from the mounted workspace

## Tooling-Agnostic Docker Workflow

If you are not using VS Code, you can still use the same contributor image with Docker directly.

Build and validate the committed contributor image:

```bash
./scripts/validate-devcontainer.sh
```

The maintained validator avoids Bash-4-only builtins, so stock macOS `/bin/bash` is enough for
this entrypoint as long as Docker and the Dev Container client are installed.

That script:

1. builds the contributor image from [../.devcontainer/Dockerfile](../.devcontainer/Dockerfile)
2. poisons the mounted cache and build-output volumes with root-owned files
3. verifies that [../scripts/devcontainer-prepare-user-home.sh](../scripts/devcontainer-prepare-user-home.sh) repairs writability for the contributor user
4. checks the pinned toolchains and Cargo QA versions from [../tooling/rust-tooling.env](../tooling/rust-tooling.env), plus the nightly `cargo +... miri` entrypoint, `shellcheck`, `gh`, `clang`, and `./check.sh --help`, on the raw Docker image
5. builds a small Dev Container CLI helper from the already-built contributor image, layers in a pinned Node 24 LTS runtime plus a pinned Docker Buildx CLI plugin for the pinned Dev Containers CLI, proves that `docker buildx` works inside that helper, brings up the committed devcontainer through that client path, and reruns the same runtime probe inside the materialized environment
6. promotes the validated contributor image to the canonical local tag `ffhn-devcontainer:local` so later cached-image checks can reuse the exact proven image instead of an ambient older tag

The contributor image copies exactly the pinned tooling manifest plus the standalone
[../scripts/bootstrap-rust-tools.sh](../scripts/bootstrap-rust-tools.sh) installer before Rust
setup begins. That bootstrap entrypoint is intentionally self-contained so the image build does not
depend on extra repo-local helper files that were never copied into Docker.

Run the full maintainer gate through the same contributor image and persistent cache volumes:

```bash
./scripts/run-devcontainer-check.sh
```

That script:

1. rebuilds the committed contributor image under the stable local tag `ffhn-devcontainer:local`
2. reuses the canonical `ffhn-cargo-registry`, `ffhn-cargo-git`, and `ffhn-user-cache` volumes
3. points Cargo at `/home/vscode/.cache/ffhn-artifacts/{target,build}` inside that mounted cache volume
4. repairs cache and managed-artifact ownership through [../scripts/devcontainer-prepare-user-home.sh](../scripts/devcontainer-prepare-user-home.sh)
5. marks the mounted repository as a Git safe directory for the raw Docker session
6. runs the maintained `./check.sh` gate inside the contributor container

GitHub CI uses the validator first, then reuses the warmed contributor image for the maintained headless full-gate pass so the second step proves `./check.sh` rather than paying for a redundant rebuild. Local release and maintainer flows can do the same by running `FFHN_DEVCONTAINER_SKIP_BUILD=1 ./scripts/run-devcontainer-check.sh` immediately after a successful validation pass.

For ad hoc terminal use without VS Code beyond the maintained full-gate path, run the image against
the mounted repository:

```bash
docker build -t ffhn-devcontainer -f .devcontainer/Dockerfile .
docker run --rm -it \
  -v "$PWD:/workspaces/ffhn" \
  -v ffhn-cargo-registry:/home/vscode/.cargo/registry \
  -v ffhn-cargo-git:/home/vscode/.cargo/git \
  -v ffhn-user-cache:/home/vscode/.cache \
  -e CARGO_HOME=/home/vscode/.cargo \
  -e CARGO_TARGET_DIR=/home/vscode/.cache/ffhn-artifacts/target \
  -e CARGO_BUILD_BUILD_DIR=/home/vscode/.cache/ffhn-artifacts/build \
  -w /workspaces/ffhn \
  ffhn-devcontainer \
  bash
```

Then inside the container:

```bash
./scripts/devcontainer-prepare-user-home.sh
git config --global --add safe.directory /workspaces/ffhn
./check.sh
```

The `safe.directory` line matters for raw `docker run` sessions because that path does not get the
automatic UID synchronization that the Dev Container workflow applies through
`updateRemoteUserUID`.

## Ownership And Cache Repair

The contributor container runs as `vscode`, not `root`.

The post-start hook:

```bash
./scripts/devcontainer-prepare-user-home.sh
```

exists for one specific reason: named Docker volumes can be left behind with root-owned cache or
build-output entries after ad hoc container sessions. When that happens, `cargo` stops being able
to write to its registry, git, cache, or managed artifact roots under
`/home/vscode/.cache/ffhn-artifacts/`. The repair hook recreates missing directories and repairs
ownership only when the mounted path is not writable.

## CI Gate Behavior

The `contributor-devcontainer-gate` CI job fires only when devcontainer-relevant files change. The
path set that triggers it:

- `.github/workflows/ci.yml` — the workflow that defines the detection logic and contributor gate
- `.devcontainer/` — the Dockerfile and `devcontainer.json`
- `tooling/rust-tooling.env`
- `scripts/bootstrap-rust-tools.sh`
- `scripts/validate-devcontainer.sh`
- `scripts/run-devcontainer-check.sh`
- `scripts/devcontainer-prepare-user-home.sh`
- `scripts/devcontainer-cli-helper.Dockerfile`
- `scripts/common.sh`
- `check.sh` — the script the gate runs inside the container

PRs that touch only application code, documentation, or tests do not trigger the devcontainer gate.
The `rust-gate` job already proves the code builds and tests pass; the devcontainer gate proves the
contributor environment. Rebuilding and re-running the entire gate for a change that cannot affect
the environment is wasted time.

When the gate is skipped, the aggregate `Check` required-status job still succeeds — a skipped
devcontainer gate is a correct, intended outcome, not a coverage gap. Only a *failed* or
*cancelled* gate prevents merge.

When the gate fires, CI runs the full two-step proof: `validate-devcontainer.sh` for the raw image
contract and the real Dev Container client path, then `run-devcontainer-check.sh` for the complete
`./check.sh` pass inside the warmed contributor container.

## Boundary With Releases

The devcontainer is a contributor environment.

It does not change these truths:

1. FFHN release artifacts remain native packaged binaries
2. macOS and Windows release smoke remain native-runner concerns in CI
3. GitHub release publication remains documented in [release-protocol.md](release-protocol.md)

Use the contributor container to edit, test, lint, fuzz, and run the maintainer gate. Do not infer
from its existence that FFHN publishes or supports a container runtime surface.
