---
afad: "4.0"
domain: EXAMPLES
updated: "2026-05-01"
route:
  keywords: [examples, JSON, file target, runnable example, materializer]
  questions: ["where are the ffhn examples?", "which checked-in JSON example is runnable?", "why does ffhn keep a materializer script in examples?"]
---

# FFHN Examples

Use this directory as the entrypoint for checked-in runnable FFHN examples.

- [file-target-json/README.md](file-target-json/README.md): materializes a valid file-backed v2
  `ffhn.target`, runs it against included JSON, and shows a typed money observation without a live service

The checked-in example assets are treated as maintained contract surfaces. When `ffhn.target`, report semantics, or the repository-backed quick-start flow in [../docs/getting-started.md](../docs/getting-started.md) changes, update the example and rerun it in a disposable temp watch root instead of only editing the prose.
