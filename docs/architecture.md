---
afad: "4.0"
domain: ARCHITECTURE
updated: "2026-08-25"
route:
  keywords: [architecture, observation graph, lineage, source, measurement, outbox, agent]
  questions: ["what does FFHN core own?", "where is graph state stored?", "what are the source and measurement boundaries?"]
---

# Architecture

`ffhn-core` owns the observation graph: graph/source/measurement configuration, complete source acquisition, typed scalar projection, exact policy decisions, lineage, crash-atomic storage, emitter-owned outboxes, delivery, and scheduling. `ffhn-cli` owns command parsing, structured-document rendering, and process exit codes. `xtask` owns repository verification and release tooling.

```text
graph lease
  └── source turn under one source lock
        ├── acquire one complete HTTP or file representation
        │     └── project eligible measurements serially
        │           └── typed policy, lifecycle events, measurement outbox
        └── drain source and measurement outboxes from immutable snapshots
```

## Ownership boundaries

A source owns representation identity, acquisition health, conditional validators or file digest, schedule, source integration faults, source lifecycle events, and the source outbox. A measurement owns one scalar projection, its declared type, exact policy state, extraction health, measurement integration faults, condition and lifecycle events, and a measurement outbox. Measurements under one source share acquisition bytes but never share policy or lifecycle state.

Source identity is the sole lineage authority. Graph, source, and measurement instance identifiers are random UUIDv4 values. Configuration presence does not mint or prune lineage. The first successful projection creates a measurement identity and state atomically; reset mints fresh lineage and never derives it from the artifacts being replaced.

## Storage and commits

The fixed graph-root hierarchy is opened through no-follow capability-scoped directory handles. Every normal source operation resolves a lineage manifest, resolves a normal commit manifest, and applies the lineage gate before opening state or outbox records. A foreign, missing, or unreadable lineage-dependent artifact is refused at its owned scope; it is never silently adopted or repaired.

Normal state and outbox updates are staged and synchronized before a durable `ffhn.commit_manifest` becomes the commit point. Recovery verifies lineage, generation, paths, and prior/result digests before applying each operation idempotently. Delivery can begin only after the complete generation is installed.

Lineage transitions use `ffhn.lineage_manifest` and a fixed implementation-owned tombstone slot. Source reset replaces the complete source storage lineage; measurement reset replaces only one measurement subtree and identity entry. There are no migrations, compatibility readers, epoch counters, or operator-controlled deletion paths.

## Scheduling and delivery

The agent holds one graph lease. `--jobs` bounds parallelism across sources; each source remains serialized. Acquisition and drain are independent capabilities with independent deferral clocks, so invalid configuration or an unreachable origin cannot suspend a reachable pending record, and no withdrawn capability hot-loops.

Events are route-independent facts. Their identities bind graph/source/measurement lineage and typed condition or episode facts, never wall-clock time. Each admitted delivery record snapshots the envelope, adapter, secrets by environment reference, and retry policy. Retries and jitter derive only from immutable record identity and the attempt number.

The executable source-structure contract in [`tooling/rust-source-shape-policy.toml`](../tooling/rust-source-shape-policy.toml) assigns every maintained Rust file an owner, dependency boundary, and size budget. `cargo xtask structure check` enforces those boundaries.
