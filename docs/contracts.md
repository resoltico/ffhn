---
afad: "4.0"
domain: CONTRACTS
updated: "2026-05-17"
route:
  keywords: [contracts, schema versions, htmlcut boundary, durable layout, extraction record, notification payload, process errors, snapshot layout]
  questions: ["what schemas does ffhn freeze today?", "what does ffhn own versus htmlcut?", "what is the persisted watch root layout?", "where is ffhn's structured process-error shape documented?"]
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

FFHN passes decoded HTML text into `htmlcut_core::interop::v1::HtmlInput`, receives one interop result, and then translates that result into FFHN-owned extraction evidence and persisted artifacts. HTMLCut does not fetch URLs for FFHN.

## Frozen Schema Inventory

All current FFHN schema documents require exact `schema_name` and `schema_version` values.

| Schema | Version | Notes |
| --- | ---: | --- |
| `ffhn.target` | 4 | loaded from `target.toml` |
| `ffhn.extraction_record` | 4 | persisted inside snapshot artifacts |
| `ffhn.state` | 4 | stored as `state.json` |
| `ffhn.run_report` | 4 | emitted for single-target `run` |
| `ffhn.last_run_snapshot` | 2 | stored as `last_run.json` |
| `ffhn.notification_payload` | 4 | written to notification route stdin |
| `ffhn.batch_run_report` | 4 | emitted for multi-target `run` |
| `ffhn.status_report` | 5 | emitted for `status` |

Embedded field vocabularies and stable subobjects inside those schemas are part of the same public
contract surface. That includes `RunResult.kind`, `RunFailureCause`, `StatusSummary.kind`,
notification-delivery outcome values, and the structured process-error detail used by failed
`persist.state_commit` / `persist.last_run_write` entries, status invalidation details, and batch
`fatal_error`.

## Durable Watch Root Layout

FFHN uses one watch root containing one directory per `target_id`.

For discovery-based `run --all`, only immediate subdirectories that contain a `target.toml` path are treated as target candidates.

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
        compare.txt
        outer.html
        extraction.json
      history/
        <snapshot_key>/
          compare.txt
          outer.html
          extraction.json
```

The durable artifact meanings are:

- `target.toml`: `ffhn.target`
- `state.json`: `ffhn.state`
- `last_run.json`: `ffhn.last_run_snapshot`, which wraps the live post-notification `ffhn.run_report` snapshot FFHN successfully published; the nested report keeps `persist.last_run_write.status = not_attempted`, and the file may lag the newest live stdout report if a later final publication attempt fails
- `lock/run.lock`: the shared/exclusive lock anchor, created lazily for valid live `run`, valid dry-run `run`, and valid `status` execution
- `snapshots/current/compare.txt`: the final compare value after FFHN applies compare-basis projection, URL rewriting, text whitespace shaping when applicable, and configured canonicalizers
- `snapshots/current/outer.html`: the FFHN-owned persisted outer-HTML artifact derived from the selected interop match
- `snapshots/current/extraction.json`: persisted `ffhn.extraction_record`

History snapshots reuse the same three artifact names under `snapshots/history/<snapshot_key>/`.

In the repository itself, `watchlist/demo` is maintained as starter target configuration. The runtime-generated files in this layout are local state that FFHN creates when you run the target.

## Snapshot Key Shape

FFHN derives each history directory key from:

1. the snapshot capture timestamp with ASCII-alphanumeric characters compacted together
2. the first 12 hex characters of the compare-value SHA-256 digest

The exact formatting is intentionally an implementation detail, but the derived directory name becomes part of the relative history paths persisted in `ffhn.state`.

## State And Snapshot Invariants

FFHN enforces these persistence rules:

1. `baseline.kind = "pending"` forbids current or historical snapshot references
2. `baseline.kind = "ready"` requires one `current_snapshot`
3. successful `last_run` summaries require `baseline.kind = "ready"`
4. `baseline.kind = "ready"` requires `last_run`
5. `current_snapshot.slot` must be `current`
6. `snapshot_history[].slot` must be `history`
7. every referenced snapshot artifact must exist and match its recorded digests

If those invariants fail during a live run or `status`, FFHN reports invalid state or integrity mismatch instead of silently repairing it.
