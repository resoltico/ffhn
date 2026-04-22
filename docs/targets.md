---
afad: "3.5"
version: "2.0.1"
domain: TARGETS
updated: "2026-04-22"
route:
  keywords: [target schema, ffhn.target, http target, file target, canonicalization, notifications, target id]
  questions: ["how is ffhn.target structured?", "what are the ffhn target defaults and validation rules?", "how do ffhn notification hooks work?"]
---

# Target Configuration

FFHN loads one `ffhn.target` document from `<watch_root>/<target_id>/target.toml`.

Every target document requires:

1. `schema_name = "ffhn.target"`
2. `schema_version = 1`
3. `target_id`
4. `display_name`
5. `enabled`
6. `[target]`
7. `[fetch]`
8. `[selection]`
9. `[compare]`

Optional sections are `[storage]`, `[[notifications]]`, top-level `[extensions]`, and reserved `[fetch.extensions]`.

`enabled` has three user-facing effects:

1. live explicit runs return `skipped_disabled`
2. `run --all` excludes the target from discovery when the document is otherwise valid
3. explicit dry-runs still inspect the target instead of short-circuiting

The checked-in public examples at [watchlist/demo/target.toml](../watchlist/demo/target.toml) and [examples/file-target-with-notifications/README.md](../examples/file-target-with-notifications/README.md) are repo-contract-tested against the current schema, so they are the canonical example entrypoints in this repository.

## `target_id` Rules

`target_id` is part of the durable filesystem contract.

It must:

1. start with `[a-z0-9]`
2. stay within 64 characters
3. use only lowercase ASCII letters, digits, `_`, and `-`
4. use only single internal separators
5. not end with `_` or `-`
6. not use reserved device names such as `con`, `prn`, `nul`, `com1`, or `lpt1`

Examples:

1. valid: `demo`, `demo_1`, `demo-1`
2. invalid: `Demo`, `demo__1`, `demo--1`, `demo_`, `con`

## Source Kinds

FFHN supports two target source families.

### HTTP targets

```toml
[target]
kind = "http"
source_url = "https://example.com"
```

Rules:

1. `source_url` must be an absolute `http` or `https` URL
2. `file_path` is forbidden
3. `fetch.engine` may be `http` or `browser`

`browser` is a stable configuration vocabulary value, but the current Rust rewrite still uses the HTTP transport backend for it. Reports preserve `engine = "browser"` when you choose that contract value.

### File targets

```toml
[target]
kind = "file"
file_path = "/absolute/path/to/page.html"

[fetch]
engine = "file"
follow_redirects = false
```

Rules:

1. `file_path` must be an absolute filesystem path
2. `source_url` is forbidden
3. `fetch.engine` must be `file`

The `file_path` above is schematic. For a checked-in runnable file-target example that materializes a real absolute path to included sample HTML, use [examples/file-target-with-notifications/README.md](../examples/file-target-with-notifications/README.md).

## Fetch Section

Shared defaults:

| Field | Default |
| --- | --- |
| `method` | `GET` |
| `timeout_ms` | `15000` |
| `max_bytes` | `2000000` |
| `follow_redirects` | `true` |

Shared validation:

1. `max_bytes` must be in `1024..=104857600`
2. `method` is currently the fixed vocabulary value `GET`

### HTTP fetch rules

Additional HTTP-only rules:

1. `timeout_ms` must be in `1000..=600000`
2. `user_agent` is required
3. `accept` is required
4. `headers` keys and values must be non-empty when present
5. `fetch.engine = "file"` is forbidden

### File fetch rules

Additional file-only rules:

1. `fetch.engine` must be `file`
2. `timeout_ms` must stay at the fixed default `15000`
3. `follow_redirects` must be `false`
4. `user_agent` must be empty
5. `accept` must be empty
6. `headers` must be empty
7. local file bytes must decode as UTF-8, or FFHN returns `fetch_decode_error`

Because `follow_redirects` defaults to `true`, file targets should set it explicitly to `false` in `target.toml`.
The remaining HTTP-only fetch knobs stay in the shared schema for compatibility with the common `[fetch]` shape, but FFHN rejects non-empty `user_agent`, non-empty `accept`, and any `timeout_ms` value other than the fixed default on file targets.

## Selection Section

FFHN supports two selection strategies:

1. `kind = "css_selector"`
2. `kind = "delimiter_pair"`

Shared fields:

1. `match = "single" | "first" | "nth"`
2. `output = "text" | "inner_html" | "outer_html"`
3. `whitespace = "preserve" | "normalize"`
4. `rewrite_urls = true | false`

### Candidate selection

`selection.index` is:

1. required and positive when `match = "nth"`
2. forbidden for `single` and `first`

### CSS selector strategy

Required:

1. `selector`

Forbidden:

1. `start`
2. `end`
3. `mode`
4. `include_start`
5. `include_end`
6. `flags`

### Delimiter-pair strategy

Required:

1. `start`
2. `end`
3. `mode = "literal" | "regex"`
4. `include_start`
5. `include_end`

Additional rules:

1. `selector` is forbidden
2. `flags` are allowed only when `mode = "regex"`

## Compare Section

Current compare basis:

1. `basis = "canonical_text_sha256"`

The canonicalization pipeline is an ordered list of:

1. `trim`
2. `collapse_whitespace`
3. `normalize_newlines`
4. `strip_regex`
5. `lowercase`

`strip_regex` additionally requires:

1. `pattern`
2. optional regex `flags`

The supported regex flags are:

1. `case_insensitive`
2. `multi_line`
3. `dot_matches_new_line`
4. `swap_greed`
5. `ignore_whitespace`

## Storage

`[storage]` controls retained successful snapshots.

```toml
[storage]
history_limit = 10
```

Rules:

1. `history_limit` defaults to `10`
2. `history_limit` must be in `1..=256`
3. the count is total retained successful snapshots, including `snapshots/current`

That means the history directory can hold at most `history_limit - 1` older snapshots.

## Notifications

Notifications are best-effort shell hooks.

```toml
[[notifications]]
name = "log-json"
on = ["changed", "failed_transient", "failed_permanent"]
shell = "/bin/sh"
command = "cat >> /tmp/ffhn-report.jsonl"
timeout_ms = 1000
```

Defaults:

| Field | Default |
| --- | --- |
| `shell` | `/bin/sh` |
| `timeout_ms` | `5000` |

Rules:

1. `name` values must be unique within the target
2. `on` must list at least one event
3. `shell` must be an absolute path
4. `command` must be non-empty
5. `timeout_ms` must be in `100..60000`

Supported `on` events match the run-outcome vocabulary:

1. `initialized`
2. `changed`
3. `unchanged`
4. `failed_transient`
5. `failed_permanent`
6. `skipped_disabled`

FFHN sends the validated pre-notification run report to the hook on stdin and also sets these environment variables:

1. `FFHN_TARGET_ID`
2. `FFHN_RUN_OUTCOME`
3. `FFHN_REASON_CODE`
4. `FFHN_RUN_MODE`
5. `FFHN_FAILURE_CLASS` (empty string for non-failure outcomes)
6. `FFHN_NOTIFICATION_EVENT`

That stdin payload is the run report before notification delivery results are appended, so the serialized payload omits `notifications` entirely and `persist.wrote_last_run` is still `false`.
