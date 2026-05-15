---
afad: "4.0"
domain: CLI
updated: "2026-05-14"
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
| `ffhn status --target <id>` | `ffhn.status_report` | status inspection; valid targets may create `lock/run.lock` and wait behind active live runs |

Per-operation help contract:

### `run`

Summary: Run one or more configured targets once.

Usage: `ffhn run (--target <ID>... | --all) [--watch-root <PATH>] [--jobs <N>] [--dry-run] [--format <FORMAT>]`

Options:

1. `--watch-root <PATH>`: Watch root directory containing per-target subdirectories with target.toml. The path must already exist and be a directory. Default: `watchlist`.
2. `--target <ID>`: One or more target ids under the watch root. Use lowercase letters or digits, with single internal '-' or '_' separators.
3. `--all`: Run each immediate watch-root subdirectory containing target.toml, including disabled targets.
4. `--jobs <N>`: Maximum concurrent target runs. Default: `1`.
5. `--dry-run`: Run validation, fetch, extraction, and comparison under the shared run lock; waits behind active live runs and skips live state/report mutations.
6. `--format <FORMAT>`: Output format: json, json-pretty, or summary. Default: `json`.

Examples:

1. `ffhn run --target demo`
2. `ffhn run --target demo_a --target demo_b`
3. `ffhn run --all --jobs 4`
4. `ffhn run --target demo --dry-run`

Output:

1. `ffhn run --target <ID>` produces one `ffhn.run_report` result in the selected format.
2. Repeated `--target` values or `--all` produce one `ffhn.batch_run_report` result in the selected format.
3. Structured run failures remain on stdout in the selected format; fatal process-level failures are written to stderr and exit `3`.

Operational notes:

1. The watch root must already exist and be a directory.
2. `--all` discovers only immediate watch-root subdirectories containing `target.toml`.
3. Disabled targets are discovered and reported normally, but they are not executed.
4. Explicit `--target` requests whose `target.toml` path is missing or unreadable emit structured `target_unavailable` results instead of raw fatal stderr.

### `status`

Summary: Read one target's current machine-readable status.

Usage: `ffhn status --target <ID> [--watch-root <PATH>] [--format <FORMAT>]`

Options:

1. `--watch-root <PATH>`: Watch root directory containing per-target subdirectories with target.toml. The path must already exist and be a directory. Default: `watchlist`.
2. `--target <ID>`: Target id under the watch root. Use lowercase letters or digits, with single internal '-' or '_' separators.
3. `--format <FORMAT>`: Output format: json, json-pretty, or summary. Default: `json`.

Examples:

1. `ffhn status --target demo`
2. `ffhn status --watch-root ./watchlist --target demo`

Output:

1. Produces one `ffhn.status_report` result in the selected format.
2. Valid-target reports carry top-level `enabled = true|false` so disablement stays separate from baseline readiness.
3. Malformed or contract-invalid target documents use `status.kind = invalid_config` with structured `status.error_detail`.
4. Missing or unreadable explicit target paths use `status.kind = unavailable_target` with structured `status.error_detail`.

Operational notes:

1. The watch root must already exist and be a directory.
2. Status waits behind any active live run so it can inspect one stable target view.


Execution modes:

1. `live`: Validation, fetch, extraction, comparison, persistence, and notifications.
2. `dry_run`: Validation, fetch, extraction, and comparison under the shared run lock; waits behind active live runs and skips live state/report mutations.

Hard limitations:

1. `--jobs` must be a positive integer; `0` is invalid CLI usage.
2. One of `--target <ID>` or `--all` is required.
3. Repeated `--target` values must be unique within one request.
4. `--target <ID>` must satisfy FFHN's durable `target_id` contract before any filesystem work begins.
5. `--all` only discovers immediate subdirectories of the watch root.
<!-- contract:cli-catalog:end -->

Angle-bracket fragments in the catalog above are metavariables that describe the command shape.
They are not literal tokens to type. This doc set uses "watch root" for the filesystem concept and
keeps the literal CLI flag spelling as `--watch-root`.

## Output Formats

`run` and `status` have one canonical result document family each, and `--format` chooses how the
CLI presents that result on stdout:

1. `json`: one compact JSON document on a single line
2. `json-pretty`: the same JSON document, pretty printed
3. `summary`: a concise human-oriented rendering of the same result

`summary` is presentation text, not a stable machine contract. Scripts and agents that need the
frozen document schema should keep using `json` or `json-pretty`.

## Top-Level Help And Version

Bare `ffhn` is CLI misuse. It writes top-level usage help to stderr and exits `2`.

`ffhn --help` writes top-level help to stdout and exits `0`.

`ffhn --version` writes one line to stdout and exits `0`: the literal tool name, one space, and
the current workspace version.

FFHN does not maintain a first-class `help` subcommand contract. Using the literal `help` token as
the first positional argument is an unrecognized subcommand and exits `2`.

Current top-level flag precedence follows Clap:

1. `ffhn --version --help` prints the version line
2. `ffhn --help --version` prints help

`--version` is a top-level flag. Passing it after `run`, `status`, or an unrecognized subcommand
is CLI misuse and exits `2`.

## Watch Root Discovery Rules

`run --all` walks the immediate subdirectories of the watch root, sorts them lexicographically, and
then decides whether to include each directory in the batch request list.

One directory is included when:

1. it contains a `target.toml` path, whether the validated target is enabled or disabled
2. it contains a `target.toml` path, but the directory label or target document violates FFHN's contract

One directory is excluded when:

1. it does not contain a `target.toml` path at all

This means `run --all` surfaces invalid target directories as batch failures instead of silently
dropping them, and it keeps disabled targets visible as `skipped_disabled` batch entries instead of
hiding them from the result.

When the discovered directory name itself violates FFHN's durable `target_id` rules, FFHN keeps
that raw directory label in `requested_targets` and emits a per-entry `fatal_error.kind = contract`
instead of rewriting or ignoring it.

Explicit live and dry-run requests against a valid disabled target both return
`result.kind = skipped_disabled` and do not fetch or extract content. Live mode persists that
skipped outcome, may deliver matching notification routes, and then attempts the final
`last_run.json` write. Dry-run keeps both persist paths at `not_attempted` and performs no
notification delivery.

If watch root traversal hits a filesystem error after discovery starts, FFHN exits fatally instead
of silently skipping the broken entry.

If the watch root does not exist or is not a directory, `run --all` exits fatally instead of
emitting an empty batch report.

## Stdout And Stderr

On every structured success or structured run/status failure, the CLI writes exactly one result to
stdout in the selected format and nothing else.

For `run`, that means invalid target documents, unavailable target paths, invalid persisted state,
integrity mismatches, disabled skips, content successes, and late persist failures all come back as
one `ffhn.run_report` or `ffhn.batch_run_report`. Callers reading JSON use `result.kind`, and for
failed results also use `result.cause` plus structured `result.error_detail`.

For `status`, FFHN emits one `ffhn.status_report`. Valid-target reports also carry top-level
`display_name` plus `enabled = true | false`, so disabled targets stay distinct from baseline
readiness without inventing extra baseline phases. Invalid target or state cases are modeled in
`status.kind` with structured `status.error_detail`.

Malformed target TOML remains `error_detail.kind = "toml"`, while structurally valid TOML that
violates FFHN's target contract uses `error_detail.kind = "contract"`.

Late live persist failures keep the already-computed `ffhn.run_report` on stdout. A failed primary
state commit downgrades the result to `result.kind = failed_transient` with
`result.cause = persist_error`, while a failed final `last_run.json` write leaves `result`
unchanged and records the failure in `persist.last_run_write`. The structured persist section
always shows which durable write failed.

Notification delivery failures behave differently from content failures. The run report keeps its
content result, but any `notifications[].outcome.status != "delivered"` makes the CLI exit `1`.

Stderr is reserved for fatal process-level failures and CLI-usage errors, and those lines are
prefixed with `error:` so shell users and agents can classify them without guessing. Examples:

1. argument parse failures
2. watch-root failures before FFHN can construct a structured result
3. JSON-rendering failures while writing the final document

## Exit Codes

| Condition | Exit code |
| --- | ---: |
| successful single-target runs, `skipped_disabled` single-target runs, dry-runs, batches with no failed/persist-failure/fatal/notification-delivery failures, or any `status` result document | `0` |
| structured `failed_transient`, structured `failed_permanent`, any live run report with `persist.has_failure = true`, any run report with notification delivery `outcome.status != "delivered"`, or any batch with failed/persist-failure/fatal/notification-delivery-failed entries | `1` |
| CLI misuse such as parse errors or invalid `--jobs` | `2` |
| fatal process-level error before structured document emission | `3` |

For valid targets, `status` waits behind any active live run so it can read one stable shared-lock
view before emitting the final `ffhn.status_report`.

`status` either exits `0` with an `ffhn.status_report` or exits `3` if FFHN could not emit a
structured status document at all.
