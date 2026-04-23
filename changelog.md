# Changelog

Notable changes to this project are documented in this file. The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [3.0.0] - 2026-04-23

- Bumped `htmlcut-core` from `v4.3.0` to `v4.4.0` and refreshed the maintained lockfiles before the public `3.0.0` release.
- Hardened the Windows standalone release ZIP path by normalizing temporary roots through the real runner temp directory, preferring bash-native ZIP extraction before PowerShell fallback, and making the PowerShell ZIP packager emit forward-slash archive members after explicitly loading both compression assemblies.
- Tightened the maintained release protocol around large-PR diff inspection, required conversation resolution, explicit fetch-plus-fast-forward sync, dirty-checkout capture via `release-prep/`, and worktree branch handoff during closeout; the repo-contract tests now enforce those release-doc invariants.
- Hardened watch-root and target loading so `run --all` now fails fast on a missing/non-directory watch root, ignores unrelated directories without a `target.toml` marker, and surfaces explicit target-load filesystem faults as fatal instead of misclassifying them as `config_invalid`.
- Reworked successful snapshot persistence into a rollback-safe transaction so a later `state.json` write failure no longer poisons the previously valid baseline or deletes still-referenced history artifacts.
- Tightened notification delivery reporting by capturing hook stderr when available and making failed deliveries surface as CLI exit-code failures even when the monitored content outcome itself stayed successful.
- Made `run_finished_at` consistent by stamping the final emitted run body after notification delivery but before the last `last_run.json` write attempt, and updated the public docs to match that contract exactly.
- Switched the README quick start to the deterministic checked-in file-target example, expanded the example README, and tightened the report plus target docs around extraction records, snapshot references, compare-time LF normalization, and notification edge cases.
- Documented the shared structured process-error detail used by `persist.error` and batch `fatal_error`, and clarified in the maintainer policy docs that embedded report vocabularies and named subobjects are part of FFHN's public contract surface too.
- Broke up two remaining codebase god-files by splitting fetch-source handling into dedicated HTTP and file modules and moving the `ffhn.run_report` contract into its own report submodule.
- Added the standalone fuzz manifest to the maintained dependency-freshness gate, updated the fuzz maintainer docs to match the split target modules, and kept the maintained quality-gate docs aligned with the real `cargo xtask check` plan.
- Strengthened repository contract tests so public Markdown local links and maintained repo-file path references stay valid, and added a CLI integration test that exercises the documented local quick-start flow end to end.
- Tightened the documentation summaries and maintainer guidance so `ffhn.extraction_record` and `ffhn.notification_payload` are enumerated consistently as FFHN-owned contracts, and re-verified the documented README/example/source-build and host release-package flows against the live repo.
- Bumped the unreleased workspace line to `3.0.0` because FFHN's public run, batch, and notification-hook contracts now expose new structured persistence and fatal-error detail instead of the previous narrower shapes.
- Reworked live persistence reporting so FFHN now preserves structured `persist.error` detail across all live persist failures, fails the CLI on successful runs whose final `last_run.json` write fails, and counts live persist issues explicitly in batch reports.
- Replaced batch's old chunk-barrier scheduling with a real bounded worker queue, changed per-target `fatal_error` entries from free-form strings into structured error objects, and made batch panic surfaces preserve the underlying panic message.
- Added the frozen `ffhn.notification_payload` hook-stdin contract, documented it as a pre-delivery snapshot, and extended tests plus fuzzing so run reports, batch reports, and notification payloads all stay aligned.

## [2.1.0] - 2026-04-22

- Pinned the maintained Rust toolchain to `1.95.0`, declared `rust-version = "1.95"` across the workspace and standalone fuzz package, and aligned GitHub Actions plus maintainer docs with that exact toolchain instead of the moving `stable` channel.
- Bumped `htmlcut-core` from `v4.2.1` to `v4.3.0`, moved the maintained `toml` dependency to the current `1.1.x` line, and refreshed the maintained lockfiles accordingly.
- Clarified the release protocol so substantive unpublished work must land on `main` before cutting a narrow `release/X.Y.Z` branch, instead of getting bundled into the release-only version bump PR.
- Clarified the post-release semver-baseline refresh protocol so protected repositories land that housekeeping change through a short follow-up PR instead of relying on a direct push to `main`.
- Audited and corrected the public documentation and examples so the README, maintainer docs, report/target docs, and file-target example all match the live code and release scripts more literally.
- Hardened the repo-contract gate so release-target documentation stays aligned with `scripts/release-targets.sh`, checked-in public examples stay runnable, and every documented `cargo xtask refresh-semver-baseline` command includes `--git-ref`.
- Broke up the remaining `xtask`, CLI-adapter, runtime-run, target-contract, fetch, and report-contract god-files into smaller modules so maintenance, coverage, and contract checks stay easier to audit end to end.
- Extended the strict tracked-coverage inventory to the new report-contract submodules and kept the workspace at 100% tracked line and branch coverage after the refactor.
- Patched both maintained lockfiles to `rustls-webpki 0.103.13` for `RUSTSEC-2026-0104`, taught `cargo xtask check` to audit the standalone `fuzz/Cargo.lock`, and isolated semver-checks scratch output so stale baseline-local `target/` trees are cleaned automatically.
- Hardened internal fetch and notification delivery helpers so malformed direct targets and serialization edge cases return structured failures instead of panicking.
- Split the remaining oversized `runtime::state`, `runtime::persist`, `runtime::status`, and report-contract test modules into focused submodules so production files stay smaller and test ownership is easier to navigate.

## [2.0.1] - 2026-04-22

- Reworked FFHN's public release system into explicit source archives, versioned platform packages, and one `ffhn-X.Y.Z-checksums.txt` manifest, replacing the old raw-binary plus per-file `.sha256` release layout.
- Hardened release automation around packaged-asset smoke tests, draft-first publication, provenance attestations, and tag-version-pinned reruns so release repair stays safe even after `main` has advanced.
- Added dedicated platform-support documentation and refreshed the README plus maintainer docs to document binary-package install, the maintained asset inventory, and the distinction between FFHN-owned source archives and GitHub's auto-generated `Source code` links.
- Bumped `htmlcut-core` from `v4.2.0` to `v4.2.1` and refreshed the workspace lockfiles to match.
- Fixed `cargo xtask refresh-semver-baseline` to accept the documented `--git-ref` argument and rebuild the checked-in `ffhn-core` baseline from the explicit published Git ref instead of the live worktree.

## [2.0.0] - 2026-04-20

- Updated FFHN to `htmlcut-core` `v4.2.0`, aligned all persisted interop identity with the published `htmlcut-v1` profile, and shipped the public Rust `2.0.0` release line.
- Added file-backed targets, durable `target_id` validation, dry-run execution, split transient versus permanent failures, machine-usable change summaries, shell-hook notifications, multi-target batch execution, and rolling snapshot history.
- Tightened the CLI and fetch contract by rejecting `--jobs 0` as invalid usage and explicitly covering `fetch.engine = "browser"` as a stable report-level contract value over the current HTTP transport backend.
- Tightened batch orchestration by rejecting duplicate requested target ids and zero core concurrency, and preserved parsed invalid-state phases in `status` when FFHN can still recover them from stored state.
- Reclassified live persistence failures after reportable outcomes as structured transient `persist_error` run reports instead of fatal process exits, and clarified the locking plus notification-payload contract in the public docs.
- Tightened file-target semantics by rejecting HTTP-only fetch knobs, distinguishing local file-access failures as `fetch_source_error`, normalizing unreadable `state.json` into structured invalid-state reports, and extracting notification plus change helpers out of the run pipeline god-file.
- Short-circuited invalid target `run` and `status` requests before lock/state work, documented the full notification-hook environment contract, and clarified that file-backed sources are UTF-8 decoded.
- Added a standalone fuzz package with checked-in seeds for target documents, report/state validation, and file-target dry-run coverage.
- Promoted `./check.sh` as the canonical maintainer gate, refreshed the maintainer docs, added `CONTRIBUTING.md`, and published example target configuration for file-source plus notification workflows.
- Kept the workspace on the unreleased `2.0.0` line, restored the public release history to `v1.0.0`, and aligned semver enforcement with that boundary.
- Tightened dry-run consistency by taking the shared run lock before inspection, surfaced mid-discovery watch-root filesystem faults during `run --all`, and made maintainer shell-script discovery fail closed instead of silently skipping broken entries.
- Centralized the internal `ffhn-core` workspace dependency, made AFAD-frontmatter versions protocol-derived and test-verified, added automated parity checks for `.claude/CLAUDE.md` and `.gemini/GEMINI.md`, kept checked-in target examples under contract tests, and removed release-version coupling from the shipped demo target configuration.
- Moved the CLI command catalog, execution-mode descriptions, hard-limit text, and command/document ids into a core-owned structured registry, switched `ffhn-cli` help and usage errors to render from that registry, and added build-failing contract linting for README and `docs/cli.md` command catalogs plus unknown FFHN operation/report ids in public Markdown.
- Extended that contract enforcement to maintained Rust source literals and document write-failure messages, so CLI help/error/output labels now render from the same core-owned document contract and unknown FFHN operation/report ids fail the build even in source-held user-facing strings.
- Adopted a maintained GitHub CLI release protocol, published release and versioning policy docs, documented the exact public asset inventory, and aligned release machinery with the Rust `Check` gate and standalone target matrix.

## [1.0.0] - 2026-03-20

- Initial release.
