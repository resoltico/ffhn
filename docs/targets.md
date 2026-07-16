---
afad: "4.0"
domain: TARGETS
updated: "2026-07-16"
route:
  keywords: [target schema, ffhn.target, JSON Pointer, typed observation, decimal, reset]
  questions: ["how is a v2 JSON target structured?", "which typed values does FFHN support?", "when is reset required?"]
---

# Target Configuration

FFHN loads one v2 target from `<watch_root>/<target_id>/target.toml`. A target owns one scalar
measurement projection (`json_pointer`, `html_text`, or `html_attribute`), one declared semantic
type, and an explicit ordered list of named conditions. It requires `schema_name = "ffhn.target"`, `schema_version = 9`, `target_id`,
`display_name`, `enabled`, `escalate_after`, `declared_type`, `conditions`, `[target]`, `[fetch]`, and
`[projection]`. `[outbox]` and `[[routes]]` are optional operational delivery configuration.
Unknown fields are rejected.

```toml
schema_name = "ffhn.target"
schema_version = 9
target_id = "price"
display_name = "Current Price"
enabled = true
escalate_after = 3
declared_type = "money"

[[conditions]]
condition_id = "price-rise"

[conditions.predicate]
kind = "delta_pct"
reference = "last_accepted_observation"
threshold = "5.0000001"

[target]
kind = "http"
source_url = "https://example.test/price.json"

[fetch]
engine = "http"
user_agent = "ffhn/example"
accept = "application/json"

[projection]
kind = "json_pointer"
pointer = "/price"

[type_params]
currency = "USD"
locale = "en_us"
```

## Delivery routes and durable outbox

Delivery is opt-in: omit `[[routes]]` for a measurement-only target. An optional `[outbox]` table
sets bounded, deterministic retry behavior. Its defaults are `max_pending = 100`,
`max_attempts = 5`, `base_backoff_ms = 1000`, and `max_backoff_ms = 300000`. The valid ranges are
`max_pending = 1..=100000`, `max_attempts = 1..=100`, `base_backoff_ms = 1..=86400000`, and
`max_backoff_ms = base_backoff_ms..=604800000`.

```toml
[outbox]
max_pending = 100
max_attempts = 5
base_backoff_ms = 1000
max_backoff_ms = 300000

[[routes]]
route_id = "condition-log"
route_family = "on_condition"

[routes.adapter]
kind = "process_stdin"
program = "/bin/sh"
args = ["-c", "cat >> /tmp/ffhn-condition-events.jsonl"]
timeout_ms = 1000

[[routes]]
route_id = "run-log"
route_family = "on_run"

[routes.adapter]
kind = "process_stdin"
program = "/bin/sh"
args = ["-c", "cat >> /tmp/ffhn-run-events.jsonl"]
timeout_ms = 1000
```

An event id is `sha256(stable_json({ target_id, route_family, key }))`. An event-predicate
condition (`changed`, `delta_abs`, `delta_pct`, or `crosses`) uses its `condition_id` and accepted
`observation_seq` as the key; a level condition uses its `condition_id` and the entry transition
time. Evaluation issues use condition id, issue kind, and sequence. Source escalations use reason
class and the episode start; permanent contract errors use contract digest, error code, and first
seen time; reset and initialization use kind and contract digest. Thus retries retain one event id,
while a later eligible observation or episode is a distinct event.

Routes are strictly ordered by `route_id`; identifiers use the same lowercase letter, digit, and
single internal `-` or `_` form as condition ids. `route_family` is `on_condition` for named
condition triggers, or `on_run` for initialization, reset, source escalation, permanent contract
errors, and condition evaluation issues. The only current adapter is `process_stdin`: `program`
must be an absolute path, every supplied argument must be nonblank, and `timeout_ms` is in
`100..=60000`.

For each staged route, FFHN computes a deterministic event id and stores one compact
`ffhn.process_stdin` version-2 JSON payload as immutable bytes. Its typed `event_key` preserves
every exact hash input, including a level condition's `entry_at`, and state loading recomputes the
event id from that key before delivery. The child process receives those exact bytes followed by one
newline, so a simple append sink produces JSON Lines. A retry never rebuilds the payload from later
state or a run report. Successful records are removed; a failed record retries at
`commit_time + min(base_backoff_ms * 2^(attempt_count - 1), max_backoff_ms)` with no jitter; the
terminal attempt is reported as dead-lettered and removed. When the bounded queue is full, FFHN
never evicts an older record: it drops only the new record and records `outbox_overflow` in the run
report while still committing measurement state.

FFHN invokes a route only after the state/outbox commit is crash durable. It synchronizes the
staged and installed state file before delivery; on Unix it also synchronizes the directory metadata
that names the replacement. A synchronization failure prevents delivery. One drain handles only
records that were due when that drain began, so a retry scheduled from its retry-state commit time
always waits for a later drain.

Persisted payload bytes are not opaque input: state loading requires canonical stable JSON for
`ffhn.process_stdin` version 2, rejects unknown or malformed fields, and requires the payload's
target, derived event id, route, and route family to match its enclosing pending record. A malformed,
inconsistently bound, or hash-shaped-but-invented payload makes the state invalid and is never
delivered. The human summary is also checked against the stored event facts, so it cannot drift from
the structured payload.

Delivery configuration is operational and does not change the measurement contract digest. A
pending record nevertheless keeps its route identity and family: removing its route or moving it
between `on_run` and `on_condition` makes the stored state invalid. Restore the matching route to
drain it, or use the explicit reset lifecycle to discard it.

## Sources and projection

`target.kind` and `fetch.engine` must match. HTTP targets use an absolute `http` or `https`
`source_url`, plus required `user_agent` and `accept`; defaults are a 15,000 ms timeout,
2,000,000 bytes, and following redirects. File targets use an absolute UTF-8 `file_path`.
`fetch.max_bytes` must be in `1024..=104857600`.

`json_pointer` accepts the empty pointer for a scalar root or an RFC 6901 path such as
`/inventory/current`; only `~0` and `~1` escapes are valid. Arrays and objects are rejected because
one target measures one scalar leaf.

`html_text` and `html_attribute` use HTMLCut's public selection strategy, candidate selection, and
rendering tables. FFHN always builds HTMLCut's structured result internally so it can retain public
match metadata, but `output` is not a target field and `html_structured` is not an FFHN projection.
An HTML target must select one candidate (`single`, `first`, or `nth`); `all` is rejected. An
`html_text` target may use HTMLCut's CSS-selector or delimiter-pair strategy; an `html_attribute`
target additionally requires a CSS-selector strategy and a `name`.

```toml
[projection]
kind = "html_attribute"
name = "content"

[projection.selection.strategy]
kind = "css_selector"
selector = "meta#price"

[projection.selection.selection]
mode = "single"

[projection.selection.rendering]
whitespace = "rendered"
rewrite_urls = false
```

Use `html_attribute` with `meta[content]` and `name = "content"`, or `time[datetime]` and
`name = "datetime"`, for those common HTML metadata values. Attribute evidence and comparison are
always the original public CSS match attribute.

CSS `html_text` targets may optionally canonicalize only their typed comparison projection:

```toml
[projection]
kind = "html_text"

[projection.selection.strategy]
kind = "css_selector"
selector = "article.price a"

[projection.selection.selection]
mode = "single"

[projection.selection.rendering]
whitespace = "rendered"
rewrite_urls = false

[projection.selection.dom_canonicalization]
ignore_attributes = ["href", "data-nonce"]
strip_whitespace_nodes = true
```

HTMLCut first selects from the original DOM. FFHN retains its original selected text as
`raw_selected`; only the text rendered from HTMLCut's detached canonical clone becomes
`comparison_projection` and is parsed into the typed value. Candidate counts, diagnostics, and
CSS match metadata remain facts from the original selected DOM. There is no implicit attribute
deny-list.

`dom_canonicalization` is valid only for CSS `html_text` targets. FFHN rejects it for delimiter
selection and `html_attribute` targets with the permanent `htmlcut_plan_invalid` error, because
an attribute measurement always uses original metadata and cannot have a clone-based comparison.
The resulting `config_invalid` run report retains HTMLCut's plan digest, reason, and any public
diagnostics rather than reducing the rejection to unstructured text.
`sort_attributes` and `strip_comments` are not supported fields and are rejected rather than
silently ignored.

HTML HTTP sources cannot contain URL userinfo, because HTMLCut must receive a safe public input
base URL. That is a permanent `htmlcut_input_invalid` configuration error, not a source outage.
When redirects are enabled, FFHN passes the final successful response URL—not the original request
URL—as HTMLCut's input base for relative URL resolution. Source decoding, JSON syntax, a missing
JSON pointer, a non-scalar JSON projection, and transient HTML selection failures never create or
advance accepted state. They persist source-health facts without changing the accepted baseline,
sequence, or condition state.

## Typed values

`declared_type` is one of `integer`, `decimal`, `money`, `semver`, or `datetime`.

| Type | Representation | Canonical form | Parameters |
| --- | --- | --- | --- |
| `integer` | `i128` | decimal integer text | none |
| `decimal` | `rust_decimal::Decimal` | normalized exact decimal text | optional `locale` |
| `money` | exact decimal plus tag | normalized exact decimal text | uppercase `currency`; optional `locale` |
| `semver` | `semver::Version` | normalized SemVer text | none |
| `datetime` | `time::OffsetDateTime` | RFC3339 UTC text | `format`; optional `assumed_offset` |

Decimal and money parsing is exact. Values outside Rust Decimal's 96-bit, scale-28 boundary are
`value_unparseable`; FFHN never rounds or truncates. Numeric grammars are `invariant` (default,
no grouping), `en_us` (comma grouping and dot decimal), and `de_de` (dot grouping and comma
decimal). A date-time carries an explicit offset or a configured `assumed_offset`; FFHN never
uses the machine-local time zone. `format = "rfc3339"` already requires an explicit offset, so an
assumed offset is valid only with a configured format that omits one.

## Named conditions

`conditions` is a top-level array of tables ordered strictly by `condition_id`. A condition id is
target-local, stable, lowercase, at most 64 characters, and uses only single internal `-` or `_`
separators. An intentionally policy-free target must declare `conditions = []`; omission is not a
legacy default.

All threshold literals are quoted to preserve their exact configured grammar. `delta_pct.threshold`
is an invariant decimal percentage; every other threshold uses the target's declared type grammar.
FFHN rejects a numeric predicate for `semver` or `datetime`, a strictly negative delta threshold,
invalid typed literals, cross-currency comparisons, and hysteresis thresholds that conflict with
their direction. Signed zero is numerically zero, so `"-0"` is valid for either delta threshold.

| `kind` | Required fields | Supported types | Result and trigger rule |
| --- | --- | --- | --- |
| `changed` | `reference` | all | canonical identity differs; every satisfied evaluation triggers |
| `delta_abs` | `reference`, `threshold` | integer, decimal, money | exact absolute delta reaches threshold; every satisfied evaluation triggers |
| `delta_pct` | `reference`, `threshold` | integer, decimal, money | exact percentage delta reaches threshold; every satisfied evaluation triggers |
| `crosses` | `threshold`, `direction` | all | pre-run accepted value crosses threshold; every satisfied evaluation triggers |
| `lt`, `gt` | `threshold` | all | strict level comparison; triggers only on entry or re-entry |
| `band` | `enter_threshold`, `exit_threshold`, `direction` | all | directional hysteresis level; triggers only on entry or re-entry |

`direction = "rising"` crosses from below to at-or-above its threshold. A rising band enters at or
above `enter_threshold` and remains active at or above `exit_threshold`, so its enter threshold
must be greater than or equal to its exit threshold. `falling` is symmetric: it crosses from above
to at-or-below, enters at or below, remains active at or below, and requires an enter threshold no
greater than its exit threshold. A second same-direction crossing requires an intervening return to
the other side of the threshold; every qualifying crossing triggers.

Reference predicates accept `last_accepted_observation`, `fixed_initial_baseline`, or
`last_condition_transition`. The last transition is always scoped to the condition being evaluated.
An absent reference produces `unavailable`; valid arithmetic that overflows produces
`arithmetic_overflow`; a runtime zero percentage reference produces `zero_reference`. These are
condition outcomes, not measurement outcomes. Decimal arithmetic is checked and exact, so FFHN
does not decide by rounded division. Integer percentage comparison retains the full `i128` domain,
including cross-sign extremes; it never narrows accepted integers into decimal storage.

The live runtime invokes the policy staging algebra with contexts derived
only from pre-run state, then retains that exact result with the resulting accepted observation,
monotonic sequence, and per-condition state in one staged run through commit. It materializes
those preserved eligibilities as durable routing without re-evaluating policy. A foreign reference
is `unavailable`, never an implicit conversion. `last_condition_transition` remains scoped to its
condition. `arithmetic_overflow`, `zero_reference`, and `unavailable` update that condition's result
but do not block a valid accepted observation from advancing.

`escalate_after` is the positive number of consecutive identical source-suspect failures that
reaches one escalation eligibility. Source health records the reason, count, first failure time,
and latest diagnostic; a changed reason starts a new episode, while a valid observation restores
health. The fixed source-suspect vocabulary is `fetch_failed`, `json_malformed`,
`json_missing_pointer_target`, `json_non_scalar_pointer_target`, `value_unparseable`,
`htmlcut_no_match`, `htmlcut_ambiguous_match`, `htmlcut_missing_attribute`,
`htmlcut_match_index_out_of_range`, and `htmlcut_internal_failure`. HTMLCut failures retain their
reason, candidate count when known, plan digest, and public diagnostics. Invalid JSON Pointer and
HTML contract errors are permanent-error episodes instead: they never change source health,
accepted state, or condition state. The permanent vocabulary is closed: `invalid_json_pointer`,
`htmlcut_plan_invalid`, `htmlcut_input_invalid`, `htmlcut_invalid_selector`,
`htmlcut_invalid_slice_pattern`, `htmlcut_unsupported_value_type`,
`html_attribute_requires_css_selector`, and `html_selection_must_select_one`.

## Contract identity and reset

FFHN stores a stable SHA-256 digest over the source-kind-tagged target, fetch policy, projection,
declared type, parser identity and grammar version, type parameters, and every named condition
including its id, predicate, references, thresholds, direction, and `escalate_after`. HTML
projections additionally bind HTMLCut's extraction-semantics version. JSON projections deliberately
do not. The per-execution HTMLCut plan digest is evidence in an HTML observation or diagnostic, not
the target contract identity. A mismatch returns `refused_contract_digest` before acquisition,
comparison, or mutation.

Routes and outbox policy are deliberately excluded from that digest. They change delivery
operations, not the value being measured or its condition interpretation.

Run `ffhn reset --watch-root <PATH> --target <ID>` to accept a changed definition. Reset acquires
the target lock and blind-deletes FFHN-owned storage while preserving `target.toml`; it does not
parse, translate, or preserve storage contents.
