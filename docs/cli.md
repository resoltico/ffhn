---
afad: "4.0"
version: "5.0.0"
domain: CLI
updated: "2026-05-03"
route:
  keywords: [cli, run command, status command, watch root discovery, exit codes, stdout json]
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
| `ffhn run --all` | `ffhn.batch_run_report` | watch root discovery |
| `ffhn status --target <id>` | `ffhn.status_report` | status inspection; valid targets may create `lock/run.lock` |

The maintained help text is:

1. `run`: Run one or more configured targets once.
2. `status`: Read one target's current machine-readable status.

`run` supports:

1. `--watch-root <PATH>`: Watch root directory containing per-target subdirectories. Default: `watchlist`.
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
3. `--target <ID>` must satisfy FFHN's durable `target_id` contract before any filesystem work begins.
4. `--all` only discovers immediate subdirectories of the watch root.
<!-- contract:cli-catalog:end -->

Angle-bracket fragments in the catalog above are metavariables that describe the command shape. They are not literal tokens to type.
This doc set uses "watch root" for the filesystem concept and keeps the literal CLI flag spelling as `--watch-root`.

Clap also exposes a built-in help subcommand in top-level help output, but FFHN's structured JSON command contract consists of `run` and `status`.

## Top-Level Help And Version

Without a subcommand, `ffhn` prints the version banner followed by long help to stdout and exits `0`.

`ffhn --help` and the built-in help subcommand emit that same top-level help output. When top-level help and version flags are combined, help wins and FFHN still prints the single-source version banner above the long help.

That long help includes the `run` and `status` operations, Clap's built-in `help` subcommand, and the standard `--help` / `--version` options.

`ffhn --version` prints the version banner from package metadata:

1. the tool name followed by the workspace version
2. the maintained product description line

`--version` is a top-level flag. Passing it after `run`, `status`, or an unrecognized subcommand is CLI misuse and exits `2`.

## Watch Root Discovery Rules

`run --all` walks the immediate subdirectories of the watch root, sorts them lexicographically, and then decides whether to include each directory in the batch request list.

One directory is included when:

1. it contains a `target.toml` path, that target validates, and `enabled = true`
2. it contains a `target.toml` path, but the directory label or target document violates FFHN's contract

One directory is excluded when:

1. it does not contain a `target.toml` path at all
2. its `target.toml` validates and `enabled = false`

This means `run --all` still surfaces invalid target directories as batch failures instead of silently dropping them.
When the discovered directory name itself violates FFHN's durable `target_id` rules, FFHN keeps that raw directory label in `requested_targets` and emits a per-entry `fatal_error.kind = contract` instead of rewriting or ignoring it.

Live explicit runs on disabled targets return `skipped_disabled`. Dry-run is intentionally different: `run --target <id> --dry-run` still validates, fetches, extracts, and compares an explicitly named disabled target, while `run --all` continues to exclude valid disabled directories before batch execution starts.

If watch root traversal hits a filesystem error after discovery starts, FFHN exits fatally instead of silently skipping the broken entry.

If the watch root does not exist or is not a directory, `run --all` exits fatally instead of emitting an empty batch report.

## Stdout And Stderr

On every structured success or structured run failure, the CLI writes exactly one JSON document to stdout and nothing else.

That includes live runs whose content outcome was otherwise successful but whose final `last_run.json` write failed. In that case stdout still carries the `ffhn.run_report`, and the CLI exits `1` because `persist.error` is populated.

That also includes notification delivery failures. The run report stays structurally successful when the monitored content path succeeded, but `notifications[].delivered = false` still makes the CLI exit `1`.

Stderr is reserved for fatal process-level failures and CLI-usage errors, such as:

1. argument parse failures
2. filesystem errors before a structured report can be emitted, such as unreadable `target.toml` paths or a missing watch root
3. JSON-rendering failures while writing the final document

## Exit Codes

| Condition | Exit code |
| --- | ---: |
| successful or `skipped_disabled` single-target runs, dry-runs, batches with no failed/fatal/persist/notification-delivery failures, or status | `0` |
| structured `failed_transient`, `failed_permanent`, live run report with `persist.error`, any run report with failed notification delivery, or batch with any failed/persist-error/fatal/notification-delivery-failed entries | `1` |
| CLI misuse such as parse errors or invalid `--jobs` | `2` |
| fatal process-level error before structured document emission | `3` |

`status` either exits `0` with an `ffhn.status_report` or exits `3` if FFHN could not emit a structured status document.
