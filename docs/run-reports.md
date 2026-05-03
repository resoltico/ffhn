---
afad: "4.0"
domain: RUN_REPORTS
updated: "2026-04-30"
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
8. optional `error_detail`
9. `target_status_after_run`
10. `compare_basis`
11. `previous_compare_digest_sha256`
12. `current_compare_digest_sha256`
13. `state_phase_before_run`
14. `state_phase_after_run`
15. `fetch`
16. `extraction`
17. `compare`
18. `change`
19. `persist`
20. `notifications`

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
6. `failed_transient` and `failed_permanent` require structured top-level `error_detail`
7. successful and skipped outcomes must not carry `error_detail`
8. failed or skipped outcomes must not carry `current_compare_digest_sha256` unless `reason_code = persist_error`
9. dry-run reports must have `persist.state_write.status = not_attempted`, `persist.last_run_write.status = not_attempted`, and no notification deliveries
10. `run_finished_at` is stamped after notification delivery and before the final `last_run.json` write attempt
11. `run_report_digest_sha256` is the stable SHA-256 digest of the report body with that field omitted

Failed reports may still carry the earlier stage sections that FFHN completed before the final outcome became a failure. For example, `persist_error` can still include fetch, extraction, compare, and change data from the already-computed run body.

Field semantics:

1. `compare_basis` is currently the fixed vocabulary value `canonical_text_sha256`
2. `previous_compare_digest_sha256` is present only when FFHN had a prior valid baseline digest
3. top-level `error_detail` carries the primary structured failure detail for `failed_transient` and `failed_permanent` outcomes
4. `change` is omitted when FFHN never reached a point where it could classify the compare result
5. `notifications` is omitted entirely from serialized JSON when no deliveries were attempted

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
5. `engine` is `http` for network targets and `file` for file targets

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
2. `state_write`
3. `last_run_write`

Each write result carries `status = "not_attempted" | "written" | "failed"`. Failed write
results also carry structured `error` detail.

Interpretation:

1. `reason_code = persist_error` means at least one live persist write failed and FFHN downgraded the run outcome to a transient structured failure
2. `state_write` and `last_run_write` track the two durable write paths independently
3. pre-delivery notification payloads always carry `last_run_write.status = not_attempted`

### Structured process errors

FFHN uses one stable `ProcessErrorDetail` shape for top-level `run_report.error_detail`,
failed persist-write entries, batch `fatal_error`, and invalid `status_report.error_detail`.

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
7. `contract`
8. `htmlcut_interop`
9. `internal`

Interpretation:

1. `toml` means the source TOML text itself could not be decoded
2. `contract` means FFHN decoded the input shape but rejected one of its durable contracts or invariants
3. `htmlcut_interop` is reserved for failures reported across FFHN's frozen HTMLCut boundary
4. `internal` means FFHN hit an internal invariant or orchestration failure that was not attributable to user input or HTMLCut

### `notifications`

`notifications` records best-effort delivery attempts, not a guarantee that external side effects completed as intended.

The `notifications` field is omitted entirely from serialized JSON when no deliveries were attempted.

Each entry carries:

1. the configured `hook_name`
2. `duration_ms`
3. `outcome`

Interpretation:

1. each delivery belongs to the parent report's `run_outcome`
2. `outcome.status = delivered` carries `exit_code = 0`
3. `outcome.status = failed` may also carry `exit_code` when the hook process exited normally
4. `outcome.status = timed_out` records timeout detail without an exit code
5. failed notification delivery does not rewrite `run_outcome`
6. failed notification delivery does make the CLI exit with code `1`
7. `outcome.error` may include captured hook stderr text when FFHN could read it

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
3. discovery-based batch reports may preserve raw directory labels that violate FFHN's durable `target_id` rules so the failing entry can be reported literally instead of being dropped or rewritten
4. `run_finished_at` is stamped after the worker pool has converged and the aggregate report is assembled

Per-entry rules:

1. each entry must carry exactly one of `run_report` or `fatal_error`
2. when `run_report` is present, `run_report.target_id` must match the entry `target_id`
3. `entries` align one-for-one with `requested_targets`
4. `requested_targets` must be unique
5. `max_concurrency` must be positive

`fatal_error` is reserved for process-level failures where FFHN could not emit a structured per-target `ffhn.run_report`, and it is itself a structured FFHN-owned error object.
Discovery-time invalid directory labels therefore surface as `fatal_error.kind = contract` entries whose `target_id` still matches the literal directory name FFHN was asked to inspect.

That structured `fatal_error` object uses the same `ProcessErrorDetail` shape documented above.

`outcome_counts.persist_error` is a secondary aggregate over any live persist issue in the batch.
Every such entry also carries `reason_code = persist_error`; the aggregate exists so callers can
read that slice without scanning every per-target report.

`outcome_counts.notification_failure` counts target entries whose `run_report.notifications` array contains at least one failed or timed-out delivery. The per-entry delivery objects remain the detailed source for hook-level diagnostics.

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
