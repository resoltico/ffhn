---
afad: "3.5"
version: "3.0.1"
domain: OVERVIEW
updated: "2026-04-23"
route:
  keywords: [ffhn, website monitor, local html monitor, quick start, release packages, json reports]
  questions: ["what is ffhn?", "how do I try ffhn quickly?", "where do I get ffhn?", "where are the full ffhn docs?"]
---

# ffhn

`ffhn` is a command-line monitor for people who keep rechecking the same website or local HTML file and want a saved, repeatable check instead of another one-off script.

If the same page keeps pulling you back before your morning coffee cools, FFHN lets you pin the exact bit you care about, rerun it later, and get machine-readable output when it changes.

- Watch live pages or local HTML files.
- Save one repeatable check instead of rebuilding the same scrape.
- Dry-run before you wire it into automation.
- Keep JSON reports, saved snapshots, and optional shell-hook notifications.

[Try one sample page](docs/getting-started.md#quick-start) · [Get a release package](https://github.com/resoltico/ffhn/releases/latest) · [See the full docs](docs/README.md)

## One Sample Check

```bash
WATCH_ROOT="$(mktemp -d)"
./examples/file-target-with-notifications/materialize-target.sh \
  "$WATCH_ROOT/release_notes/target.toml"
ffhn run --watch-root "$WATCH_ROOT" --target release_notes
```

If you are running straight from this repository, replace `ffhn` with `cargo run -p ffhn-cli --`.

That sample path is exercised by automated tests. The first live run gives you a structured JSON result and a saved point of comparison for the next run. The full sample flow, packaged install steps, and Windows path live in [docs/getting-started.md](docs/getting-started.md).

## When It Fits

FFHN is for recurring checks on one known page or file:

- release notes, status pages, prices, docs fragments, or exported HTML you want to watch again later
- scripts, cron jobs, or shell hooks that want stable JSON instead of screen-scraped terminal text
- teams that want a small monitoring tool before reaching for a full browser stack

If you only need a one-time scrape, a short script may be enough. If you need crawling, login flows, or heavy browser interaction, use a larger automation stack.

## Safe To Try

- The sample page and quick-start commands in this repository are exercised by automated tests.
- Public releases are checksummed and easy to inspect.
- The project is open source and MIT-licensed.

## Go Deeper

- [Getting started](docs/getting-started.md): source build, release packages, and the full sample flow
- [Command guide](docs/cli.md): `run`, `status`, exit codes, and discovery rules
- [Target guide](docs/targets.md): how to define what FFHN should watch
- [Report guides](docs/reports.md) and [run reports](docs/run-reports.md): what FFHN writes and emits

## License

MIT. See [LICENSE](LICENSE).
