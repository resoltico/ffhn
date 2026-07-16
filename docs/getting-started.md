---
afad: "4.0"
domain: GETTING_STARTED
updated: "2026-07-15"
route:
  keywords: [getting started, JSON file, decimal, run, reset]
  questions: ["how do I create my first target?", "how do I run a local JSON source?"]
---

# Getting Started

Create a temporary local JSON source and target:

```bash
WATCH_ROOT="$(mktemp -d)"
mkdir -p "$WATCH_ROOT/price"
printf '%s\n' '{"price":"12.50"}' >"$WATCH_ROOT/price/price.json"
cat >"$WATCH_ROOT/price/target.toml" <<EOF
schema_name = "ffhn.target"
schema_version = 9
target_id = "price"
display_name = "Example Price"
enabled = true
escalate_after = 3
declared_type = "decimal"
conditions = []

[target]
kind = "file"
file_path = "$WATCH_ROOT/price/price.json"

[fetch]
engine = "file"
max_bytes = 1024

[projection]
kind = "json_pointer"
pointer = "/price"
EOF
ffhn run --watch-root "$WATCH_ROOT" --target price
ffhn status --watch-root "$WATCH_ROOT" --target price --format json-pretty
```

The first run is `initialized`. Change `12.50` to `12.500` and run again: the raw evidence differs
but normalized decimal comparison yields `unchanged`. Change it to `13.00` for `changed`.

Changing the target definition causes `refused_contract_digest`. Run `ffhn reset` for that target
before accepting observations under the new definition.

For a checked-in materializer, see [examples/file-target-json](../examples/file-target-json/README.md).
