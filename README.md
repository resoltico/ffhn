# FFHN — repeatable HTML monitoring for websites and local files

FFHN (Focused Fragment History Notifier) monitors one exact HTML fragment from a web page or
local HTML file. You define the target once in TOML, FFHN fetches and extracts that fragment,
compares it with the saved baseline, and emits a structured report you can use in a terminal,
cron job, process hook, or pipeline.

## What It Does

Use FFHN when you care about one known fragment and need repeatable checks instead of manual
checking or one-off scripts.

- Watch a CSS-selected fragment from a live URL or a local HTML file
- Save each check as a TOML target file and rerun it unchanged later
- Emit one stable result document for live runs, dry-runs, status checks, and batches
- Preview checks with `--dry-run` before wiring them into automation
- Route matching live outcomes into process-based notification hooks

## Quick Start

This example monitors one HTML fragment on a live page. If you want a self-contained local-file
path that does not depend on a live website, use the
[Portable quick start](https://github.com/resoltico/ffhn/blob/main/docs/getting-started.md#portable-quick-start).

1. Create `checks/quarterly-report/target.toml`:

```toml
# checks/quarterly-report/target.toml
schema_name    = "ffhn.target"
schema_version = 4
target_id      = "quarterly-report"
display_name   = "Quarterly Report"
enabled        = true

[target]
kind       = "http"
source_url = "https://company.com/quarterly"

[fetch]
engine           = "http"
user_agent       = "ffhn/example"
accept           = "text/html"
follow_redirects = true

[selection]
kind     = "css_selector"
match    = "single"
selector = "section.financials"

[compare]
basis            = "outer_html"
rewrite_urls     = false
canonicalization = []
```

2. Run one live check:

```bash
ffhn run --watch-root ./checks --target quarterly-report --format summary
```

On the first live run, FFHN creates the baseline for `quarterly-report` and prints a short summary
that tells you whether the fragment was initialized, changed, unchanged, skipped, or failed.

3. Inspect the saved machine-readable state:

```bash
ffhn status --watch-root ./checks --target quarterly-report --format json-pretty
```

This prints the structured `ffhn.status_report` document for that target, including whether the
target is enabled and whether a saved baseline exists.

## Documentation Index

- [Portable quick start](https://github.com/resoltico/ffhn/blob/main/docs/getting-started.md#portable-quick-start): the shortest verified path from install to a real FFHN run
- [Full docs index](https://github.com/resoltico/ffhn/blob/main/docs/README.md): the maintained map of every document under `docs/`
- [CLI reference](https://github.com/resoltico/ffhn/blob/main/docs/cli.md): commands, output formats, exit codes, and discovery rules
- [Target reference](https://github.com/resoltico/ffhn/blob/main/docs/targets.md): the `ffhn.target` contract and validation rules
- [Report reference](https://github.com/resoltico/ffhn/blob/main/docs/run-reports.md): `ffhn.run_report`, `ffhn.batch_run_report`, and reason-code semantics

## Legal

FFHN is released under the [MIT License](LICENSE). See [NOTICE](NOTICE) and [PATENTS](PATENTS.md)
for the remaining legal files.
