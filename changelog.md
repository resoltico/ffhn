# Changelog

## [Unreleased]

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
