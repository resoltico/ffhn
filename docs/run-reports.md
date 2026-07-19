---
afad: "4.0"
domain: RUN_REPORTS
updated: "2026-07-19"
route:
  keywords: [run report, outcome, observation, value unparseable, contract digest]
  questions: ["how do I read a run report?", "which outcomes are failures?", "what proves state was written?"]
---

# Run Reports

`ffhn.run_report` records one target run. Its outcome vocabulary is `initialized`, `changed`,
`unchanged`, `skipped_disabled`, `refused_contract_digest`, `acquisition_failed`,
`value_unparseable`, `config_invalid`, `target_unavailable`, `state_invalid`, `lock_unavailable`,
`fetch_failed`, `persist_failed`, and `integration_fault`.

Successful typed runs include the current observation. A report includes the prior canonical value
when state was available, a structured error when it was not, and `state_persisted = true` only
after the v2 state write commits. This can also be true for a source-suspect, permanent-error, or
integration-fault run: it proves that health or episode state committed, never that an invalid
observation became a baseline. An integration-fault report carries the closed
`error_detail.integration_fault_code`. Every I/O report diagnostic carries exactly one typed cause:
native operating-system failures expose a closed `error_detail.io_error_class`, while HTTP status,
configured byte-limit, and UTF-8 acquisition failures expose typed `error_detail.fetch_failure`.
Every run also contains `policy_evaluation`: valid
observations carry one result for every configured condition, including its outcome, trigger,
active state before and after evaluation, and configured reference evidence; branches that did not
reach valid-observation policy evaluation state that explicitly. Both forms carry route-independent
event eligibilities, so dry runs and targets without routes reveal the exact policy events that
would be staged without claiming that delivery occurred.

Every run report also carries a dedicated `lifecycle` facet, separate from policy events. `lifecycle.before` is the complete durable source-health, permanent-error-episode, and integration-fault-episode snapshot safely read under the target lock before execution. `lifecycle.after` is the complete staged successor whenever the run stages any state transition. A disabled run has `before` when state exists but no `after`; dry runs and failed commits retain both snapshots while `state_persisted` remains `false`. `state_persisted` proves only that the staged write committed; it never decides whether the displayed `after` snapshot is present or durable. A missing `before` means FFHN could not safely obtain valid matching state. The report schema enforces that relationship directly: outcomes that stop before staging never carry `after`, staged measurement, health, persistence, and integration-fault outcomes always carry it, and `config_invalid` remains the one deliberate optional case because it can fail before staging or create a durable permanent-error episode.

Status has the same closed relationship. `ready` carries both a verified lifecycle and its accepted observation; every other status kind omits the observation. A lifecycle-bearing status also retains the verified target display name, enabled state, and contract digest. `unavailable_target` and `invalid_state` never expose lifecycle facts because FFHN could not establish that those facts are valid for the current target.

`delivery_outcomes` records every due attempt performed after the state commit. Each entry carries
the event id, route id, adapter-neutral event kind, optional condition id, final attempt count,
status, and closed diagnostic evidence where applicable. `delivered`, `retry_scheduled`, and
`dead_lettered` mean the corresponding outbox update persisted. `delivered_uncommitted`,
`retry_uncommitted`, and `dead_letter_uncommitted` mean FFHN could not persist that update; the
`outbox_error_detail` makes the external result explicit, and a later retry can duplicate a delivered payload. A
`delivered_uncommitted` result is a persistence problem, not a failed child process: the external payload was delivered.
A failed process has an `error_detail` containing the terminal, writer, and stderr facts that led
to its derived primary failure. A successful process with incomplete stderr capture instead has a
`delivery_observability_detail`; it distinguishes reader I/O failure, reader unavailability, and
reader panic as visible evidence, not as a reason to retry an accepted payload.

When a process detail carries retained stderr, JSON serializes only its exact bounded byte artifact
as `retained_bytes_base64` with the original byte count and truncation flag. The summary derives a
human presentation from those bytes and labels its classification `retained_encoding`; it describes
the retained artifact and intentionally makes no assertion about any discarded suffix. A raw prefix
with a valid UTF-8 prefix followed only by an incomplete terminal UTF-8 sequence is labeled
`utf8_incomplete_at_retention_boundary` and renders only its complete UTF-8 prefix, without adding a
replacement character. `utf8_lossy` identifies genuinely invalid retained bytes.
`outbox_error_detail` records a drain failure that prevented delivery processing from completing, including
one before an individual attempt could be identified. `outbox_overflow` records each newly staged
`(event_id, route_id)` that did not fit in the bounded queue together with its event kind and
optional condition id. Under pressure, FFHN retains existing pending records and admits new distinct
candidates in the target's condition-then-route declaration order; persisted event-id order never
decides admission. The report is written after delivery
processing, so those fields are evidence of the work this run completed; they are not a delivery
input and are not used for retries.

`ffhn.batch_run_report` preserves one report per requested target in request order. Its CLI exit is
nonzero if any member outcome is not a successful measurement or disabled skip, or if any member
has a non-delivered delivery outcome, an `outbox_error_detail`, or queue overflow. `ffhn.reset_report` has
the same delivery evidence for an optional reset event.
