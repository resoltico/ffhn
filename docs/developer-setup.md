---
afad: "4.0"
domain: SETUP
updated: "2026-05-16"
route:
  keywords: [developer setup, devcontainer, docker desktop, rustup, rust toolchain, nightly, miri, cargo-fuzz, shellcheck, gh cli, compiler override]
  questions: ["how do I set up a fresh machine for ffhn?", "what is the preferred FFHN contributor workflow?", "which tools are required for ffhn development?", "what is optional versus required for ffhn fuzzing?", "what is required for ffhn's maintained miri proof?", "what is required for ffhn release work?"]
---

# Developer Setup

This page bootstraps a fresh machine into the maintained FFHN contributor state.

Preferred contributor path:

1. use the committed devcontainer documented in [developer-devcontainer.md](developer-devcontainer.md)
2. keep host-native Rust setup as a fallback or escape hatch, not the default

## Required Versus Optional Tools

Required for the preferred contributor workflow:

1. Docker Desktop on macOS, or a compatible Docker runtime on Linux
2. either Visual Studio Code with the Dev Containers extension or another way to materialize the committed devcontainer

Required for ordinary host-native local development:

1. `rustup`
2. a working system C toolchain

Required for the maintained local gate (`./check.sh`):

1. the pinned stable and QA nightly toolchains from [../rust-toolchain.toml](../rust-toolchain.toml) and [../tooling/rust-tooling.env](../tooling/rust-tooling.env)
2. the pinned Cargo QA tools used by `cargo xtask`
3. `shellcheck`

Required for public release work:

1. GitHub CLI `gh` with authenticated `repo` and `workflow` access

Optional for manual sanitizer-backed fuzz runs:

1. `cargo-fuzz`

## Preferred Contributor Workflow

Start with [developer-devcontainer.md](developer-devcontainer.md).

The committed contributor container already bakes in the pinned Rust toolchains, Cargo QA tools,
`shellcheck`, `gh`, `clang`, and the Linux musl target used for local Linux package work.

Use host-native setup below only when you intentionally do not want the contributor container.

## Host-Native Rust With `rustup`

On macOS, install Apple Command Line Tools first if needed:

```bash
xcode-select --install
```

Then install `rustup`, enter the repository checkout, and let the repo-owned bootstrap script
install the maintained FFHN toolchains and QA tools:

```bash
curl --proto '=https' --tlsv1.2 https://sh.rustup.rs -sSf | sh -s -- -y --profile minimal
source "$HOME/.cargo/env"
./scripts/bootstrap-rust-tools.sh install-all
```

Why this shape:

1. [`rust-toolchain.toml`](../rust-toolchain.toml) and [../tooling/rust-tooling.env](../tooling/rust-tooling.env) stay the only canonical owners of exact toolchain and QA-tool versions
2. [../scripts/bootstrap-rust-tools.sh](../scripts/bootstrap-rust-tools.sh) installs those pinned versions directly instead of depending on ambient host state
3. the maintained Miri proof, coverage gate, and manual fuzzing need the pinned QA nightly toolchain, while ordinary build/test/run work uses the pinned stable toolchain

Nightly is not required for ordinary `cargo build`, `cargo test`, or `cargo run`. It is required for the maintained Miri proof, the coverage gate, and optional manual fuzzing.

The bootstrap script also installs the pinned `cargo-fuzz` and `cargo-outdated` tools. They are
not part of the required correctness gate, but FFHN uses them for manual fuzzing and the separate
dependency-freshness workflow.

## Install Host-Native ShellCheck

On macOS:

```bash
brew install shellcheck
```

On Linux, use your system package manager's `shellcheck` package.

## Install Host-Native GitHub CLI

On macOS:

```bash
brew install gh
gh auth login
```

On Linux, use your system package manager's `gh` package, then run:

```bash
gh auth login
```

Release work uses `gh`, not the GitHub web UI. The maintained release choreography lives in [release-protocol.md](release-protocol.md).

## Repository-Local Compiler Guards

The workspace still forces one repository-owned Cargo configuration surface through [../.cargo/config.toml](../.cargo/config.toml):

1. `target-dir = "../.ffhn-artifacts/target"`
2. `build-dir = "../.ffhn-artifacts/build"`
3. `CARGO_INCREMENTAL=0`

The maintained shell entrypoints and the Rust `xtask` runner additionally scrub ambient
`CC`, `CXX`, `CLANG_BIN`, `CPPFLAGS`, and `LDFLAGS` so stale host-native LLVM overrides do not
poison the canonical FFHN commands.

## Fix Stale Compiler Overrides

If Cargo fails with a missing compiler path such as:

```text
failed to find tool "/opt/homebrew/opt/llvm/bin/clang"
```

your shell is exporting a stale `CC` or `CXX` value that points at a removed Homebrew LLVM installation.

Fix the shell config, or clear the stale overrides before running ad hoc Cargo commands that bypass
FFHN's maintained entrypoints:

```bash
env -u CC -u CXX -u CLANG_BIN -u CPPFLAGS -u LDFLAGS cargo build --locked
```

## Verify The Preferred Contributor Container

If you are using the committed contributor container, validate both its raw Docker contract and
its real Dev Container client path directly:

```bash
./scripts/validate-devcontainer.sh
```

For the full maintainer gate inside the contributor image:

```bash
FFHN_DEVCONTAINER_SKIP_BUILD=1 ./scripts/run-devcontainer-check.sh
```

The validator seeds the canonical local contributor-image tag, so the skip-build gate reuses the
same image that already passed the raw-image and Dev Container client proofs.

For a live terminal inside the contributor image, follow [developer-devcontainer.md](developer-devcontainer.md).

## Verify Host-Native Setup

Verify the maintained local gate toolchain:

```bash
source "$HOME/.cargo/env"
rustc --version
cargo --version
cargo nextest --version
cargo audit --version
cargo deny --version
cargo semver-checks --version
cargo llvm-cov --version
cargo +"$(sed -n 's/^RUST_QA_NIGHTLY_TOOLCHAIN=//p' tooling/rust-tooling.env)" miri --version
shellcheck --version
./check.sh
```

If you plan to do release work from a host-native machine, verify GitHub CLI separately:

```bash
source "$HOME/.cargo/env"
gh --version
gh auth status
```

If you also installed `cargo-fuzz`, verify that separately:

```bash
cargo fuzz --version
```

## Disk Usage

The largest FFHN disk consumers are the managed sibling Cargo artifact roots under
`../.ffhn-artifacts/` plus any manually generated `fuzz/artifacts/` data.

For normal maintainer work:

1. `cargo xtask check` and `cargo xtask coverage` prepare and clean managed scratch automatically
2. `cargo xtask hygiene clean --mode safe` removes disposable scratch without touching the managed Cargo caches
3. `cargo xtask hygiene clean --mode rebuildable` removes the managed Cargo caches too

If you need to reclaim space:

```bash
cargo xtask hygiene report
cargo xtask hygiene clean --mode rebuildable
rm -rf fuzz/artifacts
```

Only remove `fuzz/corpus/*` entries if you explicitly mean to discard locally generated fuzz cases.
