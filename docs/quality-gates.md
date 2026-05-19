---
afad: "4.0"
domain: QUALITY
updated: "2026-05-19"
route:
  keywords: [quality gates, check.sh, cargo xtask, devcontainer, coverage, miri, nextest, cargo deny, semver baseline, fuzz compile smoke, package smoke]
  questions: ["what does ffhn check.sh run?", "how does the ffhn contributor container get validated?", "how does the ffhn strict-provenance miri proof run?", "how does the ffhn coverage gate work?", "what fuzzing checks are automatic versus manual?"]
---

# Quality Gates

FFHN uses `cargo xtask` as the maintained gate surface. `./check.sh` is the canonical entrypoint and simply dispatches to `cargo xtask check`.

## Toolchains

FFHN keeps its exact Rust toolchain pins in two canonical files:

1. [../rust-toolchain.toml](../rust-toolchain.toml) owns the default stable workspace toolchain
2. [../tooling/rust-tooling.env](../tooling/rust-tooling.env) owns the full maintainer toolchain and QA-tool version set

The pinned QA nightly toolchain exists for three reasons:

1. `cargo +<qa-nightly-toolchain> miri test` is required for the maintained FFHN-to-HTMLCut strict-provenance proof across both the CSS-selector validation seam and the delimiter-pair execution seam
2. `cargo +<qa-nightly-toolchain> llvm-cov --branch` is required for the maintained branch-coverage gate
3. manual `cargo-fuzz` runs require nightly sanitizer flags

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

Miri-only:

```bash
cargo xtask miri
```

That proof exercises one CSS-selector target and one delimiter-pair target
through HTMLCut interop so FFHN verifies both maintained HTML-backed target
paths under strict provenance.
As of the maintained `htmlcut-core v10.2.0` line, that proof now runs against
HTMLCut's shipped downstream-safe selector/parser stack instead of FFHN-local
crate overrides.

Artifact inventory:

```bash
cargo xtask hygiene report
```

Artifact cleanup:

```bash
cargo xtask hygiene clean --mode safe
```

Contributor-container validation:

```bash
./scripts/validate-devcontainer.sh
```

Contributor-container full gate:

```bash
./scripts/run-devcontainer-check.sh
```

Semver-only:

```bash
cargo xtask semver-check
```

Semver baseline refresh:

```bash
cargo xtask refresh-semver-baseline --git-ref vX.Y.Z
```

## What `cargo xtask check` Actually Enforces

`cargo xtask check` runs these steps, in order:

1. `cargo xtask hygiene clean --mode safe`
2. prepare and verify the managed artifact roots
3. `bash -n` over `check.sh` and every `scripts/*.sh` file
4. `shellcheck` over the same shell scripts
5. `cargo fmt --check`
6. `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
7. `cargo xtask miri`
8. `cargo xtask audit`
9. `cargo xtask audit --file fuzz/Cargo.lock`
10. `cargo deny check advisories bans licenses sources`
11. `cargo semver-checks` for `ffhn-core` against `semver-baseline/ffhn-core` with isolated managed `target` and `build` scratch roots
12. `cargo check --manifest-path fuzz/Cargo.toml --bins --locked`
13. `cargo clippy --manifest-path fuzz/Cargo.toml --bins --locked -- -D warnings`
14. `cargo +<qa-nightly-toolchain> fuzz check --fuzz-dir fuzz`
15. `cargo nextest run --no-fail-fast --workspace --all-targets --all-features --locked`
16. `cargo test --workspace --doc --all-features --locked`
17. `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked`
18. `cargo build --profile dist -p ffhn-cli --bin ffhn --locked`
19. the dist-profile `ffhn` binary at the active Cargo target root (FFHN configures `../.ffhn-artifacts/target/dist/ffhn` by default, or `${CARGO_TARGET_DIR}/dist/ffhn` when overridden) with `--version`
20. `cargo xtask coverage`
21. `cargo xtask hygiene clean --mode safe`
22. a final hygiene verification pass

Dependency freshness is intentionally separate from the required correctness gate. FFHN keeps the
freshness signal in [../.github/workflows/dependency-freshness.yml](../.github/workflows/dependency-freshness.yml),
which runs the pinned `cargo-outdated` tool without blocking unrelated correctness work.

There is no separate rustdoc-coverage percentage gate. Public-surface documentation is enforced by `#![deny(missing_docs)]` in the Rust crates, so undocumented public items fail normal compilation and test builds.

The contributor devcontainer is a maintained surface too, but it is validated separately from
`./check.sh`. That split is intentional: `./check.sh` is designed to run inside the contributor
container as the normal maintainer path, so rebuilding the contributor image on every local gate
run would turn the preferred workflow into self-recursive overhead instead of useful validation.

For headless Docker sessions, `./scripts/validate-devcontainer.sh` proves both the raw contributor
image contract and the actual Dev Container client path through a helper image derived from the
already-built contributor image, while
`./scripts/run-devcontainer-check.sh` is the maintained way to prove that the committed
contributor image can carry the full `./check.sh` gate from a cold shell environment.
After a successful validation pass, the validator promotes that exact contributor image into the
canonical local tag `ffhn-devcontainer:local`, so `FFHN_DEVCONTAINER_SKIP_BUILD=1
./scripts/run-devcontainer-check.sh` reuses the image that was just proven instead of whichever
older tag happened to exist locally.

The contributor-container workflow keeps Cargo caches plus FFHN's managed artifact roots on named
Docker volumes. That avoids relying on heavy Rust build output written through the repository bind
mount, which is especially important on macOS Docker Desktop setups where the checkout may live
under `/Volumes/...`.

GitHub CI complements that local host-native dist smoke with a release-target smoke matrix. That matrix builds each packaged release artifact, extracts it on the target's native runner, and executes the packaged binary before the aggregate required `Check` job can report success.
GitHub CI also runs a separate cross-platform Rust gate on macOS arm64 and Windows x64 for formatting, Clippy, tests, advisory/license policy, fuzz compile smoke, and the maintained semver lane.
The cross-platform job carries a `150`-minute budget for Windows and a `30`-minute budget for macOS.
The Windows runner excludes FFHN's managed Cargo artifact roots from Windows Defender before any Cargo operations begin, removing the antivirus overhead that would otherwise scan every file write during compilation.
Both runners use `Swatinem/rust-cache` to persist the Cargo registry and managed build artifacts across runs; without that cache, a cold Windows Rust build takes 15-20 minutes. FFHN keeps `~/.cargo/bin` out of that cache on purpose so restored runner state cannot take ownership of the `cargo`, `rustc`, or Cargo-QA-tool entrypoints. CI, the contributor devcontainer, and local bootstrap all converge toolchains and Cargo QA tools through the same [../scripts/bootstrap-rust-tools.sh](../scripts/bootstrap-rust-tools.sh) entrypoint, and that bootstrap now proves the stable Cargo surface by executing a real `cargo build --help` subcommand rather than trusting a version banner alone.
GitHub CI also runs a dedicated contributor-devcontainer gate on Linux to keep the committed `.devcontainer/` contract, the validator's raw-image plus Dev Container client proof, and the full headless `./check.sh` path through `./scripts/run-devcontainer-check.sh` from drifting away from the documented preferred workflow.

**Path-based devcontainer gate theory.** The devcontainer gate validates the contributor *environment*, not application code. Application code changes are already proven by `rust-gate`. Running the full devcontainer gate on every PR regardless of what changed wastes 40-45 minutes per run proving the same environment twice. The gate therefore fires only when the environment itself changes — specifically when any of these paths are touched:

- `.github/workflows/ci.yml` — the workflow that defines the detection logic and contributor gate
- `.devcontainer/` — the Dockerfile and devcontainer.json
- `tooling/rust-tooling.env`
- `scripts/bootstrap-rust-tools.sh`
- `scripts/validate-devcontainer.sh`
- `scripts/run-devcontainer-check.sh`
- `scripts/devcontainer-prepare-user-home.sh`
- `scripts/devcontainer-cli-helper.Dockerfile`
- `scripts/common.sh`
- `check.sh` — the script the gate runs inside the container

A `devcontainer-changes` detection job computes a git diff of the PR's changed files against those paths before the gate is evaluated. When no relevant files changed, `contributor-devcontainer-gate` is skipped. The aggregate `Check` required-status job uses `if: always()` and explicit failure detection so that a skipped devcontainer gate does not block merge; only a *failed* or *cancelled* gate prevents `Check` from succeeding. A skipped result is a correct, intended outcome, not a gap.

Those workflows install the same pinned stable and QA nightly toolchains declared in [../tooling/rust-tooling.env](../tooling/rust-tooling.env) rather than following moving channels, and they wrap `rustup` setup in retries so transient runner bootstrap failures do not masquerade as product regressions.

The semver lane treats the current workspace version as an unreleased major line until a matching local Git tag `vX.Y.Z` exists. That keeps release-branch checks correct after the changelog is dated but before the public tag is pushed.
FFHN's managed artifact layout keeps normal Cargo output outside the repository tree by default, and the semver lane narrows its own scratch further into isolated managed `semver-checks` directories under that layout. The final dist smoke and the coverage JSON path follow the same maintained artifact policy, while still honoring explicit `CARGO_TARGET_DIR` and `CARGO_BUILD_BUILD_DIR` overrides when a caller intentionally relocates them.

## Coverage Policy

The coverage gate:

1. starts from a clean `cargo llvm-cov` workspace
2. runs `cargo +<qa-nightly-toolchain> llvm-cov --branch --workspace --all-targets --all-features --locked`
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
4. the repository-root `AGENTS.md` must remain the only maintained agent entrypoint, and shadow agent-entrypoint files under `.codex/` must not be reintroduced
5. the README and `docs/cli.md` command catalogs must match the core-owned CLI contract metadata
6. public Markdown must not mention unknown FFHN operation ids or unknown `ffhn.*` document ids
7. user-facing Rust string literals in the maintained source tree must not mention unknown FFHN operation ids or unknown `ffhn.*` document ids
8. the README, platform-support docs, and release protocol must stay aligned with the canonical release-target and release-asset inventory emitted by `scripts/release-targets.sh`
9. every documented `cargo xtask refresh-semver-baseline` invocation in public Markdown must include the required `--git-ref` argument

The `ffhn-cli` test suite complements that repository lint by asserting that live help output, help/version write-failure handling, and document write-failure text render from the same core-owned operation, limit, and document contract instead of carrying separate hard-coded labels.

## Fuzzing Policy

The automatic gate security-audits the standalone fuzz lockfile, lint-checks the standalone fuzz package, and compile-smokes the maintained harnesses. FFHN routes RustSec auditing through `cargo xtask audit` so transient advisory-database fetch failures are retried by one maintained entrypoint instead of being reimplemented piecemeal across local and CI lanes.

Automatic:

```bash
cargo xtask audit --file fuzz/Cargo.lock
cargo check --manifest-path fuzz/Cargo.toml --bins --locked
cargo clippy --manifest-path fuzz/Cargo.toml --bins --locked -- -D warnings
cargo +<coverage-toolchain> fuzz check --fuzz-dir fuzz
```

Manual sanitizer-backed seed smokes live in [../fuzz/README.md](../fuzz/README.md). They require `cargo-fuzz` and nightly, but they are not part of `./check.sh`.

If you change `.github/workflows/ci.yml`, `.devcontainer/`, `tooling/rust-tooling.env`,
`scripts/bootstrap-rust-tools.sh`, `check.sh`, or any of the devcontainer helper scripts
(`scripts/validate-devcontainer.sh`, `scripts/run-devcontainer-check.sh`,
`scripts/devcontainer-prepare-user-home.sh`, `scripts/devcontainer-cli-helper.Dockerfile`,
`scripts/common.sh`), run `./scripts/validate-devcontainer.sh` in the same change. Those paths
are also the exact set that triggers the CI devcontainer gate, so they stay in sync.

## Scratch Directories

`cargo xtask check` treats the heaviest gate scratch trees as disposable:

1. managed coverage roots are recreated for coverage and then cleaned again
2. the managed `semver-checks` target/build scratch roots are removed before and after semver-checks
3. any stale `semver-baseline/ffhn-core/target` tree left by older semver runs is removed before and after the semver lane
4. any legacy repo-local `target/`, `fuzz/target/`, or Cargo-target-like scratch tree under `tmp/` is reported as hygiene debt and can be reclaimed with `cargo xtask hygiene clean`

Persistent disk growth should therefore come mostly from the managed Cargo caches under
`../.ffhn-artifacts/`, not from stale gate-only clutter inside the repository tree.
