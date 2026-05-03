[![ffhn Art](https://raw.githubusercontent.com/resoltico/ffhn/main/images/ffhn.png)](https://github.com/resoltico/ffhn)

[![Release](https://img.shields.io/github/v/release/resoltico/ffhn?label=release)](https://github.com/resoltico/ffhn/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey.svg)](https://github.com/resoltico/ffhn/blob/main/docs/platform-support.md)

# ffhn — repeatable HTML monitoring for websites and local files

ffhn (Focused Fragment History Notifier) watches a specific fragment of a web page or local HTML
file and tells you when it changes.
Define the check once as a TOML target file; run it any time to get a structured JSON result
against the last saved snapshot.

Every page worth monitoring is one you've been checking before your coffee cools — by hand, or by
bolting together a fresh script each time. ffhn saves the check as a file. Pour it again later and
it compares the new content against the snapshot it kept.

- Watch a CSS-selected fragment from a live URL or a local HTML file
- Save each check as a TOML target file; run it unchanged any time
- Get structured JSON on the first run and on every comparison after
- Dry-run to preview what the check returns before wiring it into automation
- Pipe results into a notification script or process hook

[Getting started](https://github.com/resoltico/ffhn/blob/main/docs/getting-started.md#portable-quick-start) · [Command guide](https://github.com/resoltico/ffhn/blob/main/docs/cli.md) · [Docs index](https://github.com/resoltico/ffhn/blob/main/docs/README.md)

## Brew Once, Watch Any Time

Write the recipe once — what page to watch, which selector to use, how to compare:

```toml
# checks/quarterly-report/target.toml
schema_name    = "ffhn.target"
schema_version = 1
target_id      = "quarterly-report"
display_name   = "Quarterly Report"
enabled        = true

[target]
kind       = "http"
source_url = "https://company.com/quarterly"

[fetch]
engine = "http"

[selection]
kind     = "css_selector"
selector = "section.financials"

[compare]
basis = "canonical_text_sha256"
```

```bash
ffhn run --watch-root ./checks --target quarterly-report
```

First run saves a snapshot while the page is still warm. Every run after is the same pour —
compare against the last snapshot, see what changed.

## Where It Fits

Recurring checks on one known page or file: a release page you follow, a price you track, a status
indicator, a doc section your automation depends on. Works in cron jobs, process hooks, and
pipelines that want stable JSON instead of fragile screen-scraping.

## Get It

[Download for macOS, Linux, or Windows →](https://github.com/resoltico/ffhn/releases)

New here? [Getting started](https://github.com/resoltico/ffhn/blob/main/docs/getting-started.md#portable-quick-start) covers install paths and your first live
check. The [target guide](https://github.com/resoltico/ffhn/blob/main/docs/targets.md) shows how to define what to watch.

## Legal

ffhn is released under the [MIT License](LICENSE). See [NOTICE](NOTICE) and
[PATENTS](PATENTS.md) for the remaining legal files.
