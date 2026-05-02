---
afad: "4.0"
version: "5.0.0"
domain: QUALITY
updated: "2026-05-03"
route:
  keywords: [quality gates, check.sh, cargo xtask, coverage, nextest, cargo deny, semver baseline, fuzz compile smoke, package smoke]
  questions: ["what does ffhn check.sh run?", "how does the ffhn coverage gate work?", "what fuzzing checks are automatic versus manual?"]
---

# Quality Gates

FFHN uses `cargo xtask` as the maintained gate surface. `./check.sh` is the canonical entrypoint and simply dispatches to `cargo xtask check`.

## Toolchains

FFHN keeps Rust `1.95.0` pinned as the default workspace toolchain.

Nightly is installed alongside Rust `1.95.0` for two reasons:

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
cargo xtask refresh-semver-baseline --git-ref vX.Y.Z
```

## What `cargo xtask check` Actually Enforces

`cargo xtask check` runs these steps, in order:

1. `bash -n` over `check.sh` and every `scripts/*.sh` file
2. `shellcheck` over the same shell scripts
3. `cargo fmt --check`
4. `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
5. `cargo outdated --workspace --root-deps-only --exit-code 1`
6. `cargo outdated --manifest-path fuzz/Cargo.toml --root-deps-only --exit-code 1`
7. `cargo audit -D warnings`
8. `cargo audit --file fuzz/Cargo.lock -D warnings`
9. `cargo deny check advisories bans licenses sources`
10. `cargo semver-checks` for `ffhn-core` against `semver-baseline/ffhn-core` with an isolated `CARGO_TARGET_DIR`
11. `cargo check --manifest-path fuzz/Cargo.toml --bins --locked`
12. `cargo nextest run --no-fail-fast --workspace --all-targets --all-features --locked`
13. `cargo test --workspace --doc --all-features --locked`
14. `cargo build --profile dist -p ffhn-cli --bin ffhn --locked`
15. `target/dist/ffhn --version`
16. `cargo xtask coverage`

There is no separate rustdoc-coverage percentage gate. Public-surface documentation is enforced by `#![deny(missing_docs)]` in the Rust crates, so undocumented public items fail normal compilation and test builds.

GitHub CI complements that local host-native dist smoke with a release-target smoke matrix. That matrix builds each packaged release artifact, extracts it on the target's native runner, and executes the packaged binary before the aggregate required `Check` job can report success.
Those workflows install the same pinned Rust `1.95.0` toolchain rather than following the moving `stable` channel.

The semver lane treats the current workspace version as an unreleased major line until a matching local Git tag `vX.Y.Z` exists. That keeps release-branch checks correct after the changelog is dated but before the public tag is pushed.
It also forces semver-checks scratch output into `target/semver-checks`, so the checked-in `semver-baseline/` tree stays disposable input rather than growing its own nested Cargo caches.

## Coverage Policy

The coverage gate:

1. starts from a clean `cargo llvm-cov` workspace
2. runs `cargo +nightly llvm-cov --branch --workspace --all-targets --all-features --locked`
3. scores every maintained non-test Rust source file under `crates/ffhn-core/src`, `crates/ffhn-cli/src`, and `xtask/src`
4. deduplicates duplicate branch spans emitted by Rust lowering before scoring
5. treats LLVM segments as line ranges rather than single-point hits, so multiline statements are scored correctly and module-barrel files with no instrumentable regions do not fail as fake misses
6. requires 100% executable-line coverage and 100% branch coverage for the tracked set
7. cleans the llvm-cov workspace again after scoring

That source-tree scan deliberately excludes `tests.rs` and any nested `tests/` modules, but it no longer relies on a hand-curated allowlist that can silently miss newly added production files.

The `xtask` test suite also enforces maintainer-facing repository contracts that are easy to let drift silently:

1. AFAD-managed Markdown under `docs/`, `examples/`, and `fuzz/` must carry AFAD frontmatter using the canonical AFAD protocol version from `.codex/PROTOCOL_AFAD.md`; the root `README.md`, `CONTRIBUTING.md`, and `changelog.md` remain special docs and are validated without forced AFAD metadata
2. public Markdown local links and maintained repo-file path mentions must still resolve
3. checked-in public target examples must still validate against the current `ffhn.target` contract
4. `.codex/AGENTS.md`, `.claude/CLAUDE.md`, and `.gemini/GEMINI.md` must remain exact parity entrypoints that redirect agents to the repository-root `AGENTS.md`
5. the README and `docs/cli.md` command catalogs must match the core-owned CLI contract metadata
6. public Markdown must not mention unknown FFHN operation ids or unknown `ffhn.*` document ids
7. user-facing Rust string literals in the maintained source tree must not mention unknown FFHN operation ids or unknown `ffhn.*` document ids
8. the README, platform-support docs, and release protocol must stay aligned with the canonical release-target and release-asset inventory emitted by `scripts/release-targets.sh`
9. every documented `cargo xtask refresh-semver-baseline` invocation in public Markdown must include the required `--git-ref` argument

The `ffhn-cli` test suite complements that repository lint by asserting that live help output, help/version write-failure handling, and document write-failure text render from the same core-owned operation, limit, and document contract instead of carrying separate hard-coded labels.

## Fuzzing Policy

The automatic gate freshness-checks the standalone fuzz manifest, security-audits the standalone fuzz lockfile, and compile-smokes the standalone fuzz package.

Automatic:

```bash
cargo outdated --manifest-path fuzz/Cargo.toml --root-deps-only --exit-code 1
cargo audit --file fuzz/Cargo.lock -D warnings
cargo check --manifest-path fuzz/Cargo.toml --bins --locked
```

Manual sanitizer-backed seed smokes live in [../fuzz/README.md](../fuzz/README.md). They require `cargo-fuzz` and nightly, but they are not part of `./check.sh`.

## Scratch Directories

`cargo xtask check` treats the heaviest gate scratch trees as disposable:

1. `target/llvm-cov-target` is recreated for coverage and then cleaned again
2. `target/semver-checks` is removed before and after semver-checks
3. any stale `semver-baseline/ffhn-core/target` tree left by older semver runs is removed before and after the semver lane

Persistent disk growth should therefore come mostly from normal Cargo build caches rather than stale gate-only artifacts.
