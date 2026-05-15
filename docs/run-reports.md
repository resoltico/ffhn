---
afad: "4.0"
domain: RUN_REPORTS
updated: "2026-05-14"
route:
  keywords: [run report, batch run report, process errors, failure causes, notification delivery, persist error]
  questions: ["what does ffhn.run_report mean?", "what does ffhn.batch_run_report mean?", "which failure causes can ffhn emit?", "what is the shared ffhn process-error shape?"]
---

# Run And Batch Reports

This page covers FFHN's run-oriented documents:

1. `ffhn.run_report`
2. `ffhn.batch_run_report`

State, snapshot, extraction-record, status, and notification-payload documents live in
[reports.md](reports.md).

## `ffhn.run_report`

Single-target `run` emits `ffhn.run_report`.

Important top-level fields:

1. `run_report_digest_sha256`
2. `target_id`
3. optional `display_name`
4. `run_started_at`
5. `run_finished_at`
6. `run_mode = "live" | "dry_run"`
7. `result`
8. `compare_basis`
9. `previous_compare_digest_sha256`
10. `current_compare_digest_sha256`
11. `baseline_phase_before_run`
12. `baseline_phase_after_run`
13. optional `fetch`
14. optional `extraction`
15. optional `compare`
16. optional `change`
17. `persist`
18. optional `notifications`

### `result`

`result` is a tagged object with `kind`.

Successful result kinds are:

1. `initialized`
2. `changed`
3. `unchanged`

Non-success result kinds are:

1. `skipped_disabled`
2. `failed_transient`
3. `failed_permanent`

Failed results additionally carry:

1. `cause`
2. `error_detail`

Key invariants:

1. successful results require `current_compare_digest_sha256`
2. successful results require `fetch`, `extraction`, and `compare`
3. successful results require `change.kind` to align with `result.kind`
4. successful live runs require `baseline_phase_after_run = "has_baseline"`
5. `skipped_disabled` forbids `cause` and `error_detail`
6. failed results require both `cause` and `error_detail`
7. `result.cause = persist_error` is encoded as `result.kind = failed_transient`, and only a failed primary `persist.state_commit` may use that cause
8. dry-run reports require `baseline_phase_before_run = baseline_phase_after_run`, `persist.state_commit.status = not_attempted`, `persist.last_run_write.status = not_attempted`, and no notification deliveries
9. live `initialized` requires `baseline_phase_before_run = "never_succeeded"`, while live `changed` and `unchanged` require `baseline_phase_before_run = "has_baseline"`
10. valid disabled targets may emit `skipped_disabled` in both live and dry-run mode; live mode may persist and notify on that outcome, while dry-run leaves both persist paths at `not_attempted`
11. `run_finished_at` is stamped after notification delivery results are appended and before the final `last_run.json` write attempt
12. `run_report_digest_sha256` is the stable SHA-256 digest of the report body with that field omitted

Failed reports may carry earlier stage sections that FFHN already completed before the final result
became a failure. For example, a failed primary state commit may still include fetch, extraction,
compare, and change data from the already-computed run body.

Field semantics:

1. `compare_basis` is currently the fixed vocabulary value `canonical_text_sha256`
2. `previous_compare_digest_sha256` is present only when FFHN had a prior valid baseline digest
3. `current_compare_digest_sha256` is absent for early failures and disabled skips, but present for successful runs and for late failures that happened after compare completed
4. `display_name` is present only when FFHN could trust the target document enough to surface operator-facing identity; `config_invalid` and `target_unavailable` failures omit it
5. `baseline_phase_before_run` and `baseline_phase_after_run` record the durable baseline phase, not the richer `status.kind` view
6. failed runs may preserve the recovered baseline phase from the stable durable state view FFHN observed while sealing the report, even when the live run body never reached persistence
7. `result.cause = target_unavailable` means the explicit `target.toml` path was missing or unreadable after FFHN had already validated the watch root and request shape

### `fetch`

`fetch` captures only the fetch-stage detail FFHN reached before extraction.

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
4. HTTP responses with no `Content-Type` header are accepted and decoded as UTF-8 by default; `fetch_unsupported_content_type` applies only when a present media type is not HTML/XHTML
5. `engine` is `http` for network targets and `file` for file targets

### `extraction`

`extraction` summarizes the FFHN-owned view of one HTMLCut execution.

Fields:

1. `comparison_input_sha256`
2. `outer_html_sha256`
3. `selection_kind`
4. `selection_match`
5. `output_kind`
6. `candidate_count`
7. `selected_candidate_index`
8. `warning_codes`
9. `duration_ms`

Interpretation:

1. `comparison_input_sha256` is the digest of HTMLCut's selected `text_output` after FFHN line-ending normalization
2. `outer_html_sha256` is the digest of the selected `outer_html_output` after FFHN line-ending normalization
3. `selection_kind`, `selection_match`, and `output_kind` are FFHN-owned vocabularies
4. `warning_codes` includes warning-level diagnostics only
5. when URL rewriting was requested but no effective HTTP(S) base URL resolved, `warning_codes` includes `EFFECTIVE_BASE_URL_UNRESOLVED` instead of turning the run into a hard failure
6. deeper selection evidence lives in the persisted sibling `ffhn.extraction_record`

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
2. `previous_*` fields are omitted on `initialized` because there was no prior baseline text
3. `common_prefix_lines` and `common_suffix_lines` count equal surrounding lines in the previous and current canonical texts
4. `changed_region` appears only for `initialized` and `changed`, and its line numbers are one-based
5. `previous_excerpt` and `current_excerpt` keep only the first four lines of the changed region and may be omitted when FFHN chooses not to surface them
6. excerpt digests appear only when the corresponding excerpt string exists

### `persist`

`persist` tells you which durable write paths the run attempted.

Fields:

1. `state_commit_duration_ms`
2. `state_commit`
3. `last_run_write_duration_ms`
4. `last_run_write`

Each write result carries `status = "not_attempted" | "written" | "failed"`. Failed write
results also carry structured `error` detail.

Interpretation:

1. `state_commit` is the primary persistence transaction that publishes snapshots and `state.json`
2. only a failed `state_commit` rewrites `result` to `failed_transient` with `cause = persist_error`
3. `last_run_write` records the final `last_run.json` durability attempt after notification delivery and may fail without changing `result`; the file itself stores `ffhn.last_run_snapshot`, which wraps the pre-publication live `ffhn.run_report` snapshot
4. pre-delivery notification payloads always carry `last_run_write.status = not_attempted`

### Structured Process Errors

FFHN uses one stable `ProcessErrorDetail` shape for top-level run-result failures,
failed persist-write entries, batch `fatal_error`, and invalid `status_report` variants.

Fields:

1. `kind`
2. `message`
3. optional `path`

`path` is present only when FFHN can associate the process-level failure with one concrete
filesystem path.

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
10. `persist_transaction`

Interpretation:

1. `toml` means the source TOML text itself could not be decoded
2. `contract` means FFHN decoded the input shape but rejected one of its durable contracts or invariants
3. `htmlcut_interop` is reserved for failures reported across FFHN's HTMLCut seam
4. `internal` means FFHN hit an invariant or orchestration failure that was not attributable to user input or the upstream seam
5. `persist_transaction` means the primary persistence transaction failed and FFHN preserved rollback or cleanup context in the structured detail

### `notifications`

`notifications` records best-effort delivery attempts, not a guarantee that external side effects
completed as intended.

The `notifications` field is omitted entirely from serialized JSON when no deliveries were
attempted.

Each entry carries:

1. `route_name`
2. `duration_ms`
3. `outcome`

Interpretation:

1. each delivery belongs to the parent report's `result.kind`
2. `outcome.status = delivered` carries `exit_code = 0`
3. `outcome.status = failed` may also carry `exit_code` when the route process exited normally
4. `outcome.status = timed_out` records timeout detail without an exit code
5. failed notification delivery does not rewrite `result.kind`
6. failed notification delivery does make the CLI exit with code `1`
7. `outcome.error` may include captured route-process stderr text when FFHN could read it

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
4. `run_finished_at` is stamped after the worker pool converges and the aggregate report is assembled

Per-entry rules:

1. each entry must carry exactly one of `run_report` or `fatal_error`
2. when `run_report` is present, `run_report.target_id` must match the entry `target_id`
3. `entries` align one-for-one with `requested_targets`
4. `requested_targets` must be unique
5. `max_concurrency` must be positive

`fatal_error` is reserved for process-level failures where FFHN could not emit a structured
per-target `ffhn.run_report`, and it uses the same `ProcessErrorDetail` shape documented above.

`outcome_counts.persist_failure` counts entries whose emitted run report has any failed persist
write, including successful content outcomes whose final `last_run.json` write failed.

`outcome_counts.notification_failure` counts target entries whose `run_report.notifications` array
contains at least one failed or timed-out delivery. The per-entry delivery objects remain the
detailed source for route-level diagnostics.

## `RunFailureCause`

FFHN's current failure-cause vocabulary is:

| Cause | Failure class | Meaning |
| --- | --- | --- |
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
| `selection_contract_invalid` | permanent | FFHN's translated selection request violated the supported extractor contract |
| `selection_no_match` | permanent | extraction matched nothing |
| `selection_ambiguous_match` | permanent | extraction matched more than one exact candidate |
| `selection_internal_error` | permanent | the extractor seam returned a failure outside FFHN's supported selection surface |
| `canonicalization_error` | permanent | compare-time canonicalization failed |
| `compare_error` | permanent | compare stage failed after extraction |
| `persist_error` | transient | the primary state-commit transaction failed after FFHN had a reportable run body |
| `integrity_mismatch` | permanent | retained snapshot artifacts no longer match recorded digests |
