---
afad: "4.0"
domain: CLI
updated: "2026-07-18"
route:
  keywords: [CLI, run, status, reset, JSON, HTML, typed measurement]
  questions: ["how do I run a target?", "how do I reset typed state?", "what does the CLI emit?"]
---

# CLI Contract

`ffhn-cli` renders v2 core documents. `json` and `json-pretty` are machine interfaces. `summary`
is a distinct human-readable text view: a run summary shows the typed observation and prior value,
every condition decision with its trigger, hysteresis state, and reference evidence, staged events,
the durable-before and staged-after lifecycle snapshots, state persistence, and every structured diagnostic fact. That includes native I/O classes, typed
HTTP and file acquisition evidence, HTMLCut error class, optional primary diagnostic code, boundary,
parser, and diagnostic-detail evidence,
integration-fault codes, and delivery evidence. For delivery-process evidence it renders every closed
terminal, writer, stderr-status, stderr-metadata, and derived-primary fact, but never retained stderr
text. Text containing control characters is rendered as a JSON string so each summary fact remains on
one line. Batch summaries retain that complete run view for each target. Summary is intentionally not
a parsing interface.

`status` reads any persisted state under the target's shared lock before exposing its lifecycle
snapshot. It first validates the target apart from projection syntax, then verifies the current
state envelope and digest, and only then checks projection syntax. A projection-invalid target can
therefore return `invalid_config` together with verified durable lifecycle facts; unreadable,
stale, or digest-mismatched state never appears in status output.

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
`<target>/.ffhn/state.json` atomically; source-suspect, permanent-error, and integration-fault
runs may also write it solely to commit health or episode facts. Every live commit then attempts due
durable deliveries.

`initialized`, `changed`, `unchanged`, and `skipped_disabled` exit zero. Acquisition, typed
parsing, contract-refusal, integration faults, persistence, retry-scheduled delivery, dead-lettered
delivery, and outbox overflow exit one while still writing a structured report. CLI misuse exits
two; fatal failures before a document exits three.

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
contains a delivery problem. This includes an `outbox_error_detail`: reset is already complete, but its
new reset-event outbox work could not complete durably.
