---
afad: "4.0"
domain: FUZZING
updated: "2026-07-14"
route:
  keywords: [fuzzing, cargo-fuzz, JSON Pointer, HTMLCut, typed observation, target TOML, state report]
  questions: ["what does the ffhn fuzz package cover?", "how do I run the ffhn fuzzers?", "which fuzzing checks are automatic versus manual?"]
---

# Fuzz Package

`fuzz/` is FFHN's standalone `cargo-fuzz` package. The required gate audits its lockfile, lint-checks the harnesses, and compile-smokes them. Live sanitizer-backed fuzzing remains a deliberate manual activity.

## Harnesses

| Fuzzer | V2 surface | Purpose |
| --- | --- | --- |
| `target_toml_documents` | `TargetDocument` | Decode arbitrary target TOML and validate JSON and HTML measurement contracts. |
| `state_and_report_json_documents` | State and emitted reports | Decode arbitrary JSON into v2 documents and run their validators. |
| `dry_run_file_targets` | File source plus JSON Pointer acquisition | Execute a dry-run against generated JSON input without mutating durable state. |

There is no retained v1 corpus. New minimized inputs belong only to a current harness and must demonstrate a v2 regression before being checked in.

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
cargo +"${QA_NIGHTLY_TOOLCHAIN}" fuzz run --fuzz-dir fuzz target_toml_documents
cargo +"${QA_NIGHTLY_TOOLCHAIN}" fuzz run --fuzz-dir fuzz state_and_report_json_documents
cargo +"${QA_NIGHTLY_TOOLCHAIN}" fuzz run --fuzz-dir fuzz dry_run_file_targets
```

Run one campaign at a time because `cargo-fuzz` owns the fuzz lockfile. Remove generated artifacts after an exploratory campaign with `cargo xtask hygiene clean --mode rebuildable` and `rm -rf fuzz/artifacts`.
