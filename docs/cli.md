---
afad: "4.0"
domain: CLI
updated: "2026-08-25"
route:
  keywords: [CLI, graph root, source, measurement, agent, reset]
  questions: ["how do I create a graph?", "how do I measure one source?", "what does reset replace?"]
---

# CLI Contract

FFHN operates on a graph root. A graph contains sources, and each source contains one acquisition contract and zero or more measurement contracts.

`json` and `json-pretty` are machine interfaces. `summary` is human-oriented text derived from the same structured document and is not a parsing interface.

| Command | Result document | Purpose |
| --- | --- | --- |
| `ffhn new source` | `ffhn.new_report` | initialize an empty graph when needed and create source TOML only |
| `ffhn new measurement` | `ffhn.new_report` | create measurement TOML only |
| `ffhn validate` | `ffhn.validate_report` | validate configuration offline |
| `ffhn list` | `ffhn.list_report` | list configured sources or measurements |
| `ffhn measure` | `ffhn.measure_report` | acquire one source and evaluate its measurements |
| `ffhn status` | `ffhn.source_status_report` | inspect a source’s stable lineage, health, and measurement facts |
| `ffhn reset` | `ffhn.reset_report` | mint fresh source or measurement lineage |
| `ffhn agent tick` | `ffhn.agent_tick_report` | execute one finite scheduled turn |
| `ffhn agent status` | `ffhn.agent_status_report` | inspect every configured source |

## Configuration workflow

```text
ffhn new source --source <ID> [--graph-root <PATH>]
ffhn new measurement --source <ID> --measurement <ID> [--graph-root <PATH>]
ffhn validate (--source <ID> | --all) [--graph-root <PATH>] [--format <FORMAT>]
ffhn list (--sources | --measurements) [--graph-root <PATH>] [--format <FORMAT>]
```

`new source` creates a disabled, file-backed template and an immutable graph identity only when the requested graph root is absent or empty. `new measurement` creates a disabled, text JSON-Pointer template. Neither command creates source or measurement lineage, state, outbox records, or delivery attempts.

`validate` performs both delivery and source/measurement contract validation without fetching, reading state, invoking adapters, or changing lineage. It exits one when any checked source or measurement is invalid.

## Measurement and status

```text
ffhn measure --source <ID> [--measurement <ID>]... [--graph-root <PATH>] [--jobs <N>] [--dry-run] [--format <FORMAT>]
ffhn status --source <ID> [--measurement <ID>] [--graph-root <PATH>] [--format <FORMAT>]
```

`measure` acquires one complete source representation and evaluates every selected configured measurement against the same in-memory document. Measurement selection is exact: an unknown or duplicate selected id is rejected. Live measurement holds the source writer lock and commits accepted state, health episodes, event envelopes, and admitted outbox records atomically before delivery can drain.

`--dry-run` takes a shared lock and executes the same configuration, lineage, acquisition, projection, parsing, and policy path without recovering manifests, minting lineage, writing state, admitting outbox records, or invoking delivery.

`status` takes a shared source lock with bounded retry. It never recovers a manifest: a present or unreadable manifest is reported as `pending` with its class. Status distinguishes invalid source/measurement configuration, source and measurement lineage reasons, MVD quarantine with stored/current digests, never-initialized and removed measurements, source acquisition health, and scoped integration-fault episodes. These facts do not depend on routes.

## Agent and reset

```text
ffhn agent run|tick|status [--graph-root <PATH>] [--jobs <N>] [--format <FORMAT>]
ffhn reset --source <ID> [--measurement <ID>] [--graph-root <PATH>] [--format <FORMAT>]
```

`agent tick` claims the graph lease, evaluates due source acquisition independently from source and measurement outbox draining, and reports both facts. `--jobs` bounds concurrency across sources only; measurements within one source remain serialized, and report order is stable by source id. `agent run` retains the lease until a handled termination signal, finishes already-started source turns, and sleeps interruptibly until the earliest permitted source due time, retry time, or in-memory deferral boundary. A second agent returns exit code 4.

`reset --source` is blind and mint-only: it creates fresh source lineage, removes every source-owned measurement state and outbox under the fixed graph layout, and never interprets old state as migration input. `reset --measurement` creates fresh lineage and state only for the named measurement while preserving source and sibling lineage. There is no migration command or compatibility path.

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | successful command, including initialized, changed, unchanged, not-modified, disabled, or skipped-locked source outcomes |
| 1 | handled structured failure, such as invalid configuration, pending or refused lineage, MVD quarantine, extraction/acquisition failure, integration fault, delivery retry/dead letter, or acquisition hold |
| 2 | CLI misuse |
| 3 | fatal failure before a structured document can be emitted |
| 4 | source busy or graph agent already running |
