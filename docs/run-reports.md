---
afad: "4.0"
domain: RUN_REPORTS
updated: "2026-07-15"
route:
  keywords: [run report, outcome, observation, value unparseable, contract digest]
  questions: ["how do I read a run report?", "which outcomes are failures?", "what proves state was written?"]
---

# Run Reports

`ffhn.run_report` records one target run. Its outcome vocabulary is `initialized`, `changed`,
`unchanged`, `skipped_disabled`, `refused_contract_digest`, `acquisition_failed`,
`value_unparseable`, `config_invalid`, `target_unavailable`, `state_invalid`, `lock_unavailable`,
`fetch_failed`, and `persist_failed`.

Successful typed runs include the current observation. A report includes the prior canonical value
when state was available, a structured error when it was not, and `state_persisted = true` only
after the v2 state write commits. This can also be true for a source-suspect or permanent-error
run: it proves that health or episode state committed, never that an invalid observation became a
baseline. Policy staging itself is not a run-report result.

`delivery_outcomes` records every due attempt performed after the state commit. Each entry carries
the event id, route id, final attempt count, and status. `delivered`, `retry_scheduled`, and
`dead_lettered` mean the corresponding outbox update persisted. `delivered_uncommitted`,
`retry_uncommitted`, and `dead_letter_uncommitted` mean FFHN could not persist that update; the
error makes the external result explicit, and a later retry can duplicate a delivered payload.
`outbox_error` records a drain failure that prevented delivery processing from completing, including
one before an individual attempt could be identified. `outbox_overflow` records each newly staged
`(event_id, route_id)` that did not fit in the bounded queue. The report is written after delivery
processing, so those fields are evidence of the work this run completed; they are not a delivery
input and are not used for retries.

`ffhn.batch_run_report` preserves one report per requested target in request order. Its CLI exit is
nonzero if any member outcome is not a successful measurement or disabled skip, or if any member
has a non-delivered delivery outcome, an `outbox_error`, or queue overflow. `ffhn.reset_report` has
the same delivery evidence for an optional reset event.
