---
afad: "4.0"
domain: CORE
updated: "2026-07-15"
route:
  keywords: [core, typed observation, run order, state, dry run]
  questions: ["what does one core run do?", "does dry run mutate state?", "when does state advance?"]
---

# Core Run Semantics

A live run acquires the target lock, loads only v2 state, rejects a contract-digest mismatch,
fetches the configured source, selects one scalar by JSON Pointer or one HTML value through
HTMLCut, and parses that value into its declared type. HTMLCut is invoked with an FFHN-owned
structured plan so that public match metadata is preserved; target authors never configure a
structured output. Only a valid typed observation can advance state. Parse and acquisition failures
leave accepted state unchanged.

The first valid observation is `initialized`. Later values compare normalized canonical text, so
decimal `1.0` and `1.00` are unchanged even though `raw_selected` preserves their presentation.
For JSON strings, that evidence is the exact selected token, including its quoting and escapes.

Dry runs take a shared lock and run the same validation, acquisition, and typing path, but never
write `.ffhn/state.json`. Disabled targets do not acquire sources and report `skipped_disabled`.

The live core invokes the policy algebra using only pre-run contexts. It stages every
`on_condition` trigger and immediate `on_run` eligibility: initialization, arithmetic overflow,
zero references, a source escalation, and a newly begun permanent-error episode. The runtime
retains that exact policy result with its staged next state through the commit boundary. A valid
observation atomically advances the accepted baseline and sequence and persists every named
condition result, active state, and result transition.
Source-suspect runs persist only health episodes; permanent JSON or HTML contract failures persist
only permanent-error episodes. Neither failure branch advances a baseline or resets hysteresis.
Dry runs stage the same decisions without writing state. HTMLCut `NO_MATCH`, `AMBIGUOUS_MATCH`,
`MISSING_ATTRIBUTE`, and `MATCH_INDEX_OUT_OF_RANGE` are source-health reasons; invalid selectors,
slice patterns, unsupported value types, invalid HTMLCut input URLs, and invalid FFHN HTML
selection contracts are permanent. The public planning API cannot stage an invented
classification.

For a live run, FFHN materializes the exact staged eligibilities into route-specific, immutable
process-stdin payload bytes and enqueues them in the same state write as measurement and temporal
facts. It starts delivery only after that commit is crash durable: the staged and installed state
file have been synchronized, and Unix also synchronizes the replacement's directory metadata. It
then drains due records, including older records, using only stored bytes; delivery never
re-evaluates a predicate and never reads a run report. Success removes a record. A failed attempt
persists deterministic retry state, and the terminal attempt removes the record while reporting it
as dead-lettered. If FFHN cannot persist one of those outbox updates after a process has run, the
report records an explicit uncommitted outcome and an outbox error instead of hiding the attempt; a
later drain may duplicate a delivered payload. A full outbox drops only the newly staged route
record and leaves both prior records and measurement state intact.

One drain considers only records already due when it began. Therefore a failed record's retry,
which is scheduled from the actual completion time, always waits for a later drain even when the
durable write takes longer than its configured backoff.

Dry runs may calculate prospective outbox overflow for the exact staged plan, but they neither
write state nor invoke a delivery process.
