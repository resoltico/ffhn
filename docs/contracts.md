---
afad: "4.0"
domain: CONTRACTS
updated: "2026-08-27"
route:
  keywords: [contracts, schema versions, graph identity, source state, measurement state, reports, reset]
  questions: ["which FFHN contracts exist?", "what is the persisted graph layout?", "what does reset replace?"]
---

# Contract Inventory

FFHN 11 is a clean observation-graph contract. Every current FFHN-owned serialized document is schema version 1; readers reject any other schema identity or unknown field. FFHN provides no compatibility reader, migration command, alias, or transitional document.

## Configuration and identity

| Document | Location |
| --- | --- |
| `ffhn.agent` | `<graph-root>/agent.toml` |
| `ffhn.graph_identity` | `<graph-root>/.ffhn-graph.json` |
| `ffhn.source` | `sources/<source-id>/source.toml` |
| `ffhn.source_identity` | `sources/<source-id>/.ffhn-identity.json` |
| `ffhn.measurement` | `sources/<source-id>/measurements/<measurement-id>/measurement.toml` |

The graph identity contains the immutable `graph_id`. Source identity is the sole lineage authority and contains one `source_instance_id` plus the authoritative map of measurement instance identities. Every identifier is validated against its directory name; instance identifiers must be UUIDv4.

## Durable state and delivery

| Document | Location |
| --- | --- |
| `ffhn.source_state` | `sources/<source-id>/.ffhn/source-state.json` |
| `ffhn.measurement_state` | `sources/<source-id>/.ffhn/measurements/<measurement-id>/state.json` |
| `ffhn.delivery_record` | source or measurement `outbox/<event-id>--<route-id>.json` |
| `ffhn.dead_letter` | source or measurement `dead-letters/<event-id>--<route-id>.json` |
| `ffhn.commit_manifest` | `sources/<source-id>/.ffhn/commit.manifest` while a normal commit is in progress |
| `ffhn.lineage_manifest` | `sources/<source-id>/.ffhn-lineage.manifest` while initialization or reset is in progress |

Every lineage-dependent document must match the authoritative source and measurement instance identities exactly. Source state owns the source generation, acquisition health, source integration-fault episode, validators or file digest, and durable schedule. Measurement state owns the accepted typed observation, observation sequence, extraction health, measurement integration-fault episode, condition state, measurement value digest, and policy revision.

A normal generation is visible only after its synchronized staged files and identity additions are installed through a validated commit manifest. FFHN synchronizes staged file payloads on every maintained platform; on Unix-like platforms it also synchronizes the parent directory after atomic replacement. Windows has no equivalent directory-handle flush through the capability API used by FFHN, so Windows preserves atomic replacement and file synchronization without claiming a directory-fsync guarantee. A delivery record contains immutable envelope, adapter, secret-reference, and retry-policy snapshots plus append-only attempt bookkeeping. Successful delivery removes it; exhaustion atomically replaces it with a dead letter.

## Reports and events

| Document | Command or role |
| --- | --- |
| `ffhn.new_report` | `new source`, `new measurement` |
| `ffhn.measure_report` | `measure` |
| `ffhn.agent_tick_report` | `agent tick` and each completed `agent run` tick |
| `ffhn.agent_status_report` | `agent status` |
| `ffhn.source_status_report` | `status --source` |
| `ffhn.measurement_status_report` | `status --source --measurement` |
| `ffhn.reset_report` | `reset` |
| `ffhn.validate_report` | `validate` |
| `ffhn.list_report` | `list` |
| `ffhn.event_envelope` | immutable route-independent event payload |

Reports expose policy decisions, lifecycle events, source and measurement health, integration-fault episodes, lineage/quarantine status, and outbox overflow independently of configured delivery routes. Every acquisition failure carries a closed failure kind and reason class.

An event ID is the SHA-256 digest of its stable typed event key. The key includes `graph_id`, source lineage, measurement lineage where applicable, and condition-definition/observation or episode facts. It excludes wall-clock time and routes. Delivery receivers must deduplicate on `event_id`.

## Digest and reset boundaries

The Source Representation Digest contains representation-affecting acquisition configuration and nonsecret secret references. The Measurement Value Digest combines it with projection, declared type, parser/value semantics, and HTMLCut semantics for HTML projections. An MVD mismatch quarantines only that measurement until reset while reachable queued delivery continues from its snapshots.

`reset --source` mints a fresh source instance, installs fresh source state, and replaces the complete source-owned storage tree. `reset --measurement` mints a fresh measurement instance and replaces only that measurement subtree. Reset never interprets replaced artifacts, never migrates them, never deletes the source lock or configuration, and uses only the fixed reserved tombstone path.
