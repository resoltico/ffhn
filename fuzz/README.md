---
afad: "3.5"
version: "2.0.1"
domain: FUZZING
updated: "2026-04-22"
route:
  keywords: [fuzzing, cargo-fuzz, libfuzzer, seeds, nightly sanitizer, dry-run harness, report validation]
  questions: ["what does the ffhn fuzz package cover?", "how do I run the ffhn seed smokes?", "which fuzzing checks are automatic versus manual?"]
---

# Fuzz Package

`fuzz/` is a standalone `cargo-fuzz` package. It is compile-smoked by `./check.sh`, but live sanitizer-backed fuzz execution is a separate manual workflow.

## Automatic Versus Manual

Automatic through `./check.sh`:

```bash
cargo audit --file fuzz/Cargo.lock -D warnings
cargo check --manifest-path fuzz/Cargo.toml --bins --locked
```

Manual live fuzzing:

1. requires `cargo-fuzz`
2. requires nightly
3. should normally be run from `fuzz/`
4. will write generated artifacts under `fuzz/target/` and may extend corpora if you do not clean up afterward

## Fuzzer Inventory

| Fuzzer | Target module(s) | Seed files | Primary concern |
| --- | --- | ---: | --- |
| `dry_run_file_targets` | `ffhn_core::fetch`, `ffhn_core::runtime::run` | 2 | file-source dry-run extraction drift |
| `state_and_report_json_documents` | `ffhn_core::model::report`, `ffhn_core::model::state` | 3 | schema validation drift |
| `target_toml_documents` | `ffhn_core::model::target` | 3 | target-contract drift |

## Representative Coverage Map

The table below names the main maintained source files each harness exercises; it is representative rather than exhaustive.

| Source module | Covered by |
| --- | --- |
| `crates/ffhn-core/src/fetch.rs` | `dry_run_file_targets` |
| `crates/ffhn-core/src/model/report.rs` | `state_and_report_json_documents` |
| `crates/ffhn-core/src/model/state.rs` | `state_and_report_json_documents` |
| `crates/ffhn-core/src/model/target.rs` | `target_toml_documents`, `dry_run_file_targets` |
| `crates/ffhn-core/src/runtime/run/execute.rs` | `dry_run_file_targets` |

## Maintained Seed-Smoke Commands

From the repository root:

```bash
cargo +nightly fuzz run --fuzz-dir fuzz target_toml_documents fuzz/corpus/target_toml_documents -- -runs=200
cargo +nightly fuzz run --fuzz-dir fuzz state_and_report_json_documents fuzz/corpus/state_and_report_json_documents -- -runs=200
cargo +nightly fuzz run --fuzz-dir fuzz dry_run_file_targets fuzz/corpus/dry_run_file_targets -- -runs=200
```

From inside `fuzz/`:

```bash
cargo +nightly fuzz run target_toml_documents corpus/target_toml_documents -- -runs=200
cargo +nightly fuzz run state_and_report_json_documents corpus/state_and_report_json_documents -- -runs=200
cargo +nightly fuzz run dry_run_file_targets corpus/dry_run_file_targets -- -runs=200
```

Those commands are the maintained seed-smoke path because they:

1. exercise the checked-in corpus directly
2. stay bounded instead of becoming open-ended fuzz campaigns
3. validate the current contract surfaces that FFHN depends on most heavily

Run the fuzzers one at a time. `cargo-fuzz` takes an exclusive lock on `fuzz/Cargo.lock`, so parallel launches just contend with each other instead of increasing useful coverage.

## Harness Notes

### `dry_run_file_targets`

Purpose:

1. exercise file-backed target fetch plus extraction
2. verify dry-run can reach a valid structured report
3. verify dry-run does not mutate persisted state snapshots or run reports

### `state_and_report_json_documents`

Purpose:

1. decode arbitrary JSON into `ffhn.state`
2. decode arbitrary JSON into `ffhn.run_report`
3. decode arbitrary JSON into `ffhn.batch_run_report`
4. decode arbitrary JSON into `ffhn.status_report`

### `target_toml_documents`

Purpose:

1. decode arbitrary TOML into `ffhn.target`
2. enforce target-id, source, fetch, selection, compare, storage, and notification rules
3. keep file-target fetch restrictions stable, including rejection of HTTP-only knobs

## Cleanup

After manual fuzzing, remove generated build and artifact output:

```bash
rm -rf fuzz/target fuzz/artifacts
```

Only keep newly generated corpus entries if you deliberately want to promote them into the checked-in fuzz inputs.
