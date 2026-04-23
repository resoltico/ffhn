---
afad: "3.5"
version: "3.0.1"
domain: RUN_REPORTS
updated: "2026-04-23"
route:
  keywords: [run report, batch run report, process errors, reason codes, notification delivery, persist error]
  questions: ["what does ffhn.run_report mean?", "what does ffhn.batch_run_report mean?", "which reason codes can ffhn emit?", "what is the shared ffhn process-error shape?"]
---

# Run And Batch Reports

This page covers the run-oriented FFHN documents:

1. `ffhn.run_report`
2. `ffhn.batch_run_report`

State, snapshot, extraction-record, status, and notification-payload documents stay in [reports.md](reports.md).

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

### `fetch`

`fetch` captures only the fetch-stage detail FFHN actually reached before extraction.

Fields:

1. `engine`
2. `final_url`
3. `http_status`
4. `content_type`
5. `bytes_read`
6. `duration_ms`

Interpretation:

1. `final_url` is the resolved HTTP URL after redirects, or a `file://` URL for file targets when FFHN can derive one from the configured absolute path
2. file-target fetch sections keep `http_status` and `content_type` null
3. `bytes_read` stays null when failure happened before FFHN accepted any body or file bytes
4. HTTP responses with no `Content-Type` header are still decoded as UTF-8 by default; `fetch_unsupported_content_type` only applies when a present media type is not HTML/XHTML
5. `engine` preserves the configured contract value, so reports may still say `browser` even though the current Rust rewrite uses the HTTP transport backend for that alias

### `change`

`change` summarizes the compare result over LF-normalized canonical text.

Fields:

1. `kind`
2. `previous_text_bytes`
3. `current_text_bytes`
4. `previous_line_count`
5. `current_line_count`
6. `common_prefix_lines`
7. `common_suffix_lines`
8. optional `changed_region`

Interpretation:

1. `kind = "initialized" | "changed" | "unchanged"` is derived from the compare digest result
2. `previous_*` fields stay null on `initialized` runs because there was no prior baseline text
3. `common_prefix_lines` and `common_suffix_lines` count unchanged surrounding lines in the previous and current canonical texts
4. `changed_region` appears only for `initialized` and `changed`, and its line numbers are one-based
5. `previous_excerpt` and `current_excerpt` keep only the first four lines of the changed region and are truncated to a short digestible fragment when necessary
6. excerpt digests appear only when the corresponding excerpt string exists

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

Interpretation:

1. `watch_root` preserves the path string FFHN was asked to use; it is not rewritten to a canonical absolute path
2. `requested_targets` preserves caller order, and `entries` use that same stable order even though workers execute concurrently
3. `run_finished_at` is stamped after the worker pool has converged and the aggregate report is assembled

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
| `fetch_unsupported_content_type` | permanent | fetched response declared a non-HTML/XHTML media type |
| `fetch_decode_error` | permanent | fetched HTTP body or local file bytes could not be decoded |
| `extraction_plan_invalid` | permanent | HTMLCut plan contract invalid |
| `extraction_no_match` | permanent | HTMLCut matched nothing |
| `extraction_ambiguous_match` | permanent | HTMLCut found multiple exact matches |
| `extraction_internal_error` | permanent | HTMLCut returned an internal extraction failure |
| `canonicalization_error` | permanent | FFHN canonicalization failed |
| `compare_error` | permanent | compare stage failed |
| `persist_error` | transient | live persistence failed after FFHN already had a structured run body |
| `integrity_mismatch` | permanent | retained snapshot artifacts did not match recorded digests |
