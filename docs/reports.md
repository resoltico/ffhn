---
afad: "4.0"
domain: REPORTS
updated: "2026-07-16"
route:
  keywords: [run report, state, status report, reset report, observation]
  questions: ["what does FFHN persist?", "what is in a run report?", "how do I interpret reset?"]
---

# v2 Measurement Artifacts

Live measurement uses `ffhn.state` as its persisted artifact and emits `ffhn.run_report`,
`ffhn.batch_run_report`, `ffhn.status_report`, and `ffhn.reset_report` as operation results.

`ffhn.state` is written after every committed valid observation, source-suspect episode update, or
permanent-error episode update. Its accepted observation, when present, records
`raw_selected`, `comparison_projection`, its acquisition kind, parser identity and grammar version,
declared type and parameters, normalized `canonical_value`, and `parse_diagnostics`.
For JSON acquisition the first two fields are the exact selected scalar token, including any string
quoting and escapes. Raw presentation is evidence; comparison uses the normalized canonical value.
JSON observations produce and accept only an empty `parse_diagnostics` list; a persisted
nonempty list is invalid state and requires reset.

For `html_text`, raw evidence is HTMLCut's original selected text output. Without DOM
canonicalization it is also the comparison projection; with canonicalization, comparison is the
text rendered from HTMLCut's detached selected-subtree clone. For `html_attribute`, both remain
the original selected CSS attribute value. An HTML observation also persists
`htmlcut_semantics_version`, `plan_digest_sha256`, a positive `htmlcut_candidate_count`, and the
public `htmlcut_diagnostics`; absence, a stale semantics version, an invented diagnostic shape, or
a nonpositive count is invalid state. A failed HTML acquisition or rejected HTMLCut preflight plan
carries the same reason, candidate count when known, plan digest, and diagnostics in the run
report's structured error detail. No HTMLCut structured result or canonical clone is exposed as an
FFHN target or observation projection.

State also persists a pending-only durable outbox. Its records have a deterministic event id and
route id, immutable payload bytes, attempt count, optional last error, and next retry timestamp.
Successful deliveries remove records; terminal failures are reported and removed. Pending records
are never evicted to admit newer events.

Run reports contain the outcome, mode, timestamps, target identity, optional contract digest,
optional observation, prior canonical value, structured error detail, whether state persisted,
delivery outcomes, outbox-overflow evidence, and any `outbox_error` that halted delivery
processing. Uncommitted delivery outcomes make an external process result visible even when FFHN
could not persist the matching outbox transition. Policy staging is intentionally not emitted in
run reports. HTML acquisition evidence is retained only in observations and HTMLCut failure
details; there is no snapshot-history artifact.

State is isolated at `<watch_root>/<target_id>/.ffhn/state.json`. A missing file means no state of
any kind has committed; an existing file may contain health or permanent-error facts before a valid
observation. `ffhn.reset_report.storage_cleared` tells whether the v2 storage root existed
when it was blind-deleted; it is never a migration record or preserved historical evidence. A reset
report can also carry delivery evidence when the target has `on_run` routes.
