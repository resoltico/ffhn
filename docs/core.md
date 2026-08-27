---
afad: "4.0"
domain: CORE
updated: "2026-08-25"
route:
  keywords: [graph, source, measurement, lineage, acquisition, outbox]
  questions: ["what does one source cycle do?", "what does dry run change?", "when is delivery allowed?"]
---

# Observation-Graph Core Semantics

FFHN v11 is an observation graph. A graph owns an immutable random `graph_id`; every source owns an independently minted `source_instance_id`; every measurement gets a `measurement_instance_id` only when its first state is atomically created or when it is explicitly reset.

```text
source acquisition
  └── one complete in-memory representation
        └── selected measurements: projection → typed value → policy → event/outbox
              └── agent: independent acquire and drain capabilities
```

One source cycle acquires one complete HTTP or file representation, then evaluates each eligible configured measurement against that same in-memory document. A measurement owns its projection, typed value contract, temporal policy state, extraction health, integration-fault episode, event identity, and measurement outbox. A source owns acquisition health, source integration faults, conditional validators or file digest, source schedule, source outbox, and per-source generation.

## Lineage and storage

The graph-root hierarchy is fixed and opened without following filesystem links. Source identity is authoritative; source and measurement state, delivery records, and manifests must match it exactly. A mismatch is `lineage_refused`, never silent repair.

Normal source work first resolves any lineage transition manifest, then any normal commit manifest, then applies the lineage gate. State and outbox changes are staged, synchronized, described by a durable commit manifest, installed idempotently, and only then become eligible for delivery. Event IDs include lineage and typed event facts but no wall-clock value.

`reset --source` and `reset --measurement` are clean lineage transitions. They mint fresh random identity, discard only the selected owned state/outbox scope, and never migrate old state. A source reset is safe even when old authority or storage is corrupt because it does not derive new lineage from those artifacts.

## Acquisition and typed policy

HTTP accepts only complete `200` or `203` representations; a direct validator-matched `304` is `not_modified`. Other `2xx` statuses are not representations. Conditional validators are sent only to the exact issuing source URL. File sources read fresh bytes every cycle. HTTP redirects are manually bounded, never downgrade HTTPS to HTTP, and strip extensible configured headers and secrets on origin changes.

JSON Pointer and HTMLCut projection select one scalar. HTMLCut plans are prepared once from configuration and executed once per HTML acquisition. `html_text` is plain DOM descendant text; `html_rendered_text` is semantic rendered text; `html_attribute` selects a named attribute. FFHN parses values under explicit type contracts and evaluates policy using only persisted pre-observation references. Decimal and money comparisons are exact.

## Dry runs, health, and delivery

Dry measurement takes a shared lock and executes the same configuration, lineage, acquisition, projection, parsing, and policy path. It never recovers a manifest, creates lineage, writes state, admits outbox records, or invokes adapters.

Acquisition failures update source health; extraction failures update only the affected measurement health. Source and measurement integration faults are independent, code-keyed episodes. Escalation and integration events are materialized route-independently and admitted to their owner’s outbox only when routes are configured.

Outbox records snapshot the event envelope, adapter, and retry policy. The agent drains snapshots independently from acquisition: invalid current configuration, quarantined measurements, and removed measurement configuration do not invalidate reachable pending records. Retry timing is deterministic from record identity, attempt count, and snapshot policy.
