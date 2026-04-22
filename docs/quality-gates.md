---
afad: "3.5"
version: "2.0.1"
domain: QUALITY
updated: "2026-04-22"
route:
  keywords: [quality gates, check.sh, cargo xtask, coverage, nextest, cargo deny, semver baseline, fuzz compile smoke, package smoke]
  questions: ["what does ffhn check.sh run?", "how does the ffhn coverage gate work?", "what fuzzing checks are automatic versus manual?"]
---

# Quality Gates

FFHN uses `cargo xtask` as the maintained gate surface. `./check.sh` is the canonical entrypoint and simply dispatches to `cargo xtask check`.

## Toolchains

FFHN keeps stable Rust as the default workspace toolchain.

Nightly is installed alongside stable for two reasons:

1. `cargo +nightly llvm-cov --branch` is required for the maintained branch-coverage gate
2. manual `cargo-fuzz` runs require nightly sanitizer flags

Neither the CLI nor the packaged public release builds require nightly.

## Maintained Commands

Full maintainer gate:

```bash
./check.sh
```

Equivalent direct form:

```bash
cargo xtask check
```

Compatibility wrapper:

```bash
./scripts/qa-gate.sh
```

Coverage-only:

```bash
cargo xtask coverage
```

Semver baseline refresh:

```bash
cargo xtask refresh-semver-baseline
```

## What `cargo xtask check` Actually Enforces

`cargo xtask check` runs these steps, in order:

1. `bash -n` over `check.sh` and every `scripts/*.sh` file
2. `shellcheck` over the same shell scripts
3. `cargo fmt --check`
4. `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
5. `cargo outdated --workspace --root-deps-only --exit-code 1`
6. `cargo audit -D warnings`
7. `cargo deny check advisories bans licenses sources`
8. `cargo semver-checks` for `ffhn-core` against `semver-baseline/ffhn-core`
9. `cargo check --manifest-path fuzz/Cargo.toml --bins --locked`
10. `cargo nextest run --workspace --all-targets --all-features --locked`
11. `cargo test --workspace --doc --all-features --locked`
12. `cargo build --profile dist -p ffhn-cli --bin ffhn --locked`
13. `target/dist/ffhn --version`
14. `cargo xtask coverage`

There is no separate rustdoc-coverage percentage gate. Public-surface documentation is enforced by `#![deny(missing_docs)]` in the Rust crates, so undocumented public items fail normal compilation and test builds.

GitHub CI complements that local host-native dist smoke with a release-target smoke matrix. That matrix builds each packaged release artifact, extracts it on the target's native runner, and executes the packaged binary before the aggregate required `Check` job can report success.

The semver lane treats the current workspace version as an unreleased major line until a matching local Git tag `vX.Y.Z` exists. That keeps release-branch checks correct after the changelog is dated but before the public tag is pushed.

## Coverage Policy

The coverage gate:

1. starts from a clean `cargo llvm-cov` workspace
2. runs `cargo +nightly llvm-cov --branch --workspace --all-targets --all-features --locked`
3. scores only the curated tracked-file list in `xtask/src/model.rs`
4. deduplicates duplicate branch spans emitted by Rust lowering before scoring
5. requires 100% executable-line coverage and 100% branch coverage for the tracked set
6. cleans the llvm-cov workspace again after scoring

The tracked-file list currently includes the maintained core runtime/model files, the core-owned CLI contract metadata, `crates/ffhn-cli/src/args.rs`, `crates/ffhn-cli/src/execute.rs`, and the `xtask` planning/coverage/model plus repo-contract helpers.

The `xtask` test suite also enforces maintainer-facing repository contracts that are easy to let drift silently:

1. AFAD-managed Markdown frontmatter must use the current workspace version and the canonical AFAD protocol version from `.codex/PROTOCOL_AFAD.md`
2. checked-in public target examples must still validate against the current `ffhn.target` contract
3. `.claude/CLAUDE.md` and `.gemini/GEMINI.md` must remain exact parity entrypoints that redirect agents to `.codex/AGENTS.md`
4. the README and `docs/cli.md` command catalogs must match the core-owned CLI contract metadata
5. public Markdown must not mention unknown FFHN operation ids or unknown `ffhn.*` document ids
6. user-facing Rust string literals in the maintained source tree must not mention unknown FFHN operation ids or unknown `ffhn.*` document ids

The `ffhn-cli` test suite complements that repository lint by asserting that live help output and CLI write-failure text render from the same core-owned operation, limit, and document contract instead of carrying separate hard-coded labels.

## Fuzzing Policy

The automatic gate only compile-smokes the standalone fuzz package.

Automatic:

```bash
cargo check --manifest-path fuzz/Cargo.toml --bins --locked
```

Manual sanitizer-backed seed smokes live in [../fuzz/README.md](../fuzz/README.md). They require `cargo-fuzz` and nightly, but they are not part of `./check.sh`.

## Scratch Directories

`cargo xtask check` treats the heaviest gate scratch trees as disposable:

1. `target/llvm-cov-target` is recreated for coverage and then cleaned again
2. `target/semver-checks` is removed before and after semver-checks

Persistent disk growth should therefore come mostly from normal Cargo build caches rather than stale gate-only artifacts.
