---
afad: "4.0"
domain: EXAMPLES
updated: "2026-04-30"
route:
  keywords: [example, file target, notifications, materialize target, absolute path]
  questions: ["how do I run the ffhn file-target example?", "why does the file-target example use a materializer script?", "where is the runnable file-target example?"]
---

# File Target With Notifications

This example stays checked in as a small directory instead of one raw `target.toml` file because FFHN file targets require an absolute `file_path`.

The included materializers write a valid `target.toml` whose `file_path` points at the checked-in [`release-notes.html`](release-notes.html) sample:

- [`materialize-target.sh`](materialize-target.sh) for POSIX shells
- [`materialize-target.ps1`](materialize-target.ps1) for PowerShell

Materialize the example into a temp watch root on macOS or Linux:

```bash
WATCH_ROOT="$(mktemp -d)"
./examples/file-target-with-notifications/materialize-target.sh \
  "$WATCH_ROOT/release_notes/target.toml"
ffhn run --watch-root "$WATCH_ROOT" --target release_notes
ffhn status --watch-root "$WATCH_ROOT" --target release_notes
ffhn run --watch-root "$WATCH_ROOT" --target release_notes --dry-run
```

The first live run above should report `initialized`, and the following `status` call should report `ready`.

Materialize the same example from PowerShell:

```powershell
$WatchRoot = Join-Path ([System.IO.Path]::GetTempPath()) ([System.Guid]::NewGuid().ToString())
New-Item -ItemType Directory -Force $WatchRoot | Out-Null
.\examples\file-target-with-notifications\materialize-target.ps1 `
  "$WatchRoot\release_notes\target.toml"
ffhn run --watch-root $WatchRoot --target release_notes
ffhn status --watch-root $WatchRoot --target release_notes
ffhn run --watch-root $WatchRoot --target release_notes --dry-run
```

The generated target keeps one best-effort notification route:

- POSIX materialization writes one `[[notification_endpoints]]` entry with `kind = "process_stdin"` and `/bin/sh` args that run the checked-in [`append-notification.sh`](append-notification.sh) helper, plus one `[[notification_routes]]` entry that points at that endpoint
- PowerShell materialization writes one `[[notification_endpoints]]` entry with `kind = "process_stdin"` and the current PowerShell host executable plus args that run the checked-in [`append-notification.ps1`](append-notification.ps1) helper, plus one `[[notification_routes]]` entry that points at that endpoint

That route only listens for `changed`, `failed_transient`, and `failed_permanent`, so the first successful `initialized` run does not append anything to `<watch_root>/release_notes/ffhn-release-notes-report.jsonl`.

When the route does run, stdin carries one `ffhn.notification_payload` document whose embedded `run_report` is the pre-delivery snapshot: `notifications` is empty and `persist.last_run_write.status` is `not_attempted`.

If you intentionally break that route, FFHN preserves the content run outcome in `ffhn.run_report`, records the failed delivery in `notifications[]`, and exits the CLI with code `1`.
