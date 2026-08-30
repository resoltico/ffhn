---
afad: "4.0"
domain: QUALITY
updated: "2026-08-30"
route:
  keywords: [quality gates, check.sh, cargo xtask, cargo-mutants, mutation testing, source structure, source shape, ownership policy, devcontainer, coverage, miri, nextest, cargo deny, semver baseline, fuzz compile smoke, package smoke, dependency freshness, cargo outdated]
  questions: ["what does ffhn check.sh run?", "how do I run FFHN mutation testing?", "why is cargo-mutants separate from the required gate?", "how does FFHN prevent god files and forbidden Rust module dependencies?", "how does the ffhn contributor container get validated?", "how does the ffhn strict-provenance miri proof run?", "how does the ffhn coverage gate work?", "what fuzzing checks are automatic versus manual?"]
---

# Quality Gates

FFHN uses `cargo xtask` as the maintained gate surface. `./check.sh` is the canonical entrypoint and simply dispatches to `cargo xtask check`.

## Toolchains

FFHN keeps its exact Rust toolchain pins in two canonical files:

1. [../rust-toolchain.toml](../rust-toolchain.toml) owns the default stable workspace toolchain
2. [../tooling/rust-tooling.env](../tooling/rust-tooling.env) owns the full maintainer toolchain and QA-tool version set

The pinned QA nightly toolchain exists for three reasons:

1. `cargo +<qa-nightly-toolchain> miri test` is required for the maintained typed-observation strict-provenance proof
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

### Gate output and evidence

`cargo xtask check` owns the output contract for the full maintainer gate. Its default human mode
prints stable lifecycle events for every policy and child-command step, elapsed time, actionable
warning diagnostics, and a final pass or failure result. It intentionally does not stream normal
compiler progress or successful-test chatter.

```bash
./check.sh --verbosity verbose
./check.sh --format json
./check.sh --retain-passing-logs
```

`--verbosity verbose` streams raw child output. `--format json` emits one
`ffhn.gate-event@1` JSON event per line and never mixes raw child output into that stream.
Every raw stdout/stderr byte is captured while a step runs. FFHN retains raw evidence under the
managed sibling `.ffhn-artifacts/gate-logs/` directory when a gate fails; successful evidence is
discarded unless `--retain-passing-logs` is explicit. This keeps the normal path concise without
destroying failure evidence. The optional `--log-dir <DIRECTORY>` directs retained successful-run
evidence to a caller-owned directory.

Do not filter gate output with `grep` for words such as `warning` or `error`. Tool streams are not
a shared severity protocol, and negative tests may intentionally print error-shaped text. The
gate's structured lifecycle state and process exit status are authoritative.

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

Mutation testing:

```bash
cargo xtask mutants
cargo xtask mutants --scope runtime
cargo xtask mutants --scope tooling
```

That proof executes text, integer, decimal, money, semantic-version, and explicit-offset
date-time normalization under strict provenance. It protects the typed-value boundary; the normal
test suite separately covers public HTMLCut interop.

Rust source-structure enforcement:

```bash
cargo xtask structure check
```

Rust source-structure report:

```bash
cargo xtask structure report
```

`structure check` is fail-closed. It measures every maintained Rust source and test module in
`crates/ffhn-core`, `crates/ffhn-cli`, and `xtask`, plus every standalone fuzz target. Its
canonical policy is [../tooling/rust-source-shape-policy.toml](../tooling/rust-source-shape-policy.toml).
Each rule declares a role, accountable owner, rationale, split trigger, bounded source-shape
metrics, and the internal crate modules that role may name directly. The gate rejects an unowned
file, an unused or duplicate rule, malformed policy metadata, an expired rule review, a breached
budget, or a direct `crate::module` dependency outside the role's declared boundary. A root-level
`crate` import is treated as use of the crate's public facade; an explicit named module is the
architectural dependency being governed. `structure report` intentionally remains diagnostic: it
prints measurements and `UNOWNED` roles so a new policy rule can be designed before `check` is run.

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
3. `cargo xtask structure check`
4. `bash -n` over `check.sh` and every `scripts/*.sh` file
5. `shellcheck` over the same shell scripts
6. `cargo fmt --check`
7. `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
8. `cargo xtask miri`
9. `cargo xtask audit`
10. `cargo xtask audit --file fuzz/Cargo.lock`
11. `cargo deny check advisories bans licenses sources`
12. `cargo semver-checks` for `ffhn-core` against `semver-baseline/ffhn-core` with isolated managed `target` and `build` scratch roots
13. `cargo check --manifest-path fuzz/Cargo.toml --bins --locked`
14. `cargo clippy --manifest-path fuzz/Cargo.toml --bins --locked -- -D warnings`
15. `cargo +<qa-nightly-toolchain> fuzz check --fuzz-dir fuzz`
16. `cargo nextest run --no-fail-fast --workspace --all-targets --all-features --locked`
17. `cargo test --workspace --doc --all-features --locked`
18. `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked`
19. `cargo build --profile dist -p ffhn-cli --bin ffhn --locked`
20. the dist-profile `ffhn` binary at the active Cargo target root (FFHN configures `../.ffhn-artifacts/target/dist/ffhn` by default, or `${CARGO_TARGET_DIR}/dist/ffhn` when overridden) with `--version`
21. `cargo xtask coverage`
22. `cargo xtask hygiene clean --mode safe`
23. a final hygiene verification pass

The duplicate-dependency policy has one exact capability-stack exception: `io-lifetimes 2.0.4` remains required by `fs-set-times 0.20.3`, while the same current `cap-primitives 4.0.3` release uses `io-lifetimes 3` directly. Both are upstream requirements of FFHN’s no-follow capability storage. The exception names the precise old line rather than the crate family, so cargo-deny makes the entry stale as soon as upstream removes it.

Dependency freshness is intentionally separate from the required correctness gate. FFHN keeps the
freshness signal in [../.github/workflows/dependency-freshness.yml](../.github/workflows/dependency-freshness.yml),
which runs the pinned `cargo-outdated` tool without blocking unrelated correctness work. That
workflow installs only `cargo-outdated`, rather than the full QA-tool suite, because it executes no
other QA lane; a reported update remains a failing, review-required maintenance signal.

There is no separate rustdoc-coverage percentage gate. Public-surface documentation is enforced by `#![deny(missing_docs)]` in the Rust crates, so undocumented public items fail normal compilation and test builds.

## Mutation Testing

`cargo xtask mutants` runs the pinned cargo-mutants tool against two independently judged first-party scopes. The runtime scope mutates `ffhn-core` and `ffhn-cli` and runs their product tests; the tooling scope mutates `xtask` and runs its maintainer-policy tests. Both configurations use all features, locked Cargo resolution, a 120-second minimum test timeout, round-robin sharding, explicit error-return mutations, and exclude test modules from mutation.

Mutation testing asks whether the tests reject plausible behavioral changes; 100% line/branch coverage alone proves only that code executed. It complements coverage, fuzzing, and Miri and remains separate from `./check.sh` because complete campaigns contain thousands of mutants.

Every run uses cargo-mutants' copied-workspace mode and retains results under `../.ffhn-artifacts/mutation-runs/<scope>/mutants.out`. The mutation child process clears ambient Cargo target and build-root overrides before its checked-in configuration assigns checkout-local scratch roots, so no worker can reuse or overwrite another worker's build evidence. A clean run replaces the selected scope's prior generated result tree; `--iterate` deliberately retains it for a local test-writing loop.

The dedicated [mutation workflow](../.github/workflows/mutants.yml) runs on every pull request, enumerates each pull-request diff first, then runs and retains a runtime or tooling mutation result only when that scope contains changed production mutants; an empty scope is a successful explicit zero rather than fabricated evidence. Its stable `Mutation testing` aggregate owns the pull-request result and is suitable for branch protection. Every `main` push, weekly run, and manual dispatch executes the complete generated plan containing twelve runtime shards and four tooling shards, so test-only changes are checked after merge as well. Machine selectors such as `0/12` remain distinct from artifact-safe identities. The summary authority rejects missing, unexpected, flattened, malformed, incomplete, empty, contradictory, or non-catching shard evidence before aggregating caught, missed, timed-out, and unviable counts.

Missed mutants and timeouts fail the campaign. FFHN maintains no survivor allowlist, skip annotations, or compatibility exemptions; fix the governing test or production design and rerun the affected scope. `cargo xtask mutants --scope <runtime|tooling> --iterate` is suitable only for a local test-writing loop, cannot combine with CI selectors, and never substitutes for a complete authoritative campaign.

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
GitHub CI also runs a separate cross-platform Rust gate on macOS arm64 and Windows x64 for formatting, Rust source-structure enforcement, Clippy, tests, advisory/license policy, fuzz compile smoke, and the maintained semver lane.
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

1. AFAD-managed Markdown under `docs/` and `fuzz/` must carry AFAD frontmatter using the canonical AFAD protocol version from `.codex/PROTOCOL_AFAD.md`; the root `README.md`, `CONTRIBUTING.md`, and `changelog.md` remain special docs and are validated without forced AFAD metadata
2. public Markdown local links and maintained repo-file path mentions must still resolve
3. public source, measurement, report, and reset documentation must name the current observation-graph contracts and must not reintroduce retired configuration or report families
4. the repository-root `AGENTS.md` must remain the only maintained agent entrypoint, and shadow agent-entrypoint files under `.codex/` must not be reintroduced
5. the README, `docs/cli.md`, and CLI integration tests must agree on the current command grammar and result documents
6. public Markdown must not mention unknown `ffhn.*` document ids
7. user-facing Rust string literals in the maintained source tree must not mention unknown `ffhn.*` document ids
8. the README, platform-support docs, and release protocol must stay aligned with the canonical release-target and release-asset inventory emitted by `scripts/release-targets.sh`
9. every documented `cargo xtask refresh-semver-baseline` invocation in public Markdown must include the required `--git-ref` argument

The `ffhn-cli` test suite complements that repository lint by asserting that live help output, help/version write-failure handling, and document write-failure text render from the same core-owned operation, limit, and document contract instead of carrying separate hard-coded labels.

## Fuzzing Policy

The automatic gate security-audits the standalone fuzz lockfile, lint-checks the standalone fuzz package, and compile-smokes the maintained harnesses. FFHN routes RustSec auditing through `cargo xtask audit` so transient advisory-database fetch failures are retried by one maintained entrypoint instead of being reimplemented piecemeal across local and CI lanes.

RustSec scans optional packages recorded in a lockfile even when no maintained feature graph can compile them. The sole maintained exception is `RUSTSEC-2026-0235` for `rkyv 0.7.46`, an optional `rust_decimal` integration FFHN never enables. Before passing `--ignore`, `cargo xtask audit` executes `cargo tree --all-targets --all-features --target all --invert rkyv@0.7.46 --locked` against the audited manifest and fails closed if that package is reachable or the proof cannot run. The exception therefore cannot mask a compiled vulnerable dependency.

Automatic:

```bash
cargo xtask audit --file fuzz/Cargo.lock
cargo check --manifest-path fuzz/Cargo.toml --bins --locked
cargo clippy --manifest-path fuzz/Cargo.toml --bins --locked -- -D warnings
cargo +<coverage-toolchain> fuzz check --fuzz-dir fuzz
```

The cross-platform CI test lane uses nextest's failure-focused status mode: passing tests are
summarized rather than printed one-by-one, while failure output remains visible.

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
