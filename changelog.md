# Changelog

Notable changes to this project are documented in this file. The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Added deterministic JSON Pointer and HTML scalar measurements for HTTP and local UTF-8 sources, with explicit source/fetch contracts, exactly one selected HTML match, exact original evidence, structured HTMLCut diagnostics, and safe final-response URL handling for relative HTML links.
- Added typed accepted observations for integer, decimal, money, Semantic Version, and explicit-offset date-time values. Every observation preserves its acquisition evidence, comparison projection, parser identity, declared type parameters, normalized canonical value, and source-specific HTMLCut evidence where applicable.
- Added named typed policy conditions with exact comparison algebra, per-condition temporal state, fixed initial and last-accepted references, level-state transitions, source-health escalation, permanent-configuration-error episodes, and deterministic `on_condition` and `on_run` eligibility.
- Added durable process-stdin delivery routes and a per-target outbox. FFHN commits immutable event payload bytes with state before delivery, uses deterministic event identities and bounded retry scheduling, reports overflow without evicting older records, and makes post-process persistence uncertainty explicit in run and reset reports.
- Added `html_text` DOM canonicalization for CSS selections. FFHN keeps original selected text as raw evidence, uses only a detached HTMLCut clone for comparison text, preserves original candidate and diagnostic facts, has no implicit attribute deny-list, and rejects unsupported projection shapes or clone canonicalization on attribute measurements instead of accepting inert configuration.
- Added bounded multi-target runs, target discovery, structured run/status/reset reports, explicit output formats, and the `ffhn reset` command. Reset takes the target lock and blindly removes only FFHN-owned storage, leaving the target definition ready for a clean initialization.

### Changed

- Replaced the prior monitoring contracts with the version-2 measurement engine and its target, state, report, snapshot, and notification vocabulary. Existing persisted state is intentionally incompatible: FFHN neither reads nor migrates it, and each target must be explicitly reset before initialization under the new contract.
- Reworked persistence around isolated target roots, source-kind measurement digests, canonical UTC timestamps, validated evidence, symlink-resistant normal I/O, crash-durable state/outbox commits, and an exact clean-reset lifecycle. A valid observation is the sole path that advances an accepted baseline.
- Reworked operational failure handling so source outages and unparseable values are temporal health facts, while invalid target and HTMLCut contracts are permanent episodes. Both classifications are typed, closed vocabularies that retain structured diagnostic evidence and cannot be invented in persisted state.
- Raised the Rust floor and maintained stable toolchain to Rust `1.97`/`1.97.0`, updated HTML measurement support to the public `htmlcut-core v11.0.0` contract, refreshed the Cargo dependency graph and QA tools, and updated immutable GitHub Actions pins.
- Reworked maintainer and release operations around managed artifact roots, reproducible coverage and Miri proofs, portable delivery fixtures, maintained release assets and checksums, strict post-publication verification, Dependabot closeout, and a post-release semver-baseline refresh.

## [8.1.0] - 2026-05-19

### Changed

- Bumped the maintained `htmlcut-core` dependency to `10.2.0`, removed FFHN's
  temporary workspace-root `servo_arc` and `tendril` overrides, and now rely on
  HTMLCut's shipped downstream-safe selector/parser stack instead of carrying a
  local release-gap patch layer.
- FFHN no longer parses CSS selectors through `scraper` during target validation; it now
  validates CSS-selector and delimiter-pair contracts through HTMLCut's interop plan surface
  so the contract check matches the upstream execution stack FFHN actually runs.

### Fixed

- The maintained FFHN-to-HTMLCut strict-provenance proof now exercises both a CSS-selector
  target and a delimiter-pair target against the shipped `htmlcut-core v10.2.0`
  dependency graph, and the maintainer docs/NOTICE now describe that upstream
  ownership truthfully instead of pointing at removed FFHN-local patch overrides.

## [8.0.0] - 2026-05-17

### Changed

- Added a maintained `cargo xtask miri` proof lane for the FFHN-to-HTMLCut selector boundary, wired it into `./check.sh`, tightened the host/devcontainer bootstrap so the pinned QA nightly toolchain now installs `miri` plus `rust-src` as part of FFHN's normal maintainer contract, and temporarily patched `servo_arc` plus `tendril` locally so FFHN can prove the current pinned HTMLCut interop stack under strict provenance until the later HTMLCut dependency bump removes the override.
- Renamed the old "coverage toolchain" concept to a truthful QA nightly toolchain across `tooling/rust-tooling.env`, maintainer scripts, fuzzing commands, and developer/release docs because the same pinned nightly now owns Miri, coverage, and fuzzing work instead of pretending to be coverage-only.
- Bumped the workspace line to `8.0.0` and updated the maintained `htmlcut-core` dependency to `10.1.0`.
- Rebuilt the FFHN target and report contract around explicit compare ownership: target selection now only locates the fragment, `[compare]` now owns the projection basis (`text`, `inner_html`, or `outer_html`) plus rendering controls, `compare.canonicalization` now defaults to `[]`, and the persisted/report vocabulary now speaks in terms of compare sources and compare artifacts instead of the older overloaded canonical-text/output-language.
- Added monitoring-contract fingerprinting to persisted state and extraction artifacts so FFHN can tell when a saved baseline belongs to a different target definition instead of silently reusing it across incompatible compare contracts.

### Fixed

- Fixed the HTMLCut-interop seam: FFHN now compares the projection named in `[compare].basis` instead of always comparing rendered text, so `outer_html` and `inner_html` monitoring actually work as configured.
- Fixed dry-run behavior for incompatible baselines so `ffhn run --dry-run` continues as a safe preview path under the shared lock, while live `run` and `status` report the incompatible baseline explicitly and require the operator to clear or replace it before mutating durable state.
- Refreshed the public target, report, and starter examples/docs to the new compare contract, updated the maintained fuzz/contract fixtures, and rewired stateful test helpers so monitoring-contract digests are derived from the target definition instead of from stale magic values.
- Fixed the contributor-devcontainer proof chain so `validate-devcontainer.sh` now verifies the nightly Miri entrypoint and promotes the validated image to `ffhn-devcontainer:local`; the documented `FFHN_DEVCONTAINER_SKIP_BUILD=1 ./scripts/run-devcontainer-check.sh` release and maintainer path now reuses the exact image that was just proven instead of drifting onto an ambient stale local tag.

## [7.0.0] - 2026-05-15

### Changed

- Bumped the workspace line to `7.0.0` because FFHN intentionally breaks its public contracts again: `ffhn.target`, `ffhn.extraction_record`, `ffhn.state`, `ffhn.run_report`, `ffhn.last_run_snapshot`, `ffhn.notification_payload`, `ffhn.batch_run_report`, and `ffhn.status_report` now ship on schema version `1`, `3`, or `4` as applicable; target notifications are now modeled as `notification_endpoints` plus `notification_routes`; persisted state now models `baseline` plus typed `last_run`; `last_run.json` now stores a dedicated wrapper artifact instead of pretending to be the final emitted `ffhn.run_report`; run reports now use `baseline_phase_before_run` / `baseline_phase_after_run` without the old target-status projection; and extraction records remain FFHN-owned instead of persisting raw upstream HTMLCut metadata.
- Bumped the maintained workspace dependency floor to `assert_cmd 2.2.1`, `clap 4.6.1`, `time 0.3.47`, and `htmlcut-core v10.0.0`; FFHN's frozen HTMLCut adapter now builds plans through HTMLCut's current interop types while continuing to reject unsupported HTMLCut selection or output modes outside FFHN's own contract.
- Added explicit CLI output-format selection for `run` and `status`: compact `json`, `json-pretty`, and human-oriented `summary` now render the same canonical FFHN result documents without changing the underlying machine contract.
- Centralized the maintained Rust toolchain, coverage toolchain, and Cargo QA tool version pins in `tooling/rust-tooling.env`, added the repo-owned `bootstrap-rust-tools.sh` installer, and aligned CI plus the contributor devcontainer with that one manifest instead of duplicating tool-version facts across workflows and setup docs.
- Added a first-class artifact hygiene system: FFHN now keeps Cargo output in managed sibling roots by default, exposes `cargo xtask hygiene report|verify|clean`, isolates coverage and semver scratch under typed managed roots, validates the same policy in the contributor devcontainer, and documents the maintained cleanup workflow instead of relying on ad hoc `target/` deletion.
- Rebuilt the maintained documentation index into a task-oriented routing page under `docs/README.md` and clarified the root README so the storefront now uses plain language, an explicit quick-start sequence, and direct links to the quick start, full docs index, and highest-signal contract references.
- Standardized the public naming split across maintained docs: `FFHN` is now the prose product name, while lowercase `ffhn` is reserved for literal identifiers such as commands, crate names, schema ids, file names, release assets, and URLs; the repo-contract suite now enforces that distinction for public Markdown prose.
- Routed maintained RustSec auditing through `cargo xtask audit`, which now owns bounded retries for transient advisory-database fetch failures and is shared by the local maintainer gate plus cross-platform CI instead of duplicating raw `cargo audit` calls in multiple lanes.

### Fixed

- Unified disabled-target behavior across explicit runs, dry-runs, and watch-root discovery: FFHN now keeps disabled targets visible in discovered batches, reports them as `skipped_disabled`, and never executes their fetch or extraction stages.
- Replaced raw fatal stderr for explicit missing or unreadable target paths with structured `target_unavailable` / `unavailable_target` run and status results, and added trusted `display_name` identity to valid run and status documents so operators do not have to remap opaque target ids by hand.
- Simplified the top-level CLI contract so bare `ffhn` is clear usage error, `ffhn --version` is one parseable line, misplaced operation flags get directed recovery guidance, and the built-from-source path is documented against FFHN's managed artifact roots instead of hidden behind `cargo run`.
- Realigned the release smoke and post-release verification path with the shipped CLI contract: packaged artifact checks now treat `ffhn --version` as one exact line instead of a two-line banner, and the release protocol plus release-script tests now enforce that same owned contract.
- Tightened summary-mode CLI rendering so valid run reports without trusted display names, partial fetch reports without a final URL, and fatal batch entries without a path all render cleanly through the same maintained contract instead of relying on implicitly covered happy-path shapes.
- Reworked live persistence finalization so `persist.state_commit` and `persist.last_run_write` are separate durable write paths, notification routing no longer races a later `last_run.json` downgrade, persist timing reflects the real write boundaries, and batch outcome counts now report any persist failure instead of only the old `persist_error` result shape.
- Hardened snapshot persistence by staging history archives inside the same transaction boundary as `state.json`, preserving the primary write failure when rollback also fails, and surfacing rollback/cleanup detail through a dedicated structured `persist_transaction` process-error kind instead of collapsing it into generic internal noise.
- Enforced local-file `fetch.max_bytes` as a bounded read instead of a post-read check, extended public Rust read APIs with coherent target/report view types plus growable non-exhaustive vocabularies, and removed the panic-prone raw-string `TargetPaths::new` constructor from the public boundary.
- Fixed the HTMLCut v10 file-target seam so FFHN no longer forces `file://` fetch URLs through HTMLCut's HTTP-only input-base contract; file targets now preserve HTMLCut's documented rewrite behavior, including warning with `EFFECTIVE_BASE_URL_UNRESOLVED` when URL rewriting has no effective HTTP(S) base.
- Moved dependency freshness out of the required correctness gate, added markdown-embedded target example validation plus toolchain-manifest drift checks to the repo-contract suite, refreshed the maintained fuzz seeds and public docs to the new contracts, and made maintained shell/Rust entrypoints scrub stale ambient compiler overrides instead of inheriting broken host LLVM paths.
- Path-gated the `contributor-devcontainer-gate` CI job so it fires only when devcontainer-relevant files actually change (`.devcontainer/`, the devcontainer helper scripts, or `check.sh`); non-devcontainer PRs now skip the full Docker build-and-run cycle entirely, reducing typical PR wall-clock time from ~45 minutes to ~3 minutes.
- Added a `devcontainer-changes` detection job that computes a git diff of the PR's changed files against the devcontainer trigger paths before the gate is evaluated; the aggregate `Check` required-status job uses `if: always()` with explicit `${{ toJSON(needs.*.result) }}` failure detection so a correctly skipped `contributor-devcontainer-gate` does not prevent `Check` from being reported or block merge — only a failed or cancelled gate prevents success.
- Made the contributor-container bootstrap contract self-contained by removing the image-build dependency on an untracked helper import, moved the validator's Dev Container CLI helper onto a pinned supported Node 20 runtime, pinned a Docker Buildx CLI plugin into that helper so Dev Containers no longer falls back to Docker's deprecated legacy builder path, fixed the helper Dockerfile so its base-image override is declared in valid Docker arg scope, taught the helper path to trust the mounted repo as a Git safe directory so Buildx can capture commit metadata cleanly, and widened the CI devcontainer trigger set to include `tooling/rust-tooling.env`, `scripts/bootstrap-rust-tools.sh`, and the workflow file that defines the gate so contributor-environment drift cannot bypass validation.
- Added a Windows Defender exclusion for the Cargo `target/` directory in `cross-platform-rust-gate` before any Cargo operations begin, eliminating AV scan overhead that otherwise scans every file write during compilation.
- Fixed the maintained release-shell Cargo-target helpers on Windows by owning the Git-Bash/native-`python3` path-conversion boundary explicitly, so release scripts and cross-platform release checks now read the committed `.cargo/config.toml` artifact roots correctly, keep default and custom Cargo target roots in one stable `/d/...` path dialect, and avoid silently falling back to a repo-local `target/` path.
- Fixed the GitHub Actions Rust-cache ownership boundary by excluding `~/.cargo/bin` from cached state in both CI and release publication lanes, keeping tool entrypoints under the repo-owned bootstrap contract, and tightening `bootstrap-rust-tools.sh` plus devcontainer validation to prove the stable Cargo surface with a real subcommand instead of trusting version output alone.
- Fixed a Windows-specific flaky test in the `ffhn-core` fetch suite where dropping a `TcpListener` did not guarantee immediate `ECONNREFUSED` — the port lingered and the 1-second timeout fired, producing `FetchTimeout` instead of `FetchNetworkError`; replaced the dropped-listener pattern with a server that accepts and reads the request then resets the stream, producing a reliable `FetchNetworkError` on all platforms.
- Tightened `ffhn.run_report` validation around lifecycle ownership: live successful runs now require `baseline_phase_after_run = has_baseline`, dry-run reports may not mutate the durable phase, `initialized` requires `baseline_phase_before_run = never_succeeded`, and live `changed` / `unchanged` runs require `baseline_phase_before_run = has_baseline`.
- Removed the legacy top-level `state_phase` recovery path from invalid persisted-state handling; FFHN now recovers invalid-state phase only from the current `ffhn.state baseline.kind` shape instead of carrying an obsolete generic-surface compatibility path.
- Removed the duplicate `workspace-version.sh` release entrypoint and routed release docs, workflows, and helper scripts through the canonical `workspace-package-field.sh version` contract; the repo-contract AFAD parser now accepts only the committed `**Version:**` protocol heading syntax.
- Fixed `ffhn run` selector diagnostics so missing `--target` or `--all` is named explicitly, prefixed fatal stderr output with `error:` consistently, and made release scripts surface missing tokens, unsupported targets, or incomplete asset inventory before complaining about a dirty tracked checkout.
- Rebuilt the maintained `run` and `status` help surfaces around explicit grammar, runnable command examples, structured-stdout notes, and separate operational notes; target-authoring TOML blocks no longer masquerade as command grammar, and conflicting `run` invocations now reuse the same canonical usage synopsis as `--help`.
- Made `ffhn.status_report` schema `4` expose target `enabled` for every valid target so disabled targets no longer masquerade as generic `pending`, fixed `lock_unavailable` reports to recover lifecycle metadata from one stable post-contention state view instead of fabricating `never_succeeded`, and switched the maintained devcontainer shell entrypoints to linear Docker build progress by default so terminal and agent users can read real build logs.

## [6.0.0] - 2026-05-03

### Fixed

- Made dry-runs and status waits resilient behind active live-run locks while surfacing missing or non-directory watch roots at the watch-root path instead of a fabricated nested `target.toml` path.
- Added a pinned Ubuntu `24.04` contributor devcontainer, a writable-cache repair hook, maintained `validate-devcontainer.sh` and `run-devcontainer-check.sh` entrypoints, and the matching docs/CI contract so FFHN's preferred development path is one reproducible container environment instead of a hand-assembled host toolchain; the contributor workflow now keeps both `target/` and `fuzz/target/` on dedicated Docker volumes, and the semver lane precreates a namespaced temp-root scratch tree before `cargo semver-checks` starts so mounted host filesystems do not strand busy tombstones under the repository `target/` tree.
- Strengthened the contributor-container validation path so `validate-devcontainer.sh` now proves the real Dev Container client flow in addition to the raw image contract, derives that helper path from the already-built contributor image instead of a second remote base image, `run-devcontainer-check.sh` can reuse the validator's warmed image in CI instead of rebuilding it immediately, and `./check.sh` shows normal Cargo startup output instead of looking hung on cold runs.
- Hardened the host-native maintainer and release path for flaky repository mounts by making `cargo xtask`'s final dist smoke plus coverage report and `build-release-artifact.sh` all honor the active `CARGO_TARGET_DIR` instead of assuming a repo-local `target/` tree.
- Release handoff scripts now reject tracked checkout drift before publishing or verifying GitHub releases, so maintained local release entrypoints all derive version and asset expectations from one checkout state.
- Bumped the workspace line to `6.0.0` because `ffhn-core` intentionally breaks its public configuration and notification-report surfaces again: file fetches now use their own typed schema instead of inheriting HTTP-only knobs, notification payloads and delivery entries now use explicit write and delivery result objects, and the exported Rust contract types now expose a coherent read API instead of half-private result bags.
- Tightened FFHN's public target and report contracts by making target source and fetch config discriminator-specific types, removing the redundant notification-trigger field from hook payloads and delivery records, enforcing stronger state/status/timestamp invariants during decode, removing the duplicate AFAD `version:` release stamp from maintained docs, hardening unsafe-code and truncating-cast lint boundaries, and making catalog/time/error-mapping internals more explicit and less reorder-fragile.
- Hardened the maintained release tooling for Windows and cross-platform portability by normalizing bash source paths, centralizing release-target script access through a Rust wrapper in `xtask`, making the release helpers self-describing with `--help`, and extending CI with a real macOS/Windows Rust gate plus retry-hardened toolchain bootstrap.
- Raised the cross-platform GitHub CI timeout budget to `150` minutes for Windows and `30` minutes for macOS so the Windows required-check lane can finish a cold `cargo nextest` build and run, dependency-policy, and semver verification instead of expiring mid-run and leaving the aggregate `Check` gate unset; added a `.config/nextest.toml` profile that warns after 60 seconds and terminates any single test after 180 seconds so a hanging test cannot silently exhaust the job budget.
- Fixed the Windows release-prep gate after live protocol exercise by making semver-baseline assertions line-ending agnostic and by teaching the delayed notification timing test to drain FFHN's JSONL hook payload before sleeping, so the Windows runner no longer hangs inside a healthy `run_once` notification proof.
- Removed Bash-4-only `mapfile` usage from the maintained release and contributor-container scripts so the release protocol and devcontainer validator run correctly under stock macOS `/bin/bash` as well as newer GNU Bash installs.
- Tightened the public CLI and report contract so `run --help` and `status --help` teach target layout directly, invalid target or state reports carry structured `error_detail`, `./check.sh --help` behaves like a real entrypoint, and packaged archives reject macOS metadata sidecars during build and smoke verification.
- Hardened the executable and library boundaries by switching the CLI to `args_os`, making notification-hook stdin newline-delimited JSONL-friendly, moving the checked-in example hook log to a target-local path, narrowing `ffhn-core`'s public read surface to deliberate view/projection types instead of internal config/report section structs, and splitting the major target/report/state/status/notification wire DTOs away from their validated domain models.
- Fixed the advertised minimal HTTP target so it validates as written, reclassified semantic target-config failures from `toml` to `contract` in structured status/run error detail, and made local source-archive plus checksum generation discoverable through a maintained `build-release-source-archives.sh` entrypoint and clearer checksum-prerequisite diagnostics.
- Reclassified persisted `state.json` and snapshot `extraction.json` contract failures from generic `json` decode errors into `contract` errors with concrete artifact paths where FFHN knows them, tightened the public Rust run-report API around canonical notification-delivery and persist-write status access instead of lossy helper booleans, and made the maintained local release builders refuse tracked dirty checkouts before they can emit mismatched assets or checksums.
- Clarified the executable and maintainer help boundaries by teaching that missing or unreadable `target.toml` paths remain fatal process errors, documenting the required `cargo xtask refresh-semver-baseline --git-ref <REF>` input directly in subcommand help, and aligning the core batch-runtime docs with the live `outcome_counts.notification_failure` aggregate.
- Made the curated `cargo xtask coverage` lane run test binaries with `--test-threads=1`, which keeps the instrumented CLI integration target deterministic when it spawns child `ffhn` processes under `cargo llvm-cov`.
- Closed a repo-integrity gap around the new contributor-container and split-module surfaces by tracking the maintained files, making repo-contract tests fail when public docs or maintained source trees depend on dirty-worktree-only files, and promoting the CI devcontainer lane from validator-only smoke to the full headless `run-devcontainer-check.sh` proof path.
- Unified `xtask`'s maintained-surface inventory so trackedness checks, watchlist starter-target discovery, and checked-in example assets now derive from one canonical owner instead of separate partial root lists; the repo-contract and coverage test authorities were split into smaller modules, and the fuzz maintainer doc freshness cue now matches the current change date.
- Removed the misleading `artifacts.current_valid` / `artifacts.previous_valid` bag from `ffhn.status_report` so snapshot presence and invalid-state reporting are the only status signals consumers need to interpret.

## [5.0.0] - 2026-05-03

- Bumped `htmlcut-core` from `v5.0.0` to `v7.0.0` and refreshed the maintained lockfiles. Adapted FFHN's frozen `htmlcut-v1` interop layer to the `v7.0.0` API surface: renamed `selected_match` field access to `selected_matches` (now a Vec that holds one entry for Single/First/Nth modes after validation), converted `DiagnosticCode` values to strings at the FFHN boundary, and added an explicit rejection for the new `SelectionMode::All` variant which is outside FFHN's frozen interop profile.

## [4.0.0] - 2026-04-24

- Bumped the workspace line to `4.0.0` because `ffhn-core` now intentionally breaks its previously published public surface: generic document bags validate on deserialize, durable `target_id` and snapshot-artifact paths are carried by validated value types instead of raw strings, aggregate batch helpers now construct through validated APIs instead of public invalid-state bags, and the structured process-error taxonomy split `htmlcut` into `contract`, `htmlcut_interop`, and `internal`.
- Bumped `htmlcut-core` from `v4.4.1` to `v5.0.0`, refreshed the maintained lockfiles, and verified that FFHN's current frozen `htmlcut-v1` interop integration still passes the full maintained gate suite unchanged.
- Hardened FFHN's contract boundaries by validating top-level documents during deserialize, rejecting Windows drive-letter and UNC snapshot artifact paths, enforcing durable `target_id` parsing at the CLI boundary, splitting process-error kinds into `contract` / `htmlcut_interop` / `internal`, and aligning the maintained docs plus fuzz seeds with those stricter rules.
- Made the repository root `AGENTS.md` the documented agent entrypoint, added parity redirect stubs for other agent surfaces, and aligned the maintainer contract checks plus docs with that single source of truth.
- Made the root `AGENTS.md` and `.codex/` files explicitly trackable in Git while keeping that maintainer-only agent configuration out of FFHN-owned source release assets through `git archive` `export-ignore`.
- Updated the maintained documentation set to the AFAD `4.0` protocol model while keeping the root `README.md`, `CONTRIBUTING.md`, and changelog human-first special documents instead of forcing AFAD frontmatter onto them.
- Hardened CLI output boundaries so failed top-level help/version writes and report/status document writes now surface as fatal process failures instead of silently exiting successfully.
- Replaced the curated Rust coverage allowlist with a derived maintained-source inventory, fixed the llvm-cov scorer so multiline statements and non-executable module-barrel files are interpreted correctly, and made the strict `nextest` gate run without fail-fast so documentation or contract drift no longer hides later failures.
- Broke up the remaining `xtask` app/plan god-files into focused modules and kept the maintainer gate/docs/fuzz coverage aligned with that new structure.
- Made `Focused Fragment History Notifier` the canonical public expansion of FFHN in the reader-facing overview and docs index while keeping the CLI/package description focused on product behavior rather than acronym expansion.
- Made the `CI` workflow's manual-dispatch recovery path explicit in the maintainer docs, clarified that GitHub may surface Dependabot as `app/dependabot`, and documented that cancelled sibling release reruns are not authoritative when another run already converged the published release object.

## [3.0.1] - 2026-04-23

- Bumped `htmlcut-core` from `v4.4.0` to `v4.4.1` and refreshed the maintained lockfiles before the public `3.0.1` release.
- Refactored the root README into a storefront-style overview, moved deeper onboarding into `docs/getting-started.md`, added `docs/run-reports.md` plus `examples/README.md`, and re-audited the maintained Markdown set so examples, metadata, and release docs match the shipped code and assets more literally.
- Standardized user-facing "watch root" terminology, documented `validate_target` plus the top-level help/version contract more literally, and tightened the CLI so root help prints the single-source version banner while `--version` remains top-level-only instead of masking invalid subcommand usage.
- Hardened packaged-artifact smoke and the release protocol against the bannered `ffhn --version` output by verifying both the version and package-description lines, and added maintained tests for the shared workspace package-field helper.
- Clarified the release protocol's missing-CI recovery path so a mergeable release PR with no delivered `Check` must be retriggered through PR reopen, a follow-up branch commit, and finally a maintainer-triggered `gh workflow run ci.yml --ref release/X.Y.Z` path backed by a newly dispatchable CI workflow, rather than being merged blind.

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
- Bumped the workspace line to `3.0.0` because FFHN's public run, batch, and notification-hook contracts now expose new structured persistence and fatal-error detail instead of the previous narrower shapes.
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
- Tightened the CLI and fetch contract by rejecting `--jobs 0` as invalid usage, removing the non-implemented `fetch.engine = "browser"` alias, promoting notification hooks from shell-string execution to explicit `program + args` process launches, and counting notification-delivery failures directly in batch outcome aggregates.
- Tightened batch orchestration by rejecting duplicate requested target ids and zero core concurrency, and preserved parsed invalid-state phases in `status` when FFHN can still recover them from stored state.
- Reclassified live persistence failures after reportable outcomes as structured transient `persist_error` run reports instead of fatal process exits, and clarified the locking plus notification-payload contract in the public docs.
- Tightened file-target semantics by rejecting HTTP-only fetch knobs, distinguishing local file-access failures as `fetch_source_error`, normalizing unreadable `state.json` into structured invalid-state reports, and extracting notification plus change helpers out of the run pipeline god-file.
- Short-circuited invalid target `run` and `status` requests before lock/state work, documented the full notification-hook environment contract, and clarified that file-backed sources are UTF-8 decoded.
- Added a standalone fuzz package with checked-in seeds for target documents, report/state validation, and file-target dry-run coverage.
- Promoted `./check.sh` as the canonical maintainer gate, refreshed the maintainer docs, added `CONTRIBUTING.md`, and published example target configuration for file-source plus notification workflows.
- Bumped the workspace line to `2.0.0`.
- Tightened dry-run consistency by taking the shared run lock before inspection, surfaced mid-discovery watch-root filesystem faults during `run --all`, and made maintainer shell-script discovery fail closed instead of silently skipping broken entries.
- Centralized the internal `ffhn-core` workspace dependency, made AFAD-frontmatter versions protocol-derived and test-verified, tightened automated checks around the repository's agent-entrypoint contract, kept checked-in target examples under contract tests, and removed release-version coupling from the shipped demo target configuration.
- Moved the CLI command catalog, execution-mode descriptions, hard-limit text, and command/document ids into a core-owned structured registry, switched `ffhn-cli` help and usage errors to render from that registry, and added build-failing contract linting for README and `docs/cli.md` command catalogs plus unknown FFHN operation/report ids in public Markdown.
- Extended that contract enforcement to maintained Rust source literals and document write-failure messages, so CLI help/error/output labels now render from the same core-owned document contract and unknown FFHN operation/report ids fail the build even in source-held user-facing strings.
- Adopted a maintained GitHub CLI release protocol, published release and versioning policy docs, documented the exact public asset inventory, and aligned release machinery with the Rust `Check` gate and standalone target matrix.

## [1.0.0] - 2026-03-20

- Initial release.
