---
afad: "4.0"
domain: EXAMPLES
updated: "2026-07-14"
route:
  keywords: [example, JSON, file target, typed observation]
  questions: ["how do I run the local JSON example?", "how do I materialize a file target?"]
---

# Local JSON File Target

The materializer creates a v2 file target with an absolute path to the checked-in
[`price.json`](price.json) source.

```bash
WATCH_ROOT="$(mktemp -d)"
sh ./examples/file-target-json/materialize-target.sh "$WATCH_ROOT/price/target.toml"
ffhn run --watch-root "$WATCH_ROOT" --target price
ffhn status --watch-root "$WATCH_ROOT" --target price
```

The first live run accepts the typed USD observation. Edit the source value and run again to see
the canonical decimal comparison report. Use `ffhn reset` to discard the isolated `.ffhn` state.
