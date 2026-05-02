---
afad: "4.0"
version: "5.0.0"
domain: EXAMPLES
updated: "2026-05-03"
route:
  keywords: [examples, example index, file target, runnable example, materializer]
  questions: ["where are the ffhn examples?", "which checked-in ffhn example is runnable?", "why does ffhn keep a materializer script in examples?"]
---

# FFHN Examples

Use this directory as the entrypoint for checked-in runnable FFHN examples.

- [file-target-with-notifications/README.md](file-target-with-notifications/README.md): materializes a valid file-backed `ffhn.target`, runs it against included sample HTML, and demonstrates notification-hook behavior without requiring a live website

The checked-in example assets are treated as maintained contract surfaces. When `ffhn.target`, report semantics, or the README quick-start flow changes, update the example and rerun it in a disposable temp watch root instead of only editing the prose.
