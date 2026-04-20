---
afad: "3.5"
version: "2.0.0"
domain: ARCHITECTURE
updated: "2026-04-20"
route:
  keywords: [architecture, ffhn-core, ffhn-cli, xtask, fuzz package, htmlcut boundary, watch root]
  questions: ["what are the ffhn repository boundaries?", "what does ffhn-core own versus ffhn-cli?", "how does ffhn interact with htmlcut?"]
---

# Architecture

`ffhn` is a Rust workspace with one monitoring engine, one CLI renderer, one maintainer-tooling crate, and one standalone fuzz package.

## Workspace Layout

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
  demo/
```

## Ownership Boundaries

`ffhn-core` is the product. It owns:

1. `ffhn.target` validation and durable `target_id` rules
2. HTTP and local-file fetching
3. mapping FFHN selection config into `htmlcut-v1`
4. compare-time canonicalization and digest decisions
5. live state persistence, current snapshots, and retained history snapshots
6. `ffhn.state`, `ffhn.run_report`, `ffhn.batch_run_report`, and `ffhn.status_report`
7. best-effort notification hook delivery

`ffhn-cli` is a thin process adapter. It owns:

1. rendering the core-owned CLI operation contract into argument parsing and help text
2. watch-root discovery for `run --all`
3. choosing single-target versus batch execution
4. emitting exactly one JSON document on stdout
5. mapping outcomes into process exit codes

`xtask` owns maintainer automation:

1. `cargo xtask check`
2. `cargo xtask coverage`
3. `cargo xtask refresh-semver-baseline`

`fuzz/` is a separate `cargo-fuzz` package. It is not part of the normal workspace members and is compile-smoked by the maintainer gate through its own manifest.

## FFHN Versus HTMLCut

FFHN and HTMLCut have a hard boundary.

FFHN owns source acquisition and persistence. HTMLCut owns extraction execution.

The current boundary is:

1. FFHN fetches or reads the source and decodes it into HTML text
2. FFHN builds an `htmlcut.plan` through `htmlcut_core::interop::v1`
3. HTMLCut returns `htmlcut.result` or `htmlcut.error`
4. FFHN validates the interop result, compares content, persists artifacts, and emits reports

FFHN does not delegate fetching to HTMLCut.

## Runtime Shape

Live `run` uses this pipeline:

1. validate `target.toml`
2. acquire the exclusive run lock
3. load `state.json`
4. fetch or read the configured source
5. execute the HTMLCut plan
6. canonicalize comparison text
7. classify the run outcome
8. persist `state.json` and snapshot artifacts when applicable
9. attempt configured notification hooks
10. attempt the final `last_run.json` write

Dry-run keeps the same validation, fetch, extraction, and comparison flow, but it acquires the shared run lock first and then intentionally skips:

1. the exclusive run lock
2. all snapshot writes
3. all `state.json` writes
4. all `last_run.json` writes
5. all notification delivery

Live runs treat invalid stored state or snapshot-integrity drift as structured permanent failures. Dry-run continues through those cases because it is explicitly a non-persistent inspection path, but the shared lock still ensures it reads a stable target directory while live persistence is in flight.

## Batch Execution

Batch execution is part of the core, not the CLI. `run_batch`:

1. accepts an explicit unique target list and run mode
2. requires a positive `max_concurrency`
3. runs targets in chunks of `max_concurrency`
4. preserves the requested target order in the final `ffhn.batch_run_report`
5. records per-target fatal errors separately when a structured `ffhn.run_report` could not be emitted

The CLI does not implement its own monitoring semantics for multi-target runs. It renders the core batch report.
