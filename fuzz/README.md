---
afad: "4.0"
domain: FUZZING
updated: "2026-08-25"
route:
  keywords: [fuzzing, cargo-fuzz, observation graph, JSON Pointer, typed measurement, lineage, state]
  questions: ["what does the ffhn fuzz package cover?", "how do I run the ffhn fuzzers?", "which fuzzing checks are automatic versus manual?"]
---

# Fuzz Package

`fuzz/` is FFHN's standalone `cargo-fuzz` package. The required gate audits its lockfile, lint-checks the harnesses, and compile-smokes them. Live sanitizer-backed fuzzing remains a deliberate manual activity.

## Harnesses

| Fuzzer | Observation-graph surface | Purpose |
| --- | --- | --- |
| `graph_toml_documents` | Agent, source, and measurement configuration | Decode arbitrary TOML through each current graph configuration boundary. |
| `graph_json_documents` | Identity, state, manifest, event, delivery, and dead-letter documents | Decode arbitrary JSON through each durable observation-graph boundary and its validation. |
| `dry_run_file_sources` | File source plus typed JSON Pointer measurement | Execute a graph dry-run against generated JSON input without minting lineage or mutating durable state. |

There is no retained corpus for superseded schemas. New minimized inputs belong only to a current harness and must demonstrate a current regression before being checked in.

## Automatic Checks

```bash
cargo xtask audit --file fuzz/Cargo.lock
cargo check --manifest-path fuzz/Cargo.toml --bins --locked
cargo clippy --manifest-path fuzz/Cargo.toml --bins --locked -- -D warnings
cargo +<qa-nightly-toolchain> fuzz check --fuzz-dir fuzz
```

## Manual Campaigns

From the repository root, use the pinned QA nightly toolchain declared in `tooling/rust-tooling.env`:

```bash
QA_NIGHTLY_TOOLCHAIN="$(sed -n 's/^RUST_QA_NIGHTLY_TOOLCHAIN=//p' tooling/rust-tooling.env)"
cargo +"${QA_NIGHTLY_TOOLCHAIN}" fuzz run --fuzz-dir fuzz graph_toml_documents
cargo +"${QA_NIGHTLY_TOOLCHAIN}" fuzz run --fuzz-dir fuzz graph_json_documents
cargo +"${QA_NIGHTLY_TOOLCHAIN}" fuzz run --fuzz-dir fuzz dry_run_file_sources
```

Run one campaign at a time because `cargo-fuzz` owns the fuzz lockfile. Remove generated artifacts after an exploratory campaign with `cargo xtask hygiene clean --mode rebuildable` and `rm -rf fuzz/artifacts`.
