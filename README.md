# ffhn

Monitor websites by extracting the part that matters and reporting changes as JSON.

`ffhn` is a JSON-first CLI for watching a specific part of a page instead of diffing an entire document. It fetches a URL, passes the HTML to [`htmlcut`](https://github.com/resoltico/HTMLCut), stores the extracted result on disk, and reports whether the target was `initialized`, `changed`, `unchanged`, or `failed`. In `ffhn`, "strict" means it prefers explicit errors over guesswork: config is schema-validated, malformed state is treated as invalid instead of silently repaired, unsupported CLI combinations are rejected, and stored artifacts are verified before they are trusted. That makes the tool predictable for scripts, cron jobs, and AI agents.

## Why ffhn

- Watch only the part of a page that matters instead of the whole HTML document.
- Get machine-friendly JSON output every time.
- Keep durable on-disk state with `current.txt`, `previous.txt`, and `state.json`.
- See explicit failures when config, state, or stored artifacts are broken.

## Requirements

- Node.js 24+
- npm available in the same active Node environment
- [`htmlcut`](https://github.com/resoltico/HTMLCut) available on `PATH`

## Install

ffhn is distributed through GitHub, not npmjs.com.
It also depends on the separate [`htmlcut`](https://github.com/resoltico/HTMLCut) command, which must be installed in the same active Node environment first.

That means there are two practical install modes:

- install from GitHub release archives
- install from git checkouts

In both modes, the final step is `npm link`, which makes the global command point at the directory on disk that you installed from.

### Install From GitHub Release Archives

Use this when you want released versions without cloning repository history.

1. Confirm which Node environment is active:

```bash
node -v
npm prefix -g
```

2. Download and extract the GitHub release archive for [`htmlcut`](https://github.com/resoltico/HTMLCut).

3. Install and link `htmlcut` from its extracted directory:

```bash
cd /path/to/htmlcut-<version>
npm install
npm link
```

4. Download and extract the GitHub release archive for `ffhn`.

5. Install and link `ffhn` from its extracted directory:

```bash
cd /path/to/ffhn-<version>
npm install
npm link
```

6. Refresh your shell's command cache and verify both commands:

```bash
hash -r
command -v htmlcut
htmlcut --version
command -v ffhn
ffhn --version
ffhn --help
```

Keep both extracted directories. The global commands point back to them. If you delete or move them, the links break.

### Install From Git Checkouts

Use this when you want to track the repositories directly or develop from source.

1. Confirm which Node environment is active:

```bash
node -v
npm prefix -g
```

2. Clone, install, and link [`htmlcut`](https://github.com/resoltico/HTMLCut) first:

```bash
git clone https://github.com/resoltico/HTMLCut
cd htmlcut
npm install
npm link
```

3. Clone, install, and link `ffhn` second:

```bash
git clone <ffhn-repo-url>
cd ffhn
npm install
npm link
```

4. Refresh your shell's command cache and verify both commands:

```bash
hash -r
command -v htmlcut
htmlcut --version
command -v ffhn
ffhn --version
ffhn --help
```

## Update

### Update A GitHub Release Archive Install

To update a release-archive install, download and extract the newer releases, then relink in this order: `htmlcut` first, `ffhn` second.

```bash
cd /path/to/htmlcut-<new-version>
npm install
npm link

cd /path/to/ffhn-<new-version>
npm install
npm link

hash -r
htmlcut --version
ffhn --version
```

Once the new links work, the older extracted directories can be removed.

### Update A Git Checkout Install

If your global `ffhn` command points at git checkouts, update and relink in this order: `htmlcut` first, `ffhn` second:

```bash
cd /path/to/htmlcut
git pull
npm install
npm link

cd /path/to/ffhn
git pull
npm install
npm link
hash -r
htmlcut --version
ffhn --version
```

If you only changed source files in an already linked checkout, you usually do not need to run `npm link` again. Run it again after switching Node environments, after reinstalling Node, or whenever the global link disappears.

## Node Version Changes And Troubleshooting

If you use `fnm`, `nvm`, `asdf`, `volta`, or any setup that switches the active Node installation, the `npm link` registration belongs to that active Node environment.

For `ffhn`, there is one extra rule: `htmlcut` must also be linked in that same active environment.

That means:

- switching Node versions can make `htmlcut`, `ffhn`, or both disappear even though their install directories still exist
- upgrading Node can change the active global prefix
- your shell can cache an old command path until you run `hash -r`
- deleting or moving an extracted or checked-out directory that `npm link` points at breaks that command

Use this sequence after a Node switch, Node upgrade, or a sudden `command not found`:

1. Inspect the active environment:

```bash
node -v
npm prefix -g
command -v htmlcut || true
command -v ffhn || true
```

2. Go back to the directories you installed from.
`htmlcut` first, then `ffhn`:

```bash
cd /path/to/htmlcut-or-htmlcut-<version>
npm install
npm link

cd /path/to/ffhn-or-ffhn-<version>
npm install
npm link
```

3. Refresh the shell cache and verify:

```bash
hash -r
command -v htmlcut
htmlcut --version
command -v ffhn
ffhn --version
ffhn --help
```

If you want the tools available under more than one installed Node version, repeat `npm link` for each tool once per version while that version is active.

## Quick Start

Create a target:

```bash
ffhn init --target news-site
```

Edit `watchlist/news-site/target.toml` so it points at the page you want and extracts the part you care about.

Run it:

```bash
ffhn run --target news-site --pretty
```

Check the stored state later:

```bash
ffhn status --target news-site
```

## How It Works

1. Fetch the target URL.
2. Send the HTML to [`htmlcut`](https://github.com/resoltico/HTMLCut).
3. Hash the extracted output.
4. Persist state and report whether the target was `initialized`, `changed`, `unchanged`, or `failed`.

## Layout

Each target lives under a watchlist directory.

```text
watchlist/
  news-site/
    target.toml
    state.json
    current.txt
    previous.txt
```

Files:

- `target.toml`: the target definition you edit
- `state.json`: ffhn runtime state, counters, hashes, timestamps, and last error
- `current.txt`: the latest extracted output
- `previous.txt`: the prior extracted output when a real change is detected

`previous.txt` is not created on the first successful run.

## Config

Example `watchlist/news-site/target.toml`:

```toml
[target]
url = "https://example.com/news"

[extract]
from = "<main\\\\b[^>]*>"
to = "</main>"
pattern = "regex"
capture = "inner"
all = false

[request]
timeout_ms = 30000
max_attempts = 3
retry_delay_ms = 750
```

Rules:

- `[target]` is required and currently supports `url` only.
- `[extract]` is required and supports `from`, `to`, `pattern`, `flags`, `capture`, `all`.
- `[request]` is optional and supports `timeout_ms`, `max_attempts`, `retry_delay_ms`, `user_agent`.
- `ffhn` does not bundle [`htmlcut`](https://github.com/resoltico/HTMLCut); it invokes the separate `htmlcut` binary available on `PATH`.
- The default request user-agent is `ffhn/<current package version>`.
- `pattern` applies to both delimiters. There are no separate regex toggles for `from` and `to`.
- Unknown sections and unknown keys fail fast.
- TOML parse failures include line, column, and parser diagnostics in the JSON error details.

## Commands

Initialize a target:

```bash
ffhn init --target news-site
```

Run one target:

```bash
ffhn run --target news-site --pretty
```

Run all targets in a custom watchlist with explicit concurrency:

```bash
ffhn run --watchlist ./watchlist --concurrency 4
```

Inspect configured targets:

```bash
ffhn status
```

Inspect one configured target:

```bash
ffhn status --target news-site
```

Show CLI help or version as commands:

```bash
ffhn help
ffhn help run
ffhn version
```

Global help/version flags also work before or after a command, and help becomes command-scoped when you provide a subject command:

```bash
ffhn run --help
ffhn status --help
ffhn --help version
ffhn --version status
```

`status` returns:

- `pending`: valid target, but no `state.json` exists yet
- `ready`: valid target with readable state
- `invalid`: broken config, broken state file, or inconsistent stored artifacts

## What "Strict" Means

- Config is schema-validated. Unknown sections and unknown keys are errors.
- Malformed TOML returns parser diagnostics instead of being guessed through.
- `state.json` is validated. Invalid JSON or wrong schema versions are failures, not auto-repair events.
- `status` verifies that stored files really match the hashes recorded in `state.json` before reporting a target as `ready`.
- Unsupported CLI flag/command combinations fail as usage errors instead of being silently ignored.
- Output is always JSON, which makes the contract stable for automation.

## Output Model

`ffhn` always emits JSON.
`--json` is accepted as an explicit no-op for callers that prefer to request compact JSON output.
Unsupported command/flag combinations fail fast as usage errors. For example, `--concurrency` is accepted only for `run`.
Command-scoped help output includes `topic`, command-specific `usage`, and only the relevant options/examples for that command.

Successful `run` output includes:

- top-level timing, version, schema version, watchlist path, and selection metadata
- one result per target
- compact content previews and size deltas for agent-friendly triage
- compact line-based change summaries when a previous snapshot exists
- a summary with counts, attention flags, name lists, and failure type aggregates

Example:

```json
{
  "command": "run",
  "exit_code": 0,
  "ffhn_version": "1.0.0",
  "schema_version": 1,
  "watchlist": "/path/to/watchlist",
  "selection": {
    "target": "news-site",
    "concurrency": 4
  },
  "targets": [
    {
      "name": "news-site",
      "status": "initialized",
      "url": "https://example.com/news",
      "request": {
        "final_url": "https://example.com/news",
        "http_status": 200,
        "status_text": "OK",
        "attempts": 1,
        "body_bytes": 12345,
        "duration_ms": 412
      },
      "extract": {
        "output_bytes": 987,
        "duration_ms": 41
      },
      "content": {
        "current_chars": 987,
        "previous_chars": null,
        "delta_chars": null,
        "current_preview": {
          "text": "Headline one\n\nHeadline two",
          "truncated": false,
          "lines": ["Headline one", "Headline two"],
          "total_lines": 2,
          "lines_truncated": false
        },
        "previous_preview": null
      },
      "change": null,
      "hash": {
        "current": "abc123",
        "previous": null,
        "algorithm": "sha256"
      }
    }
  ],
  "summary": {
    "total_targets": 1,
    "initialized": 1,
    "changed": 0,
    "unchanged": 0,
    "failed": 0,
    "successful_targets": 1,
    "success_rate": 1,
    "total_target_duration_ms": 453,
    "avg_target_duration_ms": 453,
    "attention_required": false,
    "initialized_target_names": ["news-site"],
    "changed_target_names": [],
    "failed_target_names": [],
    "failure_types": {}
  }
}
```

`status` output also includes a top-level `summary` with counts and name lists for `ready`, `pending`, and `invalid` targets, plus `selection.target` when you use `--target`.

Each `status` target now includes:

- `content`: compact previews of stored `current.txt` and `previous.txt` when readable
- preview objects now include both raw text slices and clean non-empty `lines` samples
- `last_run`: the most recent execution outcome, derived as `initialized`, `changed`, `unchanged`, `failed`, or `never`
- `change`: compact added/removed line summaries derived from stored snapshots
- `artifacts`: file presence, file hashes, expected state hashes, and exact integrity issue codes

`status.summary.invalid_codes` aggregates invalid target reasons for quick triage.
`status.summary` also aggregates recent execution outcomes, so callers can spot stored failures or stored changes without re-running targets immediately.

`pending` is strict: it only means there is no `state.json` and no orphaned stored content. If `current.txt` or `previous.txt` exist without matching state, the target is `invalid`, not `pending`.

## Status Semantics

- `initialized`: first successful run, no prior hash exists
- `changed`: prior hash exists and differs from the new hash
- `unchanged`: prior hash exists and matches the new hash
- `failed`: fetch, extraction, state, or filesystem work failed

## State File

`state.json` uses schema version `1` and stores:

- timestamps: `created_at`, `last_run_at`, `last_success_at`, `last_change_at`
- hashes: `current_hash`, `previous_hash`
- `last_error`
- stats: runs, successes, failures, changes, consecutive failures, consecutive changes

State files are validated. Invalid JSON or wrong schema versions are treated as failures, not silently repaired.

## Exit Codes

- `0`: all requested targets succeeded
- `1`: usage error
- `2`: config error, including invalid targets discovered during `run` or `status`
- `3`: dependency error, such as missing [`htmlcut`](https://github.com/resoltico/HTMLCut)
- `4`: target runtime failure, such as network, extraction, state, or filesystem errors
- `5`: internal error

## Design Notes

- Atomic writes are used for config/state/content updates.
- `current.txt` is written on every successful run.
- `previous.txt` is updated only for real changes after initialization.
- `ffhn` streams fetched HTML to [`htmlcut`](https://github.com/resoltico/HTMLCut) over stdin and reads extracted text from stdout.
- Successful results include compact previews so callers can triage changes without opening files immediately.
- Successful results also include compact added/removed line summaries, so callers can usually identify the exact change without opening files.
- `status` verifies that stored files actually match the hashes in `state.json` before reporting a target as `ready`.
- HTTP reporting includes the real response status and attempt count.
- Failed runs preserve partial request, extract, and hash telemetry when those stages already completed.

## Development

```bash
npm test
npm run lint:check
npm run coverage
```
