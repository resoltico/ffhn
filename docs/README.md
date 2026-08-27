---
afad: "4.0"
domain: DOCS
updated: "2026-08-25"
route:
  keywords: [documentation index, observation graph, source, measurement, policy, agent]
  questions: ["where is the FFHN documentation?", "how do I configure and operate FFHN?"]
---

# FFHN Documentation

- [getting-started.md](getting-started.md): create and measure a local graph from zero
- [targets.md](targets.md): source and measurement configuration, projections, typed values, and routes
- [cli.md](cli.md): commands, output formats, locking, reset, and exit behavior
- [reports.md](reports.md): operation reports, event envelopes, health, and delivery evidence
- [architecture.md](architecture.md): graph ownership, lineage, commits, agent scheduling, and outboxes
- [core.md](core.md): acquisition, policy, dry-run, health, and delivery semantics
- [contracts.md](contracts.md): current schema inventory, durable layout, digests, and reset boundaries
- [quality-gates.md](quality-gates.md): maintained verification commands
- [operations.md](operations.md): maintainer operations and release preparation

The current document set describes one clean observation-graph product: shared source acquisition, independently typed measurements, exact policy decisions, crash-atomic lineage/state, durable snapshot delivery, and a bounded-concurrency agent.
