---
afad: "3.5"
version: "2.0.0"
domain: CLI
updated: "2026-04-20"
route:
  keywords: [cli, run command, status command, watch-root discovery, exit codes, stdout json]
  questions: ["what does ffhn run emit?", "how does ffhn --all discover targets?", "which exit codes does the ffhn CLI use?"]
---

# CLI Contract

`ffhn-cli` is a thin renderer over `ffhn-core`. It does not implement a second monitoring engine.

## Commands

<!-- contract:cli-catalog:start -->
| Command | Structured stdout document | Notes |
| --- | --- | --- |
| `ffhn run --target <id>` | `ffhn.run_report` | single-target execution |
| `ffhn run --target <a> --target <b>` | `ffhn.batch_run_report` | explicit multi-target batch |
| `ffhn run --all` | `ffhn.batch_run_report` | watch-root discovery |
| `ffhn status --target <id>` | `ffhn.status_report` | status inspection; valid targets may create `lock/run.lock` |

The maintained help text is:

1. `run`: Run one or more configured targets once.
2. `status`: Read one target's current machine-readable status.

`run` supports:

1. `--watch-root <PATH>`: Watch-root directory containing per-target subdirectories. Default: `watchlist`.
2. `--target <ID>`: One or more target ids under the watch root.
3. `--all`: Run every target directory discovered under the watch root.
4. `--jobs <N>`: Maximum concurrent target runs. Default: `1`.
5. `--dry-run`: Run validation, fetch, extraction, and comparison under the shared run lock without live state/report mutations.

Execution modes:

1. `live`: Validation, fetch, extraction, comparison, persistence, and notifications.
2. `dry_run`: Validation, fetch, extraction, and comparison under the shared run lock without live state/report mutations.

Hard limitations:

1. `--jobs` must be a positive integer; `0` is invalid CLI usage.
2. Repeated `--target` values must be unique within one request.
3. `--all` only discovers immediate subdirectories of the watch root.
<!-- contract:cli-catalog:end -->

## Watch-Root Discovery Rules

`run --all` walks the immediate subdirectories of the watch root, sorts them lexicographically, and then decides whether to include each directory in the batch request list.

One directory is included when:

1. its `target.toml` validates and `enabled = true`
2. its `target.toml` does not validate at all

One directory is excluded when:

1. its `target.toml` validates and `enabled = false`

This means `run --all` still surfaces invalid target directories as batch failures instead of silently dropping them.

If watch-root traversal hits a filesystem error after discovery starts, FFHN exits fatally instead of silently skipping the broken entry.

If the watch root does not exist, `run --all` emits an empty `ffhn.batch_run_report`.

## Stdout And Stderr

On every structured success or structured run failure, the CLI writes exactly one JSON document to stdout and nothing else.

Stderr is reserved for fatal process-level failures and CLI-usage errors, such as:

1. argument parse failures
2. filesystem errors before a structured report can be emitted
3. JSON-rendering failures while writing the final document

## Exit Codes

| Condition | Exit code |
| --- | ---: |
| successful run, dry-run, batch, or status | `0` |
| structured `failed_transient`, `failed_permanent`, or batch with any failed/fatal entries | `1` |
| CLI misuse such as parse errors or invalid `--jobs` | `2` |
| fatal process-level error before structured document emission | `3` |

`status` either exits `0` with an `ffhn.status_report` or exits `3` if FFHN could not emit a structured status document.
