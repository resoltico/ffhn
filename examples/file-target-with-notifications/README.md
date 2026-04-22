---
afad: "3.5"
version: "2.0.1"
domain: EXAMPLES
updated: "2026-04-22"
route:
  keywords: [example, file target, notifications, materialize target, absolute path]
  questions: ["how do I run the ffhn file-target example?", "why does the file-target example use a materializer script?", "where is the runnable file-target example?"]
---

# File Target With Notifications

This example stays checked in as a small directory instead of one raw `target.toml` file because FFHN file targets require an absolute `file_path`.

The included [`materialize-target.sh`](materialize-target.sh) script writes a valid `target.toml` whose `file_path` points at the checked-in [`release-notes.html`](release-notes.html) sample.

Materialize the example into a temp watch root:

```bash
TMP_WATCH_ROOT="$(mktemp -d)"
./examples/file-target-with-notifications/materialize-target.sh \
  "$TMP_WATCH_ROOT/release_notes/target.toml"
ffhn run --watch-root "$TMP_WATCH_ROOT" --target release_notes
```

The generated target keeps the notification hook from the original example:

- `shell = "/bin/sh"`
- `command = "cat >> /tmp/ffhn-release-notes-report.jsonl"`

That hook shape is intentionally POSIX-oriented. On Windows, change `shell` and `command` after materialization to match your local shell.
