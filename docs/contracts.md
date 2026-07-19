---
afad: "4.0"
domain: CONTRACTS
updated: "2026-07-19"
route:
  keywords: [contracts, schema versions, target, state, reports, reset]
  questions: ["which v2 contracts exist?", "what is the persisted layout?", "what does reset delete?"]
---

# Contract Inventory

The current measurement, policy, and durable-delivery contract surface has these document
identities:

| Document | Version | Location or output |
| --- | ---: | --- |
| `ffhn.target` | 12 | `<target>/target.toml` |
| `ffhn.state` | 17 | `<target>/.ffhn/state.json` |
| `ffhn.run_report` | 17 | single-target run stdout |
| `ffhn.batch_run_report` | 17 | batch stdout |
| `ffhn.status_report` | 13 | status stdout |
| `ffhn.reset_report` | 7 | reset stdout |
| `ffhn.process_stdin` | 4 | immutable pending-delivery payload |

An accepted observation is always valid and typed. It carries raw evidence, comparison projection,
an acquisition kind (`json_pointer`, `html_plain_text`, `html_rendered_text`, or `html_attribute`), parser identity and grammar,
declared type parameters, declared-type canonical value, and parse diagnostics. JSON evidence is the
exact selected scalar token, including string quoting and escapes. For `html_text`, raw evidence
is HTMLCut's original plain DOM descendant text; `html_rendered_text` instead carries HTMLCut's
semantic rendered text. When configured, only the matching projection from a detached canonical
clone supplies comparison. `html_attribute` keeps the original CSS match-metadata attribute for
both. HTML observations additionally bind the current HTMLCut extraction-semantics
version, plan digest, positive candidate count, and the public HTMLCut diagnostics. A failed parse
is never a baseline value.

For `declared_type = "text"`, JSON evidence must be a JSON string; numbers, booleans, and `null`
are `value_unparseable` even when encountered in a persisted observation. Its canonical form is the
decoded Unicode scalar sequence, so JSON escape spelling is not comparison identity, while Unicode
normalization is intentionally not applied. HTML text and attributes instead use their configured
comparison projection as that canonical form.

The target digest includes a source-kind tag, target and fetch configuration, projection, declared
type, parser identity and grammar version, type parameters, the complete named-condition
definitions canonicalized by condition identifier, `escalate_after`, and FFHN's policy-evaluation semantics version. That version changes
whenever identical accepted observations could lead to a different condition decision, requiring
reset before temporal state is used under the new semantics. HTML targets additionally include the
HTMLCut extraction-semantics version; JSON targets do not. The HTML projection includes any DOM
canonicalization policy, so a change to that policy changes the measurement identity and requires
reset. It excludes per-execution plan digests and operational delivery policy: routes, queue
capacity, attempts, and retry timing can evolve without changing the measurement identity. A
mismatch is refused; reset is the only lifecycle operation that clears it.

Schema 12 requires an explicit `conditions` list and positive `escalate_after`; it requires a CSS
selector for the DOM-derived `html_text` projection. The policy API remains a pure planning surface.
It returns deterministic condition and immediate run-event eligibilities. The runtime retains that
result with the exact next schema-17 state until the commit
boundary; state owns the committed observation sequence, fixed initial baseline, per-condition
result/active/transition facts, source health, permanent-error episode, integration-fault episode,
and pending-only outbox.
Source-health reasons use the fixed vocabulary: `fetch_failed`, `json_malformed`,
`json_missing_pointer_target`, `json_non_scalar_pointer_target`, `value_unparseable`,
`htmlcut_no_match`, `htmlcut_ambiguous_match`, `htmlcut_missing_attribute`,
`htmlcut_match_index_out_of_range`. Permanent errors use the closed HTML and JSON configuration
vocabulary documented in [targets.md](targets.md). Integration faults use the closed
`htmlcut_internal_error`, `ffhn_boundary_invariant_violation`, and
`ffhn_policy_invariant_violation` vocabulary. All three
taxonomies are typed in the public planning API and persisted state, so arbitrary classifications
cannot be staged or stored. Every persisted temporal timestamp is canonical UTC RFC 3339 text.

Normal state loading first admits only the two-field `ffhn.state` version-17 schema envelope. Any
other decoded envelope is rejected with reset-required guidance before FFHN decodes its state
facts, so legacy field or enum shapes cannot be partially interpreted by the current model.
Malformed JSON remains unreadable state. FFHN neither migrates nor offers a compatibility parser
for any prior state schema. The public state and report value types enforce their own current
schema identity during direct deserialization as well, so a consumer cannot accidentally accept a
retired document by bypassing FFHN's filesystem loader.

State schema 17 stores pending records keyed logically by `(event_id, route_id)`. Each record owns
its adapter-neutral event kind, optional condition id, immutable payload bytes, attempt count,
optional bounded `last_error_detail`, and next retry time. A successful
delivery removes it; there is no delivered-record history or `delivered_at` field. State validation
also rejects a pending record whose route was removed or changed to another route family. This
prevents silent loss or re-routing of a stored event. For the current `process_stdin` adapter,
validation also decodes the stored bytes as canonical `ffhn.process_stdin` version-4 JSON and
requires its target, event, route, and family fields to equal the enclosing record. Its typed
`event_key` contains every formula input, including a level-condition `entry_at`; loading
recomputes `event_id = sha256(stable_json({ target_id, route_family, key }))` and rejects any
mismatch. Lifecycle, permanent-error, and integration-fault keys also bind the state contract
digest. State therefore
cannot turn arbitrary non-empty bytes or a merely hash-shaped invented id into a process delivery,
and its human summary must agree with the structured event facts.

Reports expose lifecycle state separately from policy-event evidence. A run report's
`lifecycle.before` is the complete durable source-health, permanent-error-episode, and
integration-fault-episode snapshot safely read under the target lock before execution.
`lifecycle.after` is the complete staged successor when the run transitions state. `state_persisted`
means only that the staged write committed, so dry runs and failed commits retain their staged
`after` snapshot while reporting `false`. Disabled or no-op runs expose their durable `before` when
available and no `after`. Status takes the shared target lock after target validation apart from
projection syntax, performs state envelope, self-validation, and digest checks while holding it,
then exposes the current durable lifecycle before checking projection syntax. This permits a
base-valid but projection-invalid target to return `invalid_config` with verified lifecycle facts;
unreadable, stale, or digest-mismatched state is never exposed.

A live state/outbox commit is complete only after staged state bytes and the installed replacement
have been synchronized; on Unix FFHN also synchronizes the directory metadata that names the
replacement. Any failure at that boundary prevents process delivery.

A drain considers records due when that drain begins. A failed record's retry time is based on the
retry-state commit time, and the newly scheduled retry waits for a later drain even if persistence
takes longer than its configured backoff.

Run and reset reports distinguish durable completion from an externally completed process whose
outbox update could not be persisted: `delivered_uncommitted`, `retry_uncommitted`, and
`dead_letter_uncommitted` retain the event, route, attempt count, and typed cause. A
`delivered_uncommitted` outcome is an outbox-persistence problem, not a process-delivery failure:
the child already accepted the payload and a later retry may duplicate it. A diagnostic
always has a closed `kind` and `operation`. Every `io` diagnostic carries exactly one typed cause:
a native operating-system failure carries a closed `io_error_class` instead of rendered foreign
error prose, while HTTP status, configured body-limit, and UTF-8 acquisition failures carry typed
`fetch_failure` facts. JSON, TOML, URL,
time, numeric, and Semantic Version failures likewise translate to FFHN-owned explanatory
messages, never upstream parser rendering. Its explanatory message does not repeat its
classification. If FFHN must bound a message to 1,024 UTF-8 bytes, the retained prefix remains in
`message` and typed `message_truncation` carries the original byte count and SHA-256 digest; FFHN
never writes a truncation marker into the payload string. Delivery-process evidence
exists only on a `delivery`/`delivery_process` diagnostic; HTMLCut and integration-fault evidence
have their respective HTML-extraction and policy-evaluation owners. Every `policy_invariant`
diagnostic therefore carries `ffhn_policy_invariant_violation`; every `htmlcut` diagnostic carries
validated HTMLCut failure evidence. HTMLCut details are FFHN's closed projection of the exact
reachable pinned HTMLCut v12 interop shapes; unknown details fail at the boundary instead of being
silently omitted. Failed delivery outcomes carry
a complete `delivery_process` failure, including terminal, writer, stderr, and derived primary
facts. Its `original_len_bytes` count is a canonical decimal string, preserving the exact drained
byte count independently of the platform word size. Successful outcomes can carry a separate
stderr-capture observability detail: a read
failure, an unavailable configured stderr reader, or a reader panic. They are never retried for
that reason. Such a record can be retried, including a possible duplicate after
`delivered_uncommitted`. `outbox_error_detail` records a status-compatible outbox drain or durable
outbox-state failure that halted delivery processing before it could complete.

Persisted source-health evidence is likewise semantically closed: its reason must agree with the
diagnostic's category and operation and must carry no integration-fault code. HTML source-health
reasons additionally require their exact HTMLCut `error_class`, and match-index failures require
the exact `MATCH_INDEX_OUT_OF_RANGE` primary diagnostic code. A fetch, JSON-selection, typed-value
parsing, or HTML-extraction episode therefore cannot store evidence from a different lifecycle or
from a different HTMLCut failure class.

Reset holds the target lock and deletes FFHN-owned state without decoding it. Target configuration
remains in place for a fresh initialization. When a target has a valid `on_run` route, reset creates
a fresh schema-17 state only to enqueue and deliver its one reset event; otherwise the storage root
remains absent.
