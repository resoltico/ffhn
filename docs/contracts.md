---
afad: "4.0"
domain: CONTRACTS
updated: "2026-07-16"
route:
  keywords: [contracts, schema versions, target, state, reports, reset]
  questions: ["which v2 contracts exist?", "what is the persisted layout?", "what does reset delete?"]
---

# Contract Inventory

The current measurement, policy, and durable-delivery contract surface has these document
identities:

| Document | Version | Location or output |
| --- | ---: | --- |
| `ffhn.target` | 9 | `<target>/target.toml` |
| `ffhn.state` | 9 | `<target>/.ffhn/state.json` |
| `ffhn.run_report` | 8 | single-target run stdout |
| `ffhn.batch_run_report` | 8 | batch stdout |
| `ffhn.status_report` | 7 | status stdout |
| `ffhn.reset_report` | 3 | reset stdout |

An accepted observation is always valid and typed. It carries raw evidence, comparison projection,
an acquisition kind (`json_pointer`, `html_text`, or `html_attribute`), parser identity and grammar,
declared type parameters, normalized canonical value, and parse diagnostics. JSON evidence is the
exact selected scalar token, including string quoting and escapes. For `html_text`, raw evidence
is always HTMLCut's original selected text; when configured, only its detached canonical clone
supplies the comparison projection. `html_attribute` keeps the original CSS match-metadata
attribute for both. HTML observations additionally bind the current HTMLCut extraction-semantics
version, plan digest, positive candidate count, and the public HTMLCut diagnostics. A failed parse
is never a baseline value.

The target digest includes a source-kind tag, target and fetch configuration, projection, declared
type, parser identity and grammar version, type parameters, the complete ordered named-condition
list, and `escalate_after`. HTML targets additionally include the HTMLCut extraction-semantics
version; JSON targets do not. The HTML projection includes any DOM canonicalization policy, so a
change to that policy changes the measurement identity and requires reset. It excludes
per-execution plan digests and operational delivery policy: routes, queue capacity, attempts, and
retry timing can evolve without changing the measurement identity. A mismatch is refused; reset
is the only lifecycle operation that clears it.

Schema 9 requires an explicit `conditions` list and positive `escalate_after`. The policy API
remains a pure planning surface. It returns deterministic condition and immediate run-event
eligibilities. The runtime retains that result with the exact next schema-9 state until the commit
boundary; state owns the committed observation sequence, fixed initial baseline, per-condition
result/active/transition facts, source health, permanent-error episode, and pending-only outbox.
Source-health reasons use the fixed vocabulary: `fetch_failed`, `json_malformed`,
`json_missing_pointer_target`, `json_non_scalar_pointer_target`, `value_unparseable`,
`htmlcut_no_match`, `htmlcut_ambiguous_match`, `htmlcut_missing_attribute`,
`htmlcut_match_index_out_of_range`, or `htmlcut_internal_failure`. Permanent errors use the closed
HTML and JSON configuration vocabulary documented in [targets.md](targets.md). Both taxonomies are
typed in the public planning API and persisted state, so arbitrary classifications cannot be staged
or stored. Every persisted temporal timestamp is canonical UTC RFC 3339 text.

State schema 9 stores pending records keyed logically by `(event_id, route_id)`. Each record owns
immutable payload bytes, its attempt count, optional last error, and next retry time. A successful
delivery removes it; there is no delivered-record history or `delivered_at` field. State validation
also rejects a pending record whose route was removed or changed to another route family. This
prevents silent loss or re-routing of a stored event. For the current `process_stdin` adapter,
validation also decodes the stored bytes as canonical `ffhn.process_stdin` version-2 JSON and
requires its target, event, route, and family fields to equal the enclosing record. Its typed
`event_key` contains every formula input, including a level-condition `entry_at`; loading
recomputes `event_id = sha256(stable_json({ target_id, route_family, key }))` and rejects any
mismatch. Lifecycle and permanent-error keys also bind the state contract digest. State therefore
cannot turn arbitrary non-empty bytes or a merely hash-shaped invented id into a process delivery,
and its human summary must agree with the structured event facts.

A live state/outbox commit is complete only after staged state bytes and the installed replacement
have been synchronized; on Unix FFHN also synchronizes the directory metadata that names the
replacement. Any failure at that boundary prevents process delivery.

A drain considers records due when that drain begins. A failed record's retry time is based on the
retry-state commit time, and the newly scheduled retry waits for a later drain even if persistence
takes longer than its configured backoff.

Run and reset reports distinguish durable completion from an externally completed process whose
outbox update could not be persisted: `delivered_uncommitted`, `retry_uncommitted`, and
`dead_letter_uncommitted` retain the event, route, attempt count, and cause. Such a record can be
retried, including a possible duplicate after `delivered_uncommitted`. `outbox_error` records an
outbox failure that halted delivery processing before it could complete.

Reset holds the target lock and deletes FFHN-owned state without decoding it. Target configuration
remains in place for a fresh initialization. When a target has a valid `on_run` route, reset creates
a fresh schema-9 state only to enqueue and deliver its one reset event; otherwise the storage root
remains absent.
