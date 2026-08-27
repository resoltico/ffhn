---
afad: "4.0"
domain: GETTING_STARTED
updated: "2026-08-25"
route:
  keywords: [getting started, observation graph, JSON file, decimal, measure, status]
  questions: ["how do I create my first graph?", "how do I measure a local JSON source?"]
---

# Getting Started

Create an empty graph and its source and measurement templates:

```bash
GRAPH_ROOT="$(mktemp -d)"
ffhn new source --graph-root "$GRAPH_ROOT" --source prices
ffhn new measurement --graph-root "$GRAPH_ROOT" --source prices --measurement current-price
printf '%s\n' '{"price":"12.50"}' >"$GRAPH_ROOT/prices.json"
```

Replace the generated source template with an enabled local-file source:

```bash
cat >"$GRAPH_ROOT/sources/prices/source.toml" <<EOF
schema_name = "ffhn.source"
schema_version = 1
source_id = "prices"
display_name = "Example prices"
enabled = true
escalate_after = 3

[fetch]
engine = "file"
file_path = "$GRAPH_ROOT/prices.json"
max_bytes = 1024

[conditional]
enabled = false

[schedule]
interval_ms = 300000
min_interval_ms = 60000
EOF
```

Replace the measurement template with an enabled decimal projection:

```bash
cat >"$GRAPH_ROOT/sources/prices/measurements/current-price/measurement.toml" <<'EOF'
schema_name = "ffhn.measurement"
schema_version = 1
measurement_id = "current-price"
display_name = "Current price"
enabled = true
escalate_after = 3
declared_type = "decimal"
conditions = []

[projection]
kind = "json_pointer"
pointer = "/price"
EOF

ffhn validate --graph-root "$GRAPH_ROOT" --all --format summary
ffhn measure --graph-root "$GRAPH_ROOT" --source prices --format summary
ffhn status --graph-root "$GRAPH_ROOT" --source prices --measurement current-price --format json-pretty
```

The first live measurement creates source and measurement lineage atomically with their state. Change `12.50` to `12.500` and measure again: the raw JSON evidence differs, but exact decimal canonicalization preserves the same value. Change it to `13.00` to produce a different accepted value.

Changing the source representation or measurement value contract yields measurement-scoped `refused_contract_digest`. Use `ffhn reset --source prices --measurement current-price` for a deliberate fresh measurement lineage, or omit `--measurement` to replace the whole source lineage.
