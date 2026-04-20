---
afad: "3.5"
version: "2.0.0"
domain: CONTRACTS
updated: "2026-04-20"
route:
  keywords: [contracts, schema versions, htmlcut boundary, durable layout, extraction record, snapshot layout]
  questions: ["what schemas does ffhn freeze today?", "what does ffhn own versus htmlcut?", "what is the persisted watch-root layout?"]
---

# Durable Contracts

This page documents the frozen, current-state contract surfaces that FFHN owns.

## FFHN Versus HTMLCut

FFHN owns:

1. target configuration
2. source acquisition from HTTP or local files
3. compare-time canonicalization
4. state transitions and persistence
5. run, batch, and status reports
6. notification configuration and delivery attempts

HTMLCut owns:

1. `htmlcut-v1`
2. `htmlcut.plan`
3. `htmlcut.result`
4. `htmlcut.error`
5. extraction execution and diagnostics

FFHN passes decoded HTML text into `htmlcut_core::interop::v1::HtmlInput`. HTMLCut does not fetch URLs for FFHN.

## Frozen Schema Inventory

All current FFHN schema documents require exact `schema_name` and `schema_version` values.

| Schema | Version | Notes |
| --- | ---: | --- |
| `ffhn.target` | 1 | loaded from `target.toml` |
| `ffhn.extraction_record` | 1 | persisted inside snapshot artifacts |
| `ffhn.state` | 1 | stored as `state.json` |
| `ffhn.run_report` | 1 | emitted for single-target `run` |
| `ffhn.batch_run_report` | 1 | emitted for multi-target `run` |
| `ffhn.status_report` | 1 | emitted for `status` |

The current HTMLCut interop profile is `htmlcut-v1`.

## Durable Watch-Root Layout

FFHN uses one watch root containing one directory per `target_id`.

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

The durable artifact meanings are:

- `target.toml`: `ffhn.target`
- `state.json`: `ffhn.state`
- `last_run.json`: the most recent live `ffhn.run_report` that FFHN successfully wrote
- `lock/run.lock`: the shared/exclusive lock anchor, created lazily for valid live `run`, valid dry-run `run`, and valid `status` execution
- `snapshots/current/canonical.txt`: compare-time canonical text
- `snapshots/current/outer.html`: `selected_match.outer_html`
- `snapshots/current/extraction.json`: persisted `ffhn.extraction_record`

History snapshots reuse the same three artifact names under `snapshots/history/<snapshot_key>/`.

In the repository itself, `watchlist/demo` is maintained as starter target configuration. The runtime-generated files in this layout are local state that FFHN creates when you run the target.

## Snapshot Key Shape

FFHN derives each history directory key from:

1. the snapshot capture timestamp with ASCII-alphanumeric characters compacted together
2. the first 12 hex characters of the canonical-text SHA-256 digest

The exact formatting is intentionally an implementation detail, but the directory is stable enough to be referenced from `ffhn.state`.

## State And Snapshot Invariants

FFHN enforces these persistence rules:

1. `state_phase = never_succeeded` forbids `current_snapshot` and `snapshot_history`
2. `state_phase = has_baseline` requires `current_snapshot`
3. `current_snapshot.slot` must be `current`
4. `snapshot_history[].slot` must be `history`
5. every referenced snapshot artifact must exist and match its recorded digests

If those invariants fail during a live run or `status`, FFHN reports invalid state or integrity mismatch instead of silently repairing it.
