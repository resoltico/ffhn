---
afad: "3.5"
version: "2.0.1"
domain: REPORTS
updated: "2026-04-20"
route:
  keywords: [reports, ffhn.state, ffhn.run_report, ffhn.batch_run_report, ffhn.status_report, reason codes]
  questions: ["what do ffhn reports mean?", "what is stored in ffhn.state?", "which reason codes can ffhn emit?"]
---

# State And Report Documents

FFHN currently emits or persists four top-level machine-readable document families:

1. `ffhn.state`
2. `ffhn.run_report`
3. `ffhn.batch_run_report`
4. `ffhn.status_report`

`ffhn.extraction_record` is also persisted inside snapshot artifacts, but it is not emitted directly by the CLI.

## `ffhn.state`

`state.json` records the most recent known target state.

Important fields:

1. `state_phase = "never_succeeded" | "has_baseline"`
2. `last_run_at`
3. `last_run_outcome`
4. `last_reason_code`
5. `current_snapshot`
6. `snapshot_history`

Key invariants:

1. `never_succeeded` forbids `current_snapshot` and `snapshot_history`
2. `has_baseline` requires `current_snapshot`
3. `snapshot_history` is ordered newest first
4. `current_snapshot.slot` must be `current`
5. `snapshot_history[].slot` must be `history`

## `ffhn.status_report`

`status` emits `ffhn.status_report`.

Important fields:

1. `target_status = "pending" | "ready" | "invalid"`
2. `reason_code`
3. `state_phase`
4. `artifacts.current_valid`
5. `artifacts.previous_valid`
6. `current_snapshot`
7. `snapshot_history`

Interpretation:

1. `pending` means the target is valid but no baseline is ready yet
2. `ready` means the target is valid and has a current baseline snapshot
3. `invalid` means target validation failed, stored state validation failed, or retained artifacts failed integrity
4. `state_invalid` and `integrity_mismatch` keep the parsed `state_phase` when FFHN can still recover it from `state.json`
5. unreadable `state.json` falls back to `state_phase = "never_succeeded"` because FFHN cannot safely trust any persisted phase

Special case:

1. `state_phase = null` is only valid when `reason_code = config_invalid`

`current_snapshot` and `snapshot_history` carry only digest summaries, not the full artifact bodies.

## `ffhn.run_report`

Single-target `run` emits `ffhn.run_report`.

Important top-level fields:

1. `run_report_digest_sha256`
2. `run_mode = "live" | "dry_run"`
3. `run_outcome`
4. `reason_code`
5. `failure_class`
6. `target_status_after_run`
7. `previous_compare_digest_sha256`
8. `current_compare_digest_sha256`
9. `fetch`
10. `extraction`
11. `compare`
12. `change`
13. `persist`
14. `notifications`

Successful outcomes are:

1. `initialized`
2. `changed`
3. `unchanged`

Structured non-success outcomes are:

1. `failed_transient`
2. `failed_permanent`
3. `skipped_disabled`

Key invariants:

1. successful outcomes require `reason_code = ok`
2. successful outcomes do not carry `failure_class`
3. successful outcomes require `current_compare_digest_sha256`
4. successful outcomes require `fetch`, `extraction`, and `compare`
5. `skipped_disabled` requires `reason_code = disabled`
6. failed or skipped outcomes must not carry `current_compare_digest_sha256`
7. dry-run reports must have `persist.wrote_state = false`, `persist.wrote_last_run = false`, and `notifications = []`
8. `run_report_digest_sha256` is the stable SHA-256 digest of the report body with that field omitted

Failed reports may still carry the earlier stage sections that FFHN completed before the final outcome became a failure. For example, `persist_error` can still include fetch, extraction, compare, and change data from the already-computed run body.

### `persist`

`persist` tells you what the current run actually wrote.

Fields:

1. `duration_ms`
2. `wrote_state`
3. `wrote_last_run`

### `notifications`

`notifications` records best-effort delivery attempts, not a guarantee that external side effects completed as intended.

Each entry carries:

1. the configured `hook_name`
2. the triggering `event`
3. `delivered`
4. `timed_out`
5. `exit_code`
6. `duration_ms`
7. optional error text

## `ffhn.batch_run_report`

Multi-target `run` emits `ffhn.batch_run_report`.

Important fields:

1. `run_mode`
2. `watch_root`
3. `requested_targets`
4. `max_concurrency`
5. `entries`
6. `outcome_counts`

Per-entry rules:

1. each entry must carry exactly one of `run_report` or `fatal_error`
2. `run_report.target_id` must match the entry `target_id`
3. `entries` align one-for-one with `requested_targets`
4. `requested_targets` must be unique
5. `max_concurrency` must be positive

`fatal_error` is reserved for process-level failures where FFHN could not emit a structured per-target `ffhn.run_report`.

## Reason Codes

The current FFHN reason-code vocabulary is:

| Reason code | Failure class | Meaning |
| --- | --- | --- |
| `ok` | none | successful or valid state |
| `disabled` | none | target disabled |
| `config_invalid` | permanent | target document invalid |
| `state_invalid` | permanent | stored state invalid |
| `lock_unavailable` | transient | live run lock could not be acquired |
| `fetch_http_client_error` | permanent | HTTP 3xx/4xx family failure |
| `fetch_http_server_error` | transient | HTTP 5xx family failure |
| `fetch_source_error` | permanent | local file source could not be read |
| `fetch_network_error` | transient | HTTP transport failure before a valid body existed |
| `fetch_timeout` | transient | fetch timeout |
| `fetch_too_large` | permanent | body exceeded configured size limit |
| `fetch_unsupported_content_type` | permanent | fetched response was not HTML/XHTML |
| `fetch_decode_error` | permanent | fetched HTTP body or local file bytes could not be decoded |
| `extraction_plan_invalid` | permanent | HTMLCut plan contract invalid |
| `extraction_no_match` | permanent | HTMLCut matched nothing |
| `extraction_ambiguous_match` | permanent | HTMLCut found multiple exact matches |
| `extraction_internal_error` | permanent | HTMLCut returned an internal extraction failure |
| `canonicalization_error` | permanent | FFHN canonicalization failed |
| `compare_error` | permanent | compare stage failed |
| `persist_error` | transient | live persistence failed after FFHN already had a structured run body |
| `integrity_mismatch` | permanent | retained snapshot artifacts did not match recorded digests |

## Snapshot Artifact Meanings

Snapshot references point at three persisted files:

1. `canonical.txt`: compare-time canonical text
2. `outer.html`: selected outer HTML from HTMLCut
3. `extraction.json`: persisted `ffhn.extraction_record`

Live `changed` runs rotate the previous current snapshot into history, then prune history down to `storage.history_limit - 1` older entries. Live `unchanged` runs keep the existing snapshot references intact.
