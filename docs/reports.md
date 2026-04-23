---
afad: "3.5"
version: "3.0.0"
domain: REPORTS
updated: "2026-04-23"
route:
  keywords: [reports, ffhn.state, ffhn.extraction_record, ffhn.run_report, ffhn.notification_payload, ffhn.batch_run_report, ffhn.status_report, process errors, snapshot references, reason codes]
  questions: ["what do ffhn reports mean?", "what is stored in ffhn.state?", "what is stored in ffhn.extraction_record?", "what is ffhn.notification_payload?", "which reason codes can ffhn emit?"]
---

# State, Report, And Snapshot Documents

FFHN currently emits, persists, or writes six machine-readable document families:

1. `ffhn.state`
2. `ffhn.extraction_record`
3. `ffhn.run_report`
4. `ffhn.notification_payload`
5. `ffhn.batch_run_report`
6. `ffhn.status_report`

`ffhn.extraction_record` is persisted inside snapshot artifacts, but it is not emitted directly by the CLI.

## `ffhn.state`

`state.json` records the most recent durable target state.

Important fields:

1. `target_id`
2. `state_phase = "never_succeeded" | "has_baseline"`
3. `last_run_at`
4. `last_run_outcome`
5. `last_reason_code`
6. `current_snapshot`
7. `snapshot_history`

Field semantics:

1. `last_run_at`, `last_run_outcome`, and `last_reason_code` describe the most recent attempted live run that reached state persistence, not only the most recent success
2. `current_snapshot` and `snapshot_history` are `SnapshotReference` objects that carry `slot`, digests, relative artifact paths, and `captured_at`
3. `snapshot_history` is ordered newest first

Key invariants:

1. `never_succeeded` forbids `current_snapshot` and `snapshot_history`
2. `has_baseline` requires `current_snapshot`
3. `current_snapshot.slot` must be `current`
4. `snapshot_history[].slot` must be `history`

## `ffhn.extraction_record`

Every persisted snapshot directory contains one `extraction.json` file with `ffhn.extraction_record`.

Important fields:

1. `interop_profile = "htmlcut-v1"`
2. `htmlcut_plan_digest_sha256`
3. `htmlcut_result_digest_sha256`
4. `comparison_input_sha256`
5. `outer_html_sha256`
6. `strategy_kind`
7. `selection_mode`
8. `output_kind`
9. `candidate_count`
10. `selected_candidate_index`
11. `match_metadata`
12. `warning_codes`
13. `created_at`

Interpretation:

1. `comparison_input_sha256` is the digest of HTMLCut's `comparison_input_text` after FFHN has normalized line endings to LF
2. `match_metadata` is the stable selected-match metadata object copied from HTMLCut
3. `warning_codes` includes warning-level diagnostics only
4. the record is paired one-for-one with the sibling `canonical.txt` and `outer.html` artifacts in the same snapshot directory

Key invariants:

1. `interop_profile` must match FFHN's frozen `htmlcut-v1` expectation
2. `candidate_count` and `selected_candidate_index` must be positive, and the selected index must stay within the candidate count
3. `match_metadata` must stay a JSON object

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
2. `run_started_at`
3. `run_finished_at`
4. `run_mode = "live" | "dry_run"`
5. `run_outcome`
6. `reason_code`
7. `failure_class`
8. `target_status_after_run`
9. `compare_basis`
10. `previous_compare_digest_sha256`
11. `current_compare_digest_sha256`
12. `state_phase_before_run`
13. `state_phase_after_run`
14. `fetch`
15. `extraction`
16. `compare`
17. `change`
18. `persist`
19. `notifications`

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
7. dry-run reports must have `persist.wrote_state = false`, `persist.wrote_last_run = false`, no `persist.error`, and no notification deliveries
8. `run_finished_at` is stamped after notification delivery and before the final `last_run.json` write attempt
9. `run_report_digest_sha256` is the stable SHA-256 digest of the report body with that field omitted

Failed reports may still carry the earlier stage sections that FFHN completed before the final outcome became a failure. For example, `persist_error` can still include fetch, extraction, compare, and change data from the already-computed run body.

Field semantics:

1. `compare_basis` is currently the fixed vocabulary value `canonical_text_sha256`
2. `previous_compare_digest_sha256` is present only when FFHN had a prior valid baseline digest
3. `change` is omitted when FFHN never reached a point where it could classify the compare result
4. `notifications` is omitted entirely from serialized JSON when no deliveries were attempted

### `persist`

`persist` tells you what the current run actually wrote.

Fields:

1. `duration_ms`
2. `wrote_state`
3. `wrote_last_run`
4. optional `error`

`persist.error` carries structured detail for the persist substep that failed.

Interpretation:

1. `reason_code = persist_error` means an earlier live persist step failed and FFHN downgraded the run outcome to a transient structured failure
2. `persist.error` may also appear on otherwise successful live outcomes when the final `last_run.json` write failed after FFHN already had a valid run body
3. `wrote_state` and `wrote_last_run` tell you which durable writes actually succeeded

### Structured process errors

FFHN uses one stable `ProcessErrorDetail` shape for `persist.error` and batch `fatal_error`.

Fields:

1. `kind`
2. `message`
3. optional `path`

`path` is present only when FFHN can associate the process-level failure with one concrete filesystem path.

The current `kind` vocabulary is:

1. `io`
2. `json`
3. `toml`
4. `url`
5. `time_format`
6. `time_parse`
7. `htmlcut`

### `notifications`

`notifications` records best-effort delivery attempts, not a guarantee that external side effects completed as intended.

The `notifications` field is omitted entirely from serialized JSON when no deliveries were attempted.

Each entry carries:

1. the configured `hook_name`
2. the triggering `event`
3. `delivered`
4. `timed_out`
5. `exit_code`
6. `duration_ms`
7. optional error text

Interpretation:

1. failed notification delivery does not rewrite `run_outcome`
2. failed notification delivery does make the CLI exit with code `1`
3. `error` may include captured hook stderr text when FFHN could read it

## `ffhn.notification_payload`

Notification hooks receive `ffhn.notification_payload` on stdin.

Important fields:

1. `hook_name`
2. `event`
3. `delivery_started_at`
4. `run_report`

Key invariants:

1. `run_report` is always a live pre-delivery snapshot
2. `run_report.notifications` is always empty because delivery results do not exist yet
3. `run_report.persist.wrote_last_run` is always `false` because the final `last_run.json` write happens after hook delivery
4. `run_report.persist.error` may already be present when an earlier live persist step failed before FFHN started delivering notifications

## `ffhn.batch_run_report`

Multi-target `run` emits `ffhn.batch_run_report`.

Important fields:

1. `run_mode`
2. `watch_root`
3. `requested_targets`
4. `run_started_at`
5. `run_finished_at`
6. `max_concurrency`
7. `entries`
8. `outcome_counts`

Per-entry rules:

1. each entry must carry exactly one of `run_report` or `fatal_error`
2. `run_report.target_id` must match the entry `target_id`
3. `entries` align one-for-one with `requested_targets`
4. `requested_targets` must be unique
5. `max_concurrency` must be positive

`fatal_error` is reserved for process-level failures where FFHN could not emit a structured per-target `ffhn.run_report`, and it is itself a structured FFHN-owned error object.

That structured `fatal_error` object uses the same `ProcessErrorDetail` shape documented above.

`outcome_counts.persist_error` is a secondary aggregate over any live persist issue in the batch, whether that issue changed the per-target `run_outcome` to `failed_transient` with `reason_code = persist_error` or only prevented the final `last_run.json` write on an otherwise successful run.

Notification delivery failures are not counted in `outcome_counts`. Batch callers need to inspect each entry's `run_report.notifications` array when delivery success matters.

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

Live `changed` runs rotate the previous current snapshot into history, then prune history down to `storage.history_limit - 1` older entries. FFHN stages those snapshot mutations so a later `state.json` write failure rolls the current baseline back instead of poisoning the previously valid state. Live `unchanged` runs keep the existing snapshot references intact.
