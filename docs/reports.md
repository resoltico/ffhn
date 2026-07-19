---
afad: "4.0"
domain: REPORTS
updated: "2026-07-19"
route:
  keywords: [run report, state, status report, reset report, observation]
  questions: ["what does FFHN persist?", "what is in a run report?", "how do I interpret reset?"]
---

# v2 Measurement Artifacts

Live measurement uses `ffhn.state` as its persisted artifact and emits `ffhn.run_report`,
`ffhn.batch_run_report`, `ffhn.status_report`, and `ffhn.reset_report` as operation results.

`ffhn.state` is written after every committed valid observation, source-suspect episode update,
permanent-error episode update, or integration-fault episode update. Its accepted observation,
when present, records
`raw_selected`, `comparison_projection`, its acquisition kind, parser identity and grammar version,
declared type and parameters, declared-type `canonical_value`, and `parse_diagnostics`.
For JSON acquisition the first two fields are the exact selected scalar token, including any string
quoting and escapes. Raw presentation is evidence; comparison uses the declared-type canonical
value. Text JSON observations accept strings only, decode escape spelling into their canonical
Unicode scalar sequence, and apply no trimming, case folding, locale rule, or Unicode normalization.
JSON observations produce and accept only an empty `parse_diagnostics` list; a persisted
nonempty list is invalid state and requires reset.

Normal state loading first admits only the `ffhn.state` version-17 schema envelope. A different
decoded schema is reset-required before FFHN decodes any state facts; FFHN never migrates or
partially interprets it. Malformed JSON remains unreadable state. Each public state or report type
also rejects a noncurrent schema during direct deserialization, rather than relying on callers to
remember a separate validation step.

For `html_text`, raw evidence is HTMLCut's original plain DOM descendant text output. For
`html_rendered_text`, it is HTMLCut's semantic rendered-text output. Without DOM canonicalization,
each is also its comparison projection; with canonicalization, comparison comes from the same
projection of HTMLCut's detached selected-subtree clone. For `html_attribute`, both remain the
original selected CSS attribute value. An HTML observation also persists
`htmlcut_semantics_version`, `plan_digest_sha256`, a positive `htmlcut_candidate_count`, and the
public `htmlcut_diagnostics`; absence, a stale semantics version, an invented diagnostic shape, or
a nonpositive count is invalid state. FFHN accepts only its closed projection of the exact HTMLCut
v12 diagnostic-detail shapes reachable through its pinned interop profile. In particular,
invalid-selector evidence retains the one-based source line, one-based UTF-16 column, and closed
parser class. A failed HTML acquisition or rejected HTMLCut preflight plan carries a closed
`error_class`, an optional closed primary diagnostic code, candidate count when known, plan digest,
diagnostics, selector-parse evidence when applicable, and typed FFHN postcondition evidence in the
run report's structured error detail. No HTMLCut structured
result or canonical clone is exposed as an FFHN target or observation projection.

State also persists a pending-only durable outbox. Its records have a deterministic event id and
route id, adapter-neutral event kind and optional condition id, immutable payload bytes, attempt
count, a bounded structured delivery-failure detail when an attempt has failed, and next retry
timestamp. The durable detail retains complete process terminal, writer, and stderr facts; stderr
evidence retains its exact bounded raw bytes as canonical base64, records the total source-byte
count as a canonical decimal string so it cannot saturate at a platform word-size limit, and derives
display text and retained-byte encoding only when rendered. The encoding describes the bounded
retained byte artifact, not unretained child-process stderr; JSON exposes the raw artifact only as
base64 so machine consumers can make the same interpretation. Its stable JSON encoding is bounded
before state is written.
Successful deliveries remove records; terminal failures are reported and removed. Pending records
are never evicted to admit newer events.

Run reports contain the outcome, mode, timestamps, target identity, optional contract digest,
optional observation, prior canonical value, structured error detail, `policy_evaluation`, a
separate `lifecycle` facet, whether state persisted, delivery outcomes, outbox-overflow evidence,
and any `outbox_error_detail` that
halted delivery processing. `policy_evaluation` lists every named condition decision for a valid typed
observation, its pre-run reference evidence, and every route-independent event eligibility; other
branches explicitly report that condition evaluation did not run. An `integration_fault` outcome
carries the closed `error_detail.integration_fault_code`: `htmlcut_internal_error`,
`ffhn_boundary_invariant_violation`, or `ffhn_policy_invariant_violation`. Delivery outcomes and
overflow evidence carry the same event kind and condition identity retained in durable state.

`lifecycle.before` is the complete durable snapshot read before a run when valid matching state
was safely available. `lifecycle.after` is the complete staged successor when the run transitions
state. Each snapshot always contains source health (`state`, reason, unresolved count, first-seen
time, and last detail), plus nullable permanent-error and integration-fault episodes.
`state_persisted` answers only whether the staged write committed: dry runs and failed commits
expose their staged `after` snapshot while remaining `false`. Status reports expose the same
current durable lifecycle snapshot after acquiring the target's shared lock, admitting and
validating the state envelope, and verifying the contract digest. A target that is base-valid but
projection-invalid therefore reports `invalid_config` with its verified lifecycle, whereas
unreadable, stale, or mismatched state exposes none.
Every diagnostic carries a closed `kind` and `operation`. Every `io` diagnostic carries exactly one
typed cause: a native operating-system failure has its closed `io_error_class`, while HTTP status,
configured byte-limit, and UTF-8 acquisition failures carry typed `fetch_failure` evidence. Its
`message` is only the unclassified explanatory payload, never rendered foreign I/O, parser, URL,
time, or embedded acquisition metadata prose. When FFHN must retain only a bounded message prefix,
the separate `message_truncation` object records the original byte count and SHA-256 digest; no
truncation state is encoded into the payload text.
Source-health details must match their closed reason: fetch failures carry file or HTTP I/O evidence;
JSON failures carry
JSON-selection evidence; value failures carry typed-value parsing evidence; and HTML failures carry
HTMLCut extraction evidence. Delivery and integration-fault diagnostics are rejected from persisted
health state.
A delivery outcome carries `error_detail` for a failed process attempt and
`outbox_error_detail` when the matching outbox update could not be committed. A process that
exited successfully can instead carry `delivery_observability_detail` when stderr capture was
incomplete. The detail distinguishes a reader I/O failure, an unavailable configured reader, and
a reader panic; it never changes delivery success or retry behavior. HTML acquisition evidence is retained only in observations
and HTMLCut failure details; there is no snapshot-history artifact.

The `summary` CLI view renders every structured diagnostic fact carried by these documents, including
typed acquisition and HTMLCut evidence. It deliberately excludes retained stderr text but labels its
metadata as `retained_encoding`, so the label cannot imply a claim about discarded bytes. If an exact
raw byte prefix has a valid UTF-8 prefix followed only by an incomplete terminal UTF-8 sequence, the
label is `utf8_incomplete_at_retention_boundary`; `utf8_lossy` remains reserved for genuinely invalid
retained bytes. Any diagnostic text that contains control characters is JSON-string encoded in the
summary so one evidence item cannot corrupt the line-oriented human view.

State is isolated at `<watch_root>/<target_id>/.ffhn/state.json`. A missing file means no state of
any kind has committed; an existing file may contain health, permanent-error, or integration-fault
facts before a valid observation. `ffhn.reset_report.storage_cleared` tells whether the v2 storage root existed
when it was blind-deleted; it is never a migration record or preserved historical evidence. A reset
report can also carry delivery evidence when the target has `on_run` routes.
