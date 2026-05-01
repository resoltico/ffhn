---
afad: "4.0"
domain: CORE
updated: "2026-04-30"
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

It returns `CoreError` instead of a structured report when FFHN could not read the target directory, decode valid TOML, or satisfy the target-contract invariants.

## Execution Semantics

Every run starts from the same reportable core stages:

1. validate the target document and directory identity
2. in live mode, attempt the exclusive per-target run lock
3. load prior state if it exists
4. fetch or read the configured source
5. build and execute the HTMLCut interop plan
6. canonicalize `comparison_input_text`
7. compare the current digest against the prior digest
8. emit a structured report

The main differences are in locking and side effects.

## Live Runs

`run_once` and live `run_batch`:

1. short-circuit invalid target documents before lock or state access, but surface target-load filesystem faults as fatal process errors
2. acquire the exclusive per-target run lock before reading stored state
3. treat unreadable or invalid stored state as a structured permanent failure
4. treat snapshot-integrity mismatch as a structured permanent failure
5. persist live state and snapshot artifacts on successful runs, and state-only updates when applicable
6. surface live persistence failures after a reportable outcome as structured `persist_error` reports whose persist section records the failing durable write
7. attempt notification hooks that match the final run outcome
8. keep notification delivery failures in `notifications[]` and leave `run_outcome` unchanged
9. attempt the final `last_run.json` write after notification delivery results are known

An explicitly disabled live target short-circuits to `skipped_disabled` instead of fetching or extracting.
That short-circuit is still a live path: FFHN persists the skipped outcome, may deliver `skipped_disabled` notification hooks, and still attempts the final `last_run.json` write.

Live successful outcomes are:

1. `initialized`
2. `changed`
3. `unchanged`

Live non-fatal structured outcomes are:

1. `failed_transient`
2. `failed_permanent`
3. `skipped_disabled`

Fatal errors sit outside the structured run-outcome vocabulary. They only occur when FFHN cannot emit a valid `ffhn.run_report` for that target at all.

## Dry Runs

`run_once_dry_run` and dry-run `run_batch` preserve validation, fetch, extraction, and compare behavior, but they take the shared per-target run lock first, wait behind active live runs when necessary, and then skip all live mutations:

1. no exclusive run lock
2. no snapshot writes
3. no `state.json` writes
4. no `last_run.json` writes
5. no notification hooks

Dry-run also relaxes live-state strictness. If an existing `state.json` is unreadable or invalid, or the retained snapshot artifacts fail integrity checks, dry-run still proceeds as an inspection run instead of returning the live-mode permanent failure. Shared locking keeps that relaxed inspection path from racing a live persist and fabricating transient integrity drift out of a half-updated target directory.

Dry-run also treats `enabled = false` differently from live mode. An explicitly named disabled target still follows the inspection path, while discovery-based `run --all` continues to exclude valid disabled directories before `run_batch` is called.

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

## Status Behavior

`status` validates the watch root and target first, then acquires a shared lock for valid targets and returns a `ffhn.status_report`. Target-load filesystem faults stay fatal instead of being flattened into structured `config_invalid`.

It reports:

1. `pending` when the target is valid but has never captured a baseline
2. `ready` when the target is valid and `state_phase = has_baseline`
3. `invalid` when target validation fails, state validation fails, `state.json` is unreadable, or retained artifacts fail integrity

When FFHN can still decode the stored state enough to recover its declared phase, invalid `status` reports preserve that parsed `state_phase` instead of flattening everything to `never_succeeded`. Unreadable state falls back to `never_succeeded`.

`status` never mutates `state.json` or snapshot artifacts, but valid targets may lazily create `lock/run.lock` so shared locking has a filesystem anchor. Valid dry-runs use that same shared-lock anchor, and both operations wait behind an active live run rather than failing on transient shared-lock contention.
