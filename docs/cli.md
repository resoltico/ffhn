---
afad: "4.0"
domain: CLI
updated: "2026-07-15"
route:
  keywords: [CLI, run, status, reset, JSON, HTML, typed measurement]
  questions: ["how do I run a target?", "how do I reset typed state?", "what does the CLI emit?"]
---

# CLI Contract

`ffhn-cli` renders v2 core documents. JSON and JSON-pretty are machine interfaces; summary
currently renders the same content in pretty JSON.

| Command | Result document | Purpose |
| --- | --- | --- |
| `ffhn run --target <id>` | `ffhn.run_report` | acquire and type one measurement |
| `ffhn run --target <a> --target <b>` | `ffhn.batch_run_report` | bounded parallel measurement batch |
| `ffhn run --all` | `ffhn.batch_run_report` | run immediate target directories |
| `ffhn status --target <id>` | `ffhn.status_report` | inspect accepted v2 state |
| `ffhn reset --target <id>` | `ffhn.reset_report` | blind-delete FFHN-owned storage |

## Run

```text
ffhn run (--target <ID>... | --all) [--watch-root <PATH>] [--jobs <N>] [--dry-run] [--format <FORMAT>]
```

`--watch-root` defaults to `watchlist`. `--all` discovers immediate directories containing
`target.toml`; `--jobs` must be positive. `--dry-run` fetches, projects, parses, and stages policy
without writing state or invoking delivery. A valid live observation writes
`<target>/.ffhn/state.json` atomically; source-suspect and permanent-error runs may also write it
solely to commit health or episode facts. Every live commit then attempts due durable deliveries.

`initialized`, `changed`, `unchanged`, and `skipped_disabled` exit zero. Acquisition, typed
parsing, contract-refusal, persistence, retry-scheduled delivery, dead-lettered delivery, and
outbox overflow exit one while still writing a structured report. CLI misuse exits two; fatal
failures before a document exits three.

## Status and reset

```text
ffhn status --target <ID> [--watch-root <PATH>] [--format <FORMAT>]
ffhn reset --target <ID> [--watch-root <PATH>] [--format <FORMAT>]
```

Status waits for a stable lock view and reports `pending`, `ready`, or structured invalid or
unavailable state. Reset acquires the same target lock, does not inspect stored artifacts, deletes
FFHN-owned storage, and leaves `target.toml` ready for a fresh initialization. If the still-valid
target has an `on_run` route, reset creates fresh state only to enqueue and attempt one durable
`reset` event; the reset report contains its delivery evidence and exits one when that evidence
contains a delivery problem. This includes an `outbox_error`: reset is already complete, but its
new reset-event outbox work could not complete durably.
