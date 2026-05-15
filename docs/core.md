---
afad: "4.0"
domain: CORE
updated: "2026-05-14"
route:
  keywords: [core runtime, validate_target, status, run_once, run_once_dry_run, run_batch, locking, dry run]
  questions: ["what operations does ffhn-core expose?", "what does ffhn dry-run skip?", "how does ffhn-core classify successful and failed runs?"]
---

# Core Runtime

`ffhn-core` exports five public operations:

1. `validate_target`
2. `status`
3. `run_once`
4. `run_once_dry_run`
5. `run_batch`

## Validation Behavior

`validate_target` is the lowest-level public contract check.

It:

1. verifies that the configured watch root already exists and is a directory
2. reads one `target.toml`
3. validates the frozen `ffhn.target` schema and all section-specific rules
4. verifies that `target_id` matches the containing directory name

It returns `CoreError` instead of a structured report when FFHN could not read the target
directory, decode TOML, or satisfy the target-contract invariants.

## Execution Semantics

Every run starts from the same reportable core stages:

1. validate the target document and directory identity
2. in live mode, attempt the exclusive per-target run lock
3. load prior state if it exists
4. fetch or read the configured source
5. build and execute the HTMLCut interop plan
6. canonicalize the selected `text_output`
7. compare the current digest against the prior digest
8. emit a structured report

The main differences are in locking and side effects.

## Live Runs

`run_once` and live `run_batch`:

1. short-circuit invalid target documents before lock or state access, and normalize explicit target-load filesystem faults into structured `target_unavailable` failures after watch-root validation has already succeeded
2. acquire the exclusive per-target run lock before reading stored state
3. treat unreadable or invalid stored state as a structured permanent failure
4. treat snapshot-integrity mismatch as a structured permanent failure
5. persist live state and snapshot artifacts on successful runs, and state-only updates when applicable
6. surface failed primary state commits after a reportable outcome as `result.kind = failed_transient` with `result.cause = persist_error`, and record every durable write result in the persist section
7. attempt notification delivery routes that match the sealed run result before the final `last_run.json` write
8. keep notification delivery failures in `notifications[]` and leave `result.kind` unchanged
9. attempt the final `last_run.json` write after notification delivery results are known, publishing one `ffhn.last_run_snapshot` wrapper around the live pre-publication run-report snapshot and without rewriting `result.kind` if only that final write fails

A valid disabled target short-circuits to `skipped_disabled` instead of fetching or extracting.
In live mode that short-circuit is part of the full lifecycle: FFHN persists the skipped outcome,
may deliver `skipped_disabled` notification routes, and attempts the final `last_run.json` write.

Live successful outcomes are:

1. `initialized`
2. `changed`
3. `unchanged`

Live non-fatal structured outcomes are:

1. `failed_transient`
2. `failed_permanent`
3. `skipped_disabled`

Fatal errors sit outside the structured run-result vocabulary. They occur only when FFHN cannot
emit a valid `ffhn.run_report` for that target at all.

## Dry Runs

`run_once_dry_run` and dry-run `run_batch` preserve validation, fetch, extraction, and compare
behavior, but they take the shared per-target run lock first, wait behind active live runs when
necessary, and then skip all live mutations:

1. no exclusive run lock
2. no snapshot writes
3. no `state.json` writes
4. no `last_run.json` writes
5. no notification delivery

Dry-run also relaxes live-state strictness. If an existing `state.json` is unreadable or invalid,
or the retained snapshot artifacts fail integrity checks, dry-run proceeds as an inspection run
instead of returning the live-mode permanent failure. Shared locking keeps that inspection path
from racing a live persist and fabricating transient integrity drift out of a half-updated target
directory.

Dry-run treats `enabled = false` the same way as live mode at the content boundary. A valid
disabled target returns `skipped_disabled` without fetching or extracting, while both persist paths
remain `not_attempted` and no notifications are delivered.

## Batch Behavior

`run_batch` is the core multi-target primitive.

Its guarantees are:

1. `requested_targets` must be unique
2. `max_concurrency` must be positive
3. `requested_targets` stays in caller-supplied order
4. `entries` are emitted in the same stable order
5. `max_concurrency` records the requested concurrency bound
6. idle workers pull the next target immediately instead of waiting on chunk barriers
7. each entry contains either one `run_report` or one structured `fatal_error` object
8. `outcome_counts` are derived from the actual entry contents and must match them exactly
9. `outcome_counts.notification_failure` counts entries whose notification delivery outcome is not `delivered`
10. discovery-based batches keep valid disabled targets visible as `skipped_disabled` entries instead of excluding them

## Status Behavior

`status` validates the watch root and target first, then acquires a shared lock for valid targets
and returns a `ffhn.status_report`. Explicit target-load filesystem faults after watch-root
validation become structured `unavailable_target` results instead of fatal stderr.

It reports:

1. top-level `display_name` plus `enabled = true | false` for every valid target, so target identity and disablement stay distinct from baseline readiness
2. `pending` when the target is valid but has never captured a baseline
3. `ready` when the target is valid and `baseline_phase = has_baseline`
4. `invalid_config` when target validation fails
5. `unavailable_target` when the explicit `target.toml` path is missing or unreadable
6. `invalid_state` when state validation fails or `state.json` is unreadable
7. `integrity_mismatch` when retained artifacts fail integrity

When FFHN can still decode the current stored state enough to recover `baseline.kind`, invalid
`status` reports preserve that parsed `baseline_phase` instead of flattening everything to
`never_succeeded`. Unreadable or unrecoverable state falls back to `never_succeeded`.

`status` never mutates `state.json` or snapshot artifacts, but valid targets may lazily create
`lock/run.lock` so shared locking has a filesystem anchor. Valid dry-runs use that same shared-lock
anchor, and both operations wait behind an active live run rather than failing on transient
shared-lock contention.

Live lock-contention failures take a similar stable-view approach for lifecycle metadata. When a
second live run sees `lock_unavailable`, FFHN waits long enough to read one stable durable state
view before sealing the failure report's baseline-phase fields.
