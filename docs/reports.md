---
afad: "4.0"
version: "5.0.0"
domain: REPORTS
updated: "2026-05-03"
route:
  keywords: [reports, ffhn.state, ffhn.extraction_record, ffhn.notification_payload, ffhn.status_report, snapshot references, snapshot artifacts, state document]
  questions: ["what do ffhn state and status documents mean?", "what is stored in ffhn.state?", "what is stored in ffhn.extraction_record?", "what is ffhn.notification_payload?", "what do ffhn snapshot artifacts mean?"]
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

Run-oriented semantics live in [run-reports.md](run-reports.md):

1. `ffhn.run_report`
2. `ffhn.batch_run_report`
3. reason-code vocabulary
4. the shared structured process-error shape used by `persist.error` and batch `fatal_error`

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
3. snapshot artifact paths are FFHN-owned forward-slash relative paths beneath the target directory; absolute Unix paths, Windows drive prefixes, UNC paths, empty segments, and `.` or `..` segments are invalid
4. `snapshot_history` is ordered newest first

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

## Snapshot Artifact Meanings

Snapshot references point at three persisted files:

1. `canonical.txt`: compare-time canonical text
2. `outer.html`: selected outer HTML from HTMLCut
3. `extraction.json`: persisted `ffhn.extraction_record`

Live `changed` runs rotate the previous current snapshot into history, then prune history down to `storage.history_limit - 1` older entries. FFHN stages those snapshot mutations so a later `state.json` write failure rolls the current baseline back instead of poisoning the previously valid state. Live `unchanged` runs keep the existing snapshot references intact.
