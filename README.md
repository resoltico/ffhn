<!--
RETRIEVAL_HINTS:
  keywords: [ffhn, deterministic monitoring, ffhn-core, ffhn-cli, htmlcut, watch root, target.toml, snapshots]
  answers: [what is ffhn?, how do I run ffhn?, how does ffhn work?, where are the ffhn docs?, what does ffhn persist?]
  related: [docs/README.md, docs/cli.md, docs/targets.md, docs/reports.md, docs/contracts.md, CONTRIBUTING.md, fuzz/README.md]
-->
# ffhn

`ffhn` is a Rust workspace for deterministic monitoring of websites and local HTML files.

It fetches or reads a source, extracts the exact slice you care about through HTMLCut, canonicalizes that content before comparison, persists durable snapshots, and emits machine-readable JSON reports. The engine lives in `ffhn-core`. The CLI in `ffhn-cli` is only a process adapter that renders a core-owned command contract into help text, argument parsing, JSON output, and exit codes.

## Why FFHN

- Deterministic comparisons: FFHN compares canonicalized content instead of raw page noise.
- Clean ownership boundaries: FFHN owns fetching, persistence, reports, and notifications; HTMLCut owns extraction execution.
- Real operational surface: live runs, dry runs, batch runs, retained history snapshots, and shell-hook notifications are first-class features.
- Automation-friendly CLI: the supported operations emit stable JSON documents instead of human-only terminal text.
- Two source families: monitor `http` or `https` pages, or monitor local files through absolute paths.
- Sharper file-target contract: file sources reject HTTP-only fetch knobs instead of silently ignoring them, and FFHN expects local file bytes to decode as UTF-8.

## Install

Build from source:

```bash
cargo build --release -p ffhn-cli
./target/release/ffhn --help
```

Regular builds and normal CLI usage do not need nightly Rust. Nightly is only part of the coverage and fuzzing workflows.

Or download a standalone binary from [GitHub Releases](https://github.com/resoltico/ffhn/releases). The maintained release matrix currently covers macOS arm64, macOS x64, Linux x64 musl, and Windows x64.

The maintained public release assets are:

- `ffhn-<version>.zip`
- `ffhn-<version>.tar.gz`
- `ffhn-aarch64-apple-darwin`
- `ffhn-aarch64-apple-darwin.sha256`
- `ffhn-x86_64-apple-darwin`
- `ffhn-x86_64-apple-darwin.sha256`
- `ffhn-x86_64-unknown-linux-musl`
- `ffhn-x86_64-unknown-linux-musl.sha256`
- `ffhn-x86_64-pc-windows-msvc.exe`
- `ffhn-x86_64-pc-windows-msvc.exe.sha256`

Release choreography lives in [docs/release-protocol.md](docs/release-protocol.md). Packaging mechanics live in [docs/operations.md](docs/operations.md).

If you are working directly from the repository without installing the binary, replace `ffhn` in the examples below with `cargo run -p ffhn-cli --`.

## Five-Minute Start

FFHN expects one directory per target under a watch root. The default watch root is `./watchlist`.

The checked-in `watchlist/demo` directory is a starter config, not a frozen sample state. Live runs create local runtime artifacts such as `state.json`, `last_run.json`, and `snapshots/` under that directory, and both `run` and `status` may create `lock/` on first use for locking. Those generated artifacts are ignored by Git.

Use the checked-in demo target:

```bash
ffhn run --target demo
```

Inspect the current status for that same target:

```bash
ffhn status --target demo
```

Run the full watch root in parallel:

```bash
ffhn run --all --jobs 4
```

Inspect everything without mutating snapshots or run reports:

```bash
ffhn run --target demo --dry-run
```

## Mental Model

One live FFHN run is intentionally simple:

1. load and validate `target.toml`
2. acquire the exclusive run lock
3. load stored state
4. fetch the URL or read the local file
5. ask HTMLCut for the configured slice
6. canonicalize the compare text and classify the outcome
7. persist state and snapshots when applicable
8. attempt best-effort notifications
9. attempt the final `last_run.json` write

Dry-run uses the same validation, fetch, extraction, and comparison path, but it takes the shared run lock before reading state, then skips snapshot writes, `state.json` writes, `last_run.json` writes, and notifications.

## Minimal Target

Every target lives at `<watch_root>/<target_id>/target.toml`.

- [watchlist/demo/target.toml](watchlist/demo/target.toml): minimal HTTP target
- [examples/file-target-with-notifications.toml](examples/file-target-with-notifications.toml): file-backed target with `follow_redirects = false`, history retention, and notification hooks

Those checked-in example targets are validated by the test suite against the current `ffhn.target` contract, so they are the canonical public examples in this repository. The full target contract, defaults, and validation rules live in [docs/targets.md](docs/targets.md).

## What FFHN Persists

For each target, FFHN uses a durable directory layout like this:

```text
<watch_root>/
  <target_id>/
    target.toml
    state.json
    last_run.json
    lock/
      run.lock
    snapshots/
      current/
        canonical.txt
        outer.html
        extraction.json
      history/
        <snapshot_key>/
          canonical.txt
          outer.html
          extraction.json
```

In practice:

- `state.json` stores the latest durable state summary.
- `last_run.json` stores the most recent live run report that FFHN successfully wrote after notification delivery results were appended.
- `lock/run.lock` backs shared and exclusive locking and is created lazily by valid live runs, valid dry-runs, and valid status reads.
- `snapshots/current` holds the active baseline.
- `snapshots/history` holds older successful baselines, pruned by `storage.history_limit`.

The durable filesystem contract is documented in [docs/contracts.md](docs/contracts.md). Report semantics live in [docs/reports.md](docs/reports.md).

## How Runs Behave

Live runs validate configuration, acquire the exclusive lock, load state, fetch or read the source, execute the HTMLCut plan, compare canonicalized content, persist results, attempt best-effort notifications, and then attempt the final `last_run.json` write.

Invalid target documents short-circuit before lock or state access, so FFHN reports `config_invalid` without creating runtime artifacts for a broken target.

Dry-run keeps the same validation, fetch, extraction, and comparison pipeline, but it acquires the shared run lock first so it reads a stable on-disk view while live runs are persisting. It still skips snapshot writes, `state.json`, `last_run.json`, and notifications.

`run --all` discovers immediate subdirectories under the watch root, sorts them lexicographically, includes valid enabled targets, skips valid disabled targets, and still keeps invalid target directories in the batch so their failures surface instead of disappearing silently. If walking the watch root fails partway through, FFHN exits fatally instead of silently dropping the broken entry.

## CLI Contract At A Glance

<!-- contract:cli-summary:start -->
| Command | Stdout document | Notes |
| --- | --- | --- |
| `ffhn run --target <id>` | `ffhn.run_report` | single-target execution |
| `ffhn run --target <a> --target <b>` | `ffhn.batch_run_report` | explicit multi-target batch |
| `ffhn run --all` | `ffhn.batch_run_report` | watch-root discovery |
| `ffhn status --target <id>` | `ffhn.status_report` | status inspection; valid targets may create `lock/run.lock` |
<!-- contract:cli-summary:end -->

The CLI writes exactly one JSON document to stdout whenever it can produce a structured result. Stderr is reserved for fatal process-level failures and CLI-usage errors. The exact exit-code rules are documented in [docs/cli.md](docs/cli.md).

## Repository Map

```text
crates/
  ffhn-core/
  ffhn-cli/
xtask/
fuzz/
docs/
examples/
scripts/
watchlist/
```

- `ffhn-core`: monitoring engine, contracts, persistence, notifications, and batch execution
- `ffhn-cli`: argument parsing, watch-root discovery, JSON rendering, and exit-code mapping
- `xtask`: maintainer automation such as `check`, coverage, and semver baseline refresh
- `fuzz`: standalone `cargo-fuzz` package and checked-in seed corpora

## Documentation

Start with [docs/README.md](docs/README.md).

The most important pages are:

- [docs/architecture.md](docs/architecture.md): crate boundaries, runtime ownership, and the FFHN versus HTMLCut split
- [docs/cli.md](docs/cli.md): `run`, `status`, exit codes, stdout/stderr rules, and `--all` discovery
- [docs/targets.md](docs/targets.md): `ffhn.target` schema, defaults, validation, storage, and notifications
- [docs/reports.md](docs/reports.md): `ffhn.state`, run reports, batch reports, status reports, and reason codes
- [docs/quality-gates.md](docs/quality-gates.md): what `./check.sh` and `cargo xtask` actually enforce
- [docs/release-protocol.md](docs/release-protocol.md): maintained public-release procedure through GitHub CLI
- [docs/versioning-policy.md](docs/versioning-policy.md): version-source, contract, frozen-interop, and semver-baseline policy
- [CONTRIBUTING.md](CONTRIBUTING.md): contributor workflow, test expectations, and docs hygiene
- [fuzz/README.md](fuzz/README.md): manual fuzz inventory and maintained seed-smoke commands

## Maintainer Gate

The maintained local gate is:

```bash
./check.sh
```

Equivalent direct commands remain available through:

```bash
cargo xtask check
cargo xtask coverage
cargo xtask refresh-semver-baseline
```
