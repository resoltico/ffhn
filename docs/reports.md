---
afad: "4.0"
domain: REPORTS
updated: "2026-05-14"
route:
  keywords: [reports, ffhn.state, ffhn.extraction_record, ffhn.last_run_snapshot, ffhn.notification_payload, ffhn.status_report, snapshot references, snapshot artifacts, state document]
  questions: ["what do ffhn state and status documents mean?", "what is stored in ffhn.state?", "what is stored in ffhn.extraction_record?", "what is stored in ffhn.last_run_snapshot?", "what is ffhn.notification_payload?", "what do ffhn snapshot artifacts mean?"]
---

# State, Report, And Snapshot Documents

FFHN currently emits, persists, or writes seven machine-readable document families:

1. `ffhn.state`
2. `ffhn.extraction_record`
3. `ffhn.run_report`
4. `ffhn.last_run_snapshot`
5. `ffhn.notification_payload`
6. `ffhn.batch_run_report`
7. `ffhn.status_report`

`ffhn.extraction_record` is persisted inside snapshot artifacts, but it is not emitted directly by the CLI.

Run-oriented semantics live in [run-reports.md](run-reports.md):

1. `ffhn.run_report`
2. `ffhn.batch_run_report`
3. run-result and failure-cause vocabulary
4. the shared structured process-error shape used by failed persist-write entries and batch `fatal_error`

## `ffhn.state`

`state.json` records the most recent durable target state.

Important fields:

1. `target_id`
2. `baseline`
3. optional `last_run`

Field semantics:

1. `baseline.kind = "pending" | "ready"` is the durable baseline-state discriminator
2. `baseline.ready.current_snapshot` and `baseline.ready.snapshot_history` are `SnapshotReference` objects that carry `slot`, digests, relative artifact paths, and `captured_at`
3. `last_run` summarizes the most recent attempted live run that reached state persistence, not only the most recent success
4. `last_run.outcome` is one of `initialized`, `changed`, `unchanged`, `skipped_disabled`, `failed_transient`, or `failed_permanent`

## `ffhn.last_run_snapshot`

`last_run.json` stores `ffhn.last_run_snapshot`.

Important fields:

1. `run_report`

Key invariants:

1. `run_report` is always a live `ffhn.run_report`
2. `run_report.notifications` already includes post-delivery notification results
3. `run_report.persist.last_run_write.status` is always `not_attempted` inside this artifact because the nested report is the exact snapshot FFHN attempted to publish
4. the file exists only when that publication succeeded; if the final write fails, stdout carries the newer live `ffhn.run_report` with `persist.last_run_write.status = failed`, but `last_run.json` remains at the previous successful snapshot
5. failed `last_run` summaries carry `cause`
6. snapshot artifact paths are FFHN-owned forward-slash relative paths beneath the target directory; absolute Unix paths, Windows drive prefixes, UNC paths, empty segments, and `.` or `..` segments are invalid
7. `baseline.ready.snapshot_history` is ordered newest first

Key invariants:

1. `baseline.pending` forbids current or historical snapshots
2. `baseline.ready` requires `current_snapshot`
3. `baseline.ready` requires `last_run`
4. successful `last_run` summaries require `baseline.ready`
5. `current_snapshot.slot` must be `current`
6. `snapshot_history[].slot` must be `history`

## `ffhn.extraction_record`

Every persisted snapshot directory contains one `extraction.json` file with `ffhn.extraction_record`.

Important fields:

1. `comparison_input_sha256`
2. `outer_html_sha256`
3. `selection_kind`
4. `selection_match`
5. `output_kind`
6. `candidate_count`
7. `selected_candidate_index`
8. `selection_evidence`
9. `warning_codes`
10. `created_at`

Interpretation:

1. `comparison_input_sha256` is the digest of HTMLCut's selected `text_output` after FFHN has normalized line endings to LF
2. `selection_evidence` is FFHN-owned evidence for the selected payload, not a raw upstream HTMLCut metadata object
3. `warning_codes` includes warning-level diagnostics only
4. `warning_codes` can include `EFFECTIVE_BASE_URL_UNRESOLVED` when URL rewriting was requested but HTMLCut could not resolve an effective HTTP(S) base URL
5. the record is paired one-for-one with the sibling `canonical.txt` and `outer.html` artifacts in the same snapshot directory

Key invariants:

1. `candidate_count` and `selected_candidate_index` must be positive, and the selected index must stay within the candidate count
2. `selection_kind`, `selection_match`, `output_kind`, and `selection_evidence.kind` must stay internally consistent
3. CSS-selector evidence carries `path` plus `tag_name`; delimiter-pair evidence carries FFHN-owned byte ranges and boundary-inclusion flags

## `ffhn.status_report`

`status` emits `ffhn.status_report`.

Important fields:

1. optional `display_name`
2. optional `enabled`
3. `status`
4. optional `baseline_phase`

Interpretation:

1. valid-target reports carry both `display_name` and `enabled = true | false`, so operator identity and disablement stay machine-distinct from baseline readiness
2. `display_name` and `enabled` are both omitted only for `status.kind = "invalid_config"` and `status.kind = "unavailable_target"` because FFHN could not trust the target document enough to surface target metadata
3. `status.kind = "pending"` means the target is valid but no baseline is ready yet
4. `status.kind = "ready"` means the target is valid and has a current baseline snapshot summary plus optional history
5. `status.kind = "invalid_config"` means target validation failed before state loading
6. `status.kind = "unavailable_target"` means the explicit `target.toml` path was missing or unreadable after watch-root validation had already succeeded
7. `status.kind = "invalid_state"` means `state.json` was unreadable or contract-invalid
8. `status.kind = "integrity_mismatch"` means retained artifacts failed digest or extraction-record integrity
9. `baseline_phase` is a derived view of the durable baseline state: `pending` implies `never_succeeded`, `ready` implies `has_baseline`, and invalid-state or integrity-mismatch reports preserve the recovered `baseline.kind` when FFHN can still decode it
10. unreadable or unrecoverable `state.json` falls back to `baseline_phase = "never_succeeded"`
11. malformed persisted JSON reports `error_detail.kind = "json"`, while valid JSON that violates FFHN's `state.json` or `extraction.json` contract reports `error_detail.kind = "contract"`

Special case:

1. `status.kind = "invalid_config"` and `status.kind = "unavailable_target"` do not carry `baseline_phase`, `display_name`, or `enabled`
2. invalid status variants require `error_detail`, while `pending` and `ready` forbid it

`status.ready.current_snapshot` and `status.ready.snapshot_history` carry only digest summaries, not the full artifact bodies.

## `ffhn.notification_payload`

Notification routes receive `ffhn.notification_payload` on stdin.

Important fields:

1. `route_name`
2. `delivery_started_at`
3. `run_report`

Key invariants:

1. `run_report` is always a live pre-delivery snapshot
2. `delivery_started_at` is on or after `run_report.run_finished_at`
3. `run_report.notifications` is always empty inside the payload because delivery results do not exist yet
4. `run_report.persist.last_run_write.status` is always `not_attempted` because the final `last_run.json` write happens after route delivery
5. `run_report.persist.state_commit.status` may already be `failed` when an earlier live persist step failed before FFHN started delivering notifications

## Snapshot Artifact Meanings

Snapshot references point at three persisted files:

1. `canonical.txt`: compare-time canonical text
2. `outer.html`: selected outer HTML from HTMLCut
3. `extraction.json`: persisted `ffhn.extraction_record`

Live `changed` runs rotate the previous current snapshot into history, then prune history down to `storage.history_limit - 1` older entries. FFHN stages those snapshot mutations so a later `state.json` write failure rolls the current baseline back instead of poisoning the previously valid state. Live `unchanged` runs keep the existing snapshot references intact.
