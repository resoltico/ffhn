---
afad: "4.0"
domain: REPORTS
updated: "2026-08-25"
route:
  keywords: [measure report, source status, agent tick, event envelope, outbox]
  questions: ["what does measure report?", "how do I inspect source health?", "which facts require routes?"]
---

# Observation-Graph Reports

Every v11 operation emits a version-1 JSON document. `json` and `json-pretty` preserve the document; `summary` renders the same facts for people. Reports are operation evidence, not delivery payloads.

| Schema | Command | Main facts |
| --- | --- | --- |
| `ffhn.new_report` | `new source`, `new measurement` | created configuration kind and source/measurement identity |
| `ffhn.measure_report` | `measure` | source outcome; per-measurement status, policy decisions, staged event envelopes, and outbox-overflow facts |
| `ffhn.agent_tick_report` | `agent tick`, each completed `agent run` tick | one source turn per configured source, independent acquisition/drain facts, and deferral boundaries |
| `ffhn.agent_status_report` | `agent status` | stable status reports for all sources |
| `ffhn.source_status_report` | `status` | source lineage/config status, generation, schedule, health, faults, and measurement status |
| `ffhn.measurement_status_report` | `status --measurement` | one configured, authoritative, or artifact-backed measurement status |
| `ffhn.validate_report` | `validate` | offline source and measurement validation facts |
| `ffhn.list_report` | `list` | stable source or configured measurement listing |
| `ffhn.reset_report` | `reset` | reset source and optional measurement scope |

## Measurement report

`ffhn.measure_report` is route-independent. A measurement with no `[outbox]` or `[[routes]]` still reports typed policy evaluations, reference evidence, event envelopes that policy or lifecycle staged, and candidates rejected because its bounded outbox was full.

Every measurement result carries a closed status such as `initialized`, `changed`, `unchanged`, `not_modified`, `extraction_failed`, `integration_fault`, `quarantined`, `lineage_held`, `config_invalid`, or `disabled`. A successful result includes the complete accepted typed observation and its raw/comparison/parser/HTMLCut evidence. Policy evaluation records the condition id, exact outcome, trigger decision, active-before/active-after hysteresis facts, and resolved or unavailable named-reference evidence.

Source results retain configuration errors, unresolvable manifest class, lineage-refusal reason, acquisition health, source integration-fault episode, and the current failure’s typed `kind` plus `reason_class` when applicable. Measurement results retain configuration or lineage-hold reason, stored/current MVD evidence for quarantine, extraction health, and measurement integration-fault episodes.

`ffhn.event_envelope` has a deterministic `event_id` derived only from its typed event key. The key binds graph, source, and measurement lineage where applicable, definition and observation or episode facts, and excludes wall-clock time. Envelope evidence records its commit time and display facts but does not influence identity. HTMLCut remains a pinned upstream extraction dependency; FFHN reports only its validated plain DOM text, rendered-text, or attribute projection facts rather than exposing an HTMLCut structured payload.

## Status report

`status` acquires a shared source lock and observes only a fully installed generation. A present or unreadable lineage or commit manifest is `pending`; a source lineage mismatch is `lineage_refused`; a never-run configured source is `uninitialized`.

Ready source status includes durable source acquisition health, any source integration-fault episode, generation, and next due UTC time. Measurement status distinguishes `never_initialized`, `ready`, `config_invalid`, `quarantined`, `lineage_held`, and `not_configured`; it retains accepted-observation sequence, stored/current MVDs, exact hold reason, extraction health, and any measurement integration-fault episode where those facts are authoritative.

`ffhn.agent_tick_report` embeds the complete acquisition report for every attempted source and reports source/measurement drain dispositions separately. Every active in-memory acquire or drain deferral carries both its UTC boundary and a closed reason such as `lock_contention`, `config_invalid`, `lineage_refused`, `delivery_unreachable`, or `unreadable`.

A source reset that discards an unresolvable lineage or commit manifest reports its class and exact opaque bytes as Base64 when the fixed manifest entry was readable. `bytes_unavailable = true` records the narrower case where reset remained safe but evidence capture could not read the artifact. Measurement reset never discards a source-scoped manifest.

## Delivery evidence

Delivery records are owned by their emitting source or measurement. A record snapshots its event envelope, adapter configuration, and retry policy. The agent drains only from that snapshot, so a later route edit, measurement quarantine, or configuration removal cannot reroute existing delivery. Success removes a record; exhausted attempts become a `ffhn.dead_letter`; reset drops only records in the reset scope.

Outbox admission preserves declaration priority and never evicts older pending records. A rejected new admission is reported as `outbox_overflow` with event kind, optional condition id, route id, and route family; it does not erase state or an existing record. Retry timing is deterministic from immutable record identity, attempt, and policy, based on the retry-state transition time after the failed adapter attempt completes.
