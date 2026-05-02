---
afad: "4.0"
version: "5.0.0"
domain: SETUP
updated: "2026-05-03"
route:
  keywords: [developer setup, rustup, Rust 1.95.0, nightly llvm-cov, cargo-fuzz, shellcheck, gh cli, clang override]
  questions: ["how do I set up a fresh machine for ffhn?", "which tools are required for ffhn development?", "what is optional versus required for ffhn fuzzing?", "what is required for ffhn release work?"]
---

# Developer Setup

This page bootstraps a fresh machine into the maintained FFHN contributor state.

## Required Versus Optional Tools

Required for ordinary local development:

1. Rust `1.95.0` through `rustup`
2. a working system C toolchain

Required for the maintained local gate (`./check.sh`):

1. nightly Rust with `llvm-tools-preview`
2. Cargo QA tools used by `cargo xtask`
3. `shellcheck`

Required for public release work:

1. GitHub CLI `gh` with authenticated `repo` and `workflow` access

Optional for manual sanitizer-backed fuzz runs:

1. `cargo-fuzz`

## Install Rust With `rustup`

On macOS, install Apple Command Line Tools first if needed:

```bash
xcode-select --install
```

Then install Rust and the maintained FFHN toolchains:

```bash
curl --proto '=https' --tlsv1.2 https://sh.rustup.rs -sSf | sh -s -- -y --profile minimal --default-toolchain 1.95.0
source "$HOME/.cargo/env"
rustup toolchain install nightly --profile minimal --component llvm-tools-preview
rustup component add clippy rustfmt llvm-tools-preview --toolchain 1.95.0
```

Why this shape:

1. Rust `1.95.0` is the workspace default from [`rust-toolchain.toml`](../rust-toolchain.toml)
2. nightly exists for branch coverage and optional `cargo-fuzz`
3. FFHN needs `rustup` control over both toolchains and their components

Nightly is not required for ordinary `cargo build`, `cargo test`, or `cargo run`. It is required for the maintained coverage gate and optional manual fuzzing.

## Install Required Cargo QA Tools

On the maintained macOS path, install the required Cargo subcommands with the system compiler forced explicitly:

```bash
source "$HOME/.cargo/env"
CC=clang CXX=clang++ cargo install cargo-nextest cargo-audit cargo-deny cargo-semver-checks cargo-outdated cargo-llvm-cov --locked
```

Why this shape:

1. these tools live naturally beside `cargo` under the Rust-managed toolchain
2. `--locked` uses each tool's checked-in lockfile
3. `CC=clang CXX=clang++` protects fresh macOS machines from stale Homebrew LLVM shell overrides

## Install Optional `cargo-fuzz`

Only install this if you plan to run the manual fuzz smokes from [../fuzz/README.md](../fuzz/README.md):

```bash
source "$HOME/.cargo/env"
CC=clang CXX=clang++ cargo install cargo-fuzz --locked
```

`cargo-fuzz` is not required for `./check.sh`.

## Install ShellCheck

On macOS:

```bash
brew install shellcheck
```

On Linux, use your system package manager's `shellcheck` package.

## Install GitHub CLI

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

The workspace already forces two repository-local Cargo environment values through [../.cargo/config.toml](../.cargo/config.toml):

1. `CC=clang`
2. `CARGO_INCREMENTAL=0`

That means normal Cargo commands inside this repository already inherit the maintained compiler override and disable incremental compilation.

## Fix Stale Compiler Overrides

If Cargo fails with a missing compiler path such as:

```text
failed to find tool "/opt/homebrew/opt/llvm/bin/clang"
```

your shell is exporting a stale `CC` or `CXX` value that points at a removed Homebrew LLVM installation.

Fix the shell config, or override it when installing tools:

```bash
CC=clang CXX=clang++ cargo install cargo-nextest --locked
```

## Verify The Setup

Verify the maintained local gate toolchain:

```bash
source "$HOME/.cargo/env"
rustc --version
cargo --version
cargo nextest --version
cargo audit --version
cargo deny --version
cargo semver-checks --version
cargo outdated --version
cargo llvm-cov --version
shellcheck --version
./check.sh
```

If you plan to do release work from this machine, verify GitHub CLI separately:

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

The largest FFHN disk consumers are build outputs under `target/` plus optional `fuzz/target/` data from manual fuzzing.

For normal maintainer work:

1. `target/llvm-cov-target` is cleaned again after coverage
2. `target/semver-checks` is cleaned before and after semver-checks

If you need to reclaim space:

```bash
source "$HOME/.cargo/env"
cargo llvm-cov clean --workspace
cargo clean
rm -rf fuzz/target fuzz/artifacts
```

Only remove `fuzz/corpus/*` entries if you explicitly mean to discard locally generated fuzz cases.
