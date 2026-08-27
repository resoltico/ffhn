---
afad: "4.0"
domain: TARGETS
updated: "2026-08-25"
route:
  keywords: [source, measurement, ffhn.source, ffhn.measurement, JSON Pointer, HTMLCut]
  questions: ["how is a source configured?", "how is a measurement configured?", "which HTML projections exist?"]
---

# Source and Measurement Configuration

An FFHN graph separates acquisition from measurement. `sources/<source_id>/source.toml` defines one complete HTTP or local-file representation. `sources/<source_id>/measurements/<measurement_id>/measurement.toml` defines one typed scalar projection over that representation. Configuration creates no lineage; first successful projection creates measurement lineage atomically with state.

## Source contract

`source.toml` is `ffhn.source` version 1. It owns `source_id`, display name, enablement, source-health escalation, fetch contract, conditional-request policy, schedule, and optional `on_source` delivery configuration.

```toml
schema_name = "ffhn.source"
schema_version = 1
source_id = "shop"
display_name = "Shop product"
enabled = true
escalate_after = 3

[fetch]
engine = "http"
source_url = "https://example.test/product.json"
user_agent = "ffhn/example"
accept = "application/json"
max_bytes = 2000000
follow_redirects = true
max_redirects = 5

[fetch.timeouts]
connect_ms = 5000
read_idle_ms = 15000
total_ms = 30000

[conditional]
enabled = true

[schedule]
interval_ms = 300000
min_interval_ms = 60000
```

HTTP accepts only complete `200` or `203` representations; conditional `304` means `not_modified`. Other `2xx` responses are not representations. File sources always read fresh bytes. HTTP header names are case-insensitively validated: fixed `accept` and `user_agent` are non-secret protocol parameters; extensible headers and environment-backed header secrets never cross an origin boundary. HTMLCut is used only after a complete HTML representation is accepted.

## Measurement contract

`measurement.toml` is `ffhn.measurement` version 1. It owns `measurement_id`, projection, declared type and parameters, named conditions, extraction-health escalation, and optional `on_condition` and `on_measurement` delivery configuration.

```toml
schema_name = "ffhn.measurement"
schema_version = 1
measurement_id = "price"
display_name = "Current price"
enabled = true
escalate_after = 3
declared_type = "money"

[[conditions]]
condition_id = "price-rise"
[conditions.predicate]
kind = "delta_pct"
reference = "last_accepted_observation"
threshold = "5"

[projection]
kind = "json_pointer"
pointer = "/price"

[type_params]
currency = "USD"
locale = "en_us"
```

Supported declared types are `text`, `integer`, `decimal`, `money`, `semver`, and `datetime`. Parsing and policy arithmetic are exact; FFHN does not infer currency, convert incompatible values, round decimal decisions, or consult the local time zone.

`json_pointer` follows RFC 6901 and selects a scalar JSON value. `html_text`, `html_rendered_text`, and `html_attribute` use a validated HTMLCut plan prepared once per measurement and executed once against the source document. `html_text` is plain DOM descendant text; `html_rendered_text` preserves semantic document structure; `html_attribute` selects a named attribute. DOM text and attribute projections require a CSS selector. Optional `dom_canonicalization` is valid only for text comparison, uses a detached canonical clone, and never changes raw selected evidence; `html_attribute` rejects it as inert configuration.

## Delivery and reset

Delivery is opt-in: omit `[outbox]` and `[[routes]]` for measurement-only operation. They are mutually required when present. Source routes accept `on_source`; measurement routes accept `on_condition` or `on_measurement`. Each admitted record snapshots its adapter and retry policy, so later configuration edits cannot reroute queued delivery.

Every process-stdin attempt owns a complete operating-system process group on Unix and a Job Object on Windows. Success, failure, timeout, or wait failure terminates any surviving descendants before FFHN joins its bounded stdin and stderr workers, so an inherited pipe cannot extend the configured attempt timeout.

Event IDs derive from graph, source, and measurement lineage plus typed event facts; they exclude wall-clock time. A source or measurement reset mints fresh random lineage and discards only that scope’s pending records. FFHN has no migration or compatibility path.

Use `ffhn reset --source <ID>` for a complete source fresh start, or `ffhn reset --source <ID> --measurement <ID>` to replace one measurement lineage.
