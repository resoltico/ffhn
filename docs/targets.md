---
afad: "4.0"
domain: TARGETS
updated: "2026-05-17"
route:
  keywords: [target schema, ffhn.target, http target, file target, canonicalization, notifications, target id]
  questions: ["how is ffhn.target structured?", "what are the ffhn target defaults and validation rules?", "how do ffhn notification routes work?"]
---

# Target Configuration

FFHN loads one `ffhn.target` document from `<watch_root>/<target_id>/target.toml`.

Every target document requires:

1. `schema_name = "ffhn.target"`
2. `schema_version = 4`
3. `target_id`
4. `display_name`
5. `enabled`
6. `[target]`
7. `[fetch]`
8. `[selection]`
9. `[compare]`

Optional sections are `[storage]`, `[[notification_endpoints]]`, `[[notification_routes]]`, top-level `[extensions]`, and reserved
`[fetch.extensions]`. FFHN preserves those extension objects structurally but does not interpret
them semantically.

`enabled` has three user-facing effects:

1. live explicit runs return `skipped_disabled`
2. `run --all` excludes the target from discovery when the directory has a `target.toml` marker and that document is otherwise valid
3. explicit dry-runs continue through validation, fetch, extraction, and compare

The checked-in public examples at [watchlist/demo/target.toml](../watchlist/demo/target.toml) and
[examples/file-target-with-notifications/README.md](../examples/file-target-with-notifications/README.md)
are repo-contract-tested against the current schema, so they are the canonical example entrypoints
in this repository.

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

[fetch]
engine = "http"
user_agent = "ffhn/example"
accept = "text/html"
```

Rules:

1. `source_url` must be an absolute `http` or `https` URL
2. `file_path` is forbidden
3. `fetch.engine` must be `http`

### File targets

```toml
[target]
kind = "file"
file_path = "/absolute/path/to/page.html"

[fetch]
engine = "file"
max_bytes = 2000000
```

Rules:

1. `file_path` must be an absolute filesystem path
2. `source_url` is forbidden
3. `fetch.engine` must be `file`

The `file_path` above is schematic. For a checked-in runnable file-target example that materializes
a real absolute path to included sample HTML, use
[examples/file-target-with-notifications/README.md](../examples/file-target-with-notifications/README.md).

## Fetch Section

Network-fetch defaults:

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

HTTP responses with no `Content-Type` header are accepted and decoded as UTF-8 by default.
`fetch_unsupported_content_type` is reserved for responses that do send a non-HTML/XHTML media
type.

### File fetch rules

Additional file-only rules:

1. `fetch.engine` must be `file`
2. `max_bytes` is optional and defaults to `2000000`
3. `method`, `timeout_ms`, `user_agent`, `follow_redirects`, `accept`, and `headers` are not part of the file-fetch schema
4. local file bytes must decode as UTF-8, or FFHN returns `fetch_decode_error`

## Selection Section

FFHN supports two selection strategies:

1. `kind = "css_selector"`
2. `kind = "delimiter_pair"`

Shared fields:

1. `match = "single" | "first" | "nth"`

Selection tells FFHN how to locate one fragment. It does not decide how FFHN compares that
fragment. Projection choices such as rendered text versus HTML now live under `[compare]`.

### Candidate selection

The `match` shape is part of the selection type itself:

1. `match = "single"` and `match = "first"` carry no additional fields
2. `match = "nth"` requires `index`, and that index must be positive
3. supplying `index` for `single` or `first` is a decode error, not a later runtime validation warning

### CSS selector strategy

Required:

1. `selector`

Shape rule:

1. FFHN rejects delimiter-only fields during decode because `css_selector` and `delimiter_pair` are distinct tagged shapes, not one partially-filled bag

### Delimiter-pair strategy

Required:

1. `start`
2. `end`
3. `mode = "literal" | "regex"`
4. `include_start`
5. `include_end`

Additional rules:

1. `selector` is rejected during decode because it belongs only to the CSS-selector shape
2. `flags` are allowed only when `mode = "regex"`

## Compare Section

Compare owns the projection that FFHN turns into a durable compare artifact.

Supported compare bases:

1. `basis = "text"`
2. `basis = "inner_html"`
3. `basis = "outer_html"`

Text compare example:

```toml
[compare]
basis = "text"
whitespace = "normalize"
rewrite_urls = false
canonicalization = []
```

Outer-HTML compare example:

```toml
[compare]
basis = "outer_html"
rewrite_urls = false
canonicalization = []
```

Rules:

1. `compare.whitespace = "preserve" | "normalize"` is required when `basis = "text"`
2. `compare.whitespace` is forbidden when `basis = "inner_html"` or `basis = "outer_html"`
3. `compare.rewrite_urls = true | false` defaults to `false`
4. `compare.canonicalization` defaults to `[]`
5. FFHN always normalizes line endings to LF before hashing the final compare value

`compare.rewrite_urls` follows HTMLCut's effective-base rules:

1. HTTP targets pass their final `http` or `https` fetch URL into HTMLCut as the input base URL
2. file targets do not invent an input base URL from `file://...`
3. file targets can rewrite relative URLs when the document itself resolves an effective HTTP(S) base, such as `<base href="https://example.com/docs/">`
4. when `rewrite_urls = true` and no effective HTTP(S) base resolves, FFHN keeps the extraction successful and surfaces the HTMLCut warning code `EFFECTIVE_BASE_URL_UNRESOLVED` in `warning_codes`

The canonicalization pipeline is an ordered list of:

1. `trim`
2. `collapse_whitespace`
3. `normalize_newlines`
4. `strip_regex`
5. `lowercase`

When `compare.canonicalization` is empty or omitted, FFHN hashes the LF-normalized compare-basis
projection directly without any additional caller-configured transforms.

FFHN persists two different artifacts for successful baselines:

1. `outer.html` is the selected outer HTML after FFHN line-ending normalization
2. `compare.txt` is the final compare value after basis projection, URL rewriting, optional text
   whitespace shaping, and any configured canonicalizers

`normalize_newlines` remains a stable explicit vocabulary value, but it now declares an otherwise
implicit step in the caller-visible compare pipeline rather than serving as a hidden special case.

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

Notifications are best-effort delivery routes. FFHN models them in two layers:

1. `[[notification_endpoints]]` define reusable delivery adapters
2. `[[notification_routes]]` bind run outcomes to one named endpoint

The current adapter vocabulary has one member: `process_stdin`.

POSIX append-only JSONL sink:

```toml
[[notification_endpoints]]
name = "log-json"
kind = "process_stdin"
program = "/bin/sh"
args = ["-c", "cat >> /tmp/ffhn-report.jsonl"]
timeout_ms = 1000

[[notification_routes]]
name = "log-json"
on = ["changed", "failed_transient", "failed_permanent"]
endpoint = "log-json"
```

PowerShell append-only JSONL sink:

```toml
[[notification_endpoints]]
name = "log-json"
kind = "process_stdin"
program = "C:\\Program Files\\PowerShell\\7\\pwsh.exe"
args = [
  "-NoLogo",
  "-NoProfile",
  "-Command",
  "[System.IO.File]::AppendAllText($args[0], [Console]::In.ReadToEnd())",
  "C:\\Temp\\ffhn-report.jsonl",
]
timeout_ms = 1000

[[notification_routes]]
name = "log-json"
on = ["changed", "failed_transient", "failed_permanent"]
endpoint = "log-json"
```

FFHN writes one compact JSON document followed by a newline to the route process stdin, so simple
append-based sinks produce valid JSONL. Use the shell host and absolute paths that exist on the
target machine. The checked-in
[examples/file-target-with-notifications/README.md](../examples/file-target-with-notifications/README.md)
shows the same append-only pattern through repo-owned POSIX and PowerShell helpers.

Defaults:

| Field | Default |
| --- | --- |
| `notification_endpoints.timeout_ms` | `5000` |

Rules:

1. `notification_endpoints.name` values must be unique within the target
2. `notification_routes.name` values must be unique within the target
3. `notification_routes.on` must list at least one unique run outcome
4. `notification_routes.endpoint` must reference an existing `notification_endpoints.name`
5. `notification_endpoints.kind` must be `process_stdin`
6. `notification_endpoints.program` must be an absolute path
7. every `notification_endpoints.args` entry must be non-empty
8. `notification_endpoints.timeout_ms` must be in `100..60000`

Supported `on` values match the run-outcome vocabulary:

1. `initialized`
2. `changed`
3. `unchanged`
4. `failed_transient`
5. `failed_permanent`
6. `skipped_disabled`

`skipped_disabled` deliveries arise only from live runs on disabled targets. `run --all` keeps
valid disabled targets visible in the batch result as `skipped_disabled` entries, and dry-run
never delivers notifications.

FFHN sends the validated pre-delivery `ffhn.notification_payload` document to the route process on
stdin and also sets these environment variables:

1. `FFHN_TARGET_ID`
2. `FFHN_RUN_OUTCOME`
3. `FFHN_FAILURE_CAUSE`
4. `FFHN_RUN_MODE`
5. `FFHN_FAILURE_CLASS` (empty string for non-failure outcomes)

That stdin payload wraps the run report snapshot FFHN had before delivery results were appended, so
`run_report.notifications` is empty and `run_report.persist.last_run_write.status` is
`not_attempted`. If an earlier live persist substep already failed,
`run_report.persist.state_commit.status` may already be `failed` inside the stdin payload.

Route-process failures do not rewrite the run result, but they remain operationally significant:
FFHN records them in `run_report.notifications`, captures stderr text when it can, and makes the
CLI exit with code `1`.
