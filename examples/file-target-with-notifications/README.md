---
afad: "3.5"
version: "3.0.0"
domain: EXAMPLES
updated: "2026-04-23"
route:
  keywords: [example, file target, notifications, materialize target, absolute path]
  questions: ["how do I run the ffhn file-target example?", "why does the file-target example use a materializer script?", "where is the runnable file-target example?"]
---

# File Target With Notifications

This example stays checked in as a small directory instead of one raw `target.toml` file because FFHN file targets require an absolute `file_path`.

The included [`materialize-target.sh`](materialize-target.sh) script writes a valid `target.toml` whose `file_path` points at the checked-in [`release-notes.html`](release-notes.html) sample.

Materialize the example into a temp watch root:

```bash
WATCH_ROOT="$(mktemp -d)"
./examples/file-target-with-notifications/materialize-target.sh \
  "$WATCH_ROOT/release_notes/target.toml"
ffhn run --watch-root "$WATCH_ROOT" --target release_notes
ffhn status --watch-root "$WATCH_ROOT" --target release_notes
ffhn run --watch-root "$WATCH_ROOT" --target release_notes --dry-run
```

The first live run above should report `initialized`, and the following `status` call should report `ready`.

The generated target keeps the notification hook from the original example:

- `shell = "/bin/sh"`
- `command = "cat >> /tmp/ffhn-release-notes-report.jsonl"`

That hook only listens for `changed`, `failed_transient`, and `failed_permanent`, so the first successful `initialized` run does not append anything to `/tmp/ffhn-release-notes-report.jsonl`.

When the hook does run, stdin carries one `ffhn.notification_payload` document whose embedded `run_report` is the pre-delivery snapshot: `notifications` is still empty and `persist.wrote_last_run` is still `false`.

If you intentionally break that hook, FFHN still preserves the content run outcome in `ffhn.run_report`, but the failed delivery appears in `notifications[]` and the CLI exits with code `1`.

That hook shape is intentionally POSIX-oriented. On Windows, change `shell` and `command` after materialization to match your local shell.
