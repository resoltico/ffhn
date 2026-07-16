---
afad: "4.0"
domain: ARCHITECTURE
updated: "2026-07-15"
route:
  keywords: [architecture, ffhn-core, typed observation, JSON Pointer, state, outbox, delivery, reset]
  questions: ["what does FFHN core own?", "where is v2 state stored?", "what is the policy boundary?"]
---

# Architecture

`ffhn-core` owns target validation, HTTP/file source acquisition, JSON Pointer and HTMLCut scalar
selection, typed parsing, named-condition policy staging, contract identity, target locking,
isolated state persistence, durable delivery, and blind reset. `ffhn-cli` owns parsing, bounded
batches, rendering, and exit codes. `xtask` owns maintainer gates.

Within `ffhn-core`, the model is split by target contract, observation, policy condition/evaluation,
persisted state, and report; the runtime is split by execution, acquisition, storage, locking, and
report translation. These are responsibility boundaries, not version-labelled cutover modules.

A target flows through validation, source acquisition, JSON decoding or HTMLCut selection, typed
parsing, policy evaluation against pre-run state, one complete in-memory staged run, one
crash-durable atomic state/outbox commit, due-outbox drain, and report rendering. The HTMLCut
adapter is a narrow anti-corruption boundary: FFHN creates the structured plan internally, selects
one measurement, and retains only public metadata that belongs to FFHN evidence.

Configuration remains at `<watch_root>/<target_id>/target.toml`. State is isolated at
`<watch_root>/<target_id>/.ffhn/state.json`; locks live under `<watch_root>/.ffhn-locks/` so reset
can remove storage while holding the same target lock. The loader reads only the v2 storage path.
Normal state I/O rejects symlinked or non-regular storage nodes; only reset removes an arbitrary
`.ffhn` root node without inspecting its contents.

`TargetDocument::stage_policy_run` remains the pure policy boundary: it receives a complete
pre-run condition context plus the state-owned source/permanent episode transition, then returns an
all-in-memory branch with deterministic `on_condition` and `on_run` eligibilities. The runtime
combines that exact policy result with the next accepted-observation sequence, per-condition
temporal state, source-health, or permanent-error episode in one staged run. It materializes that
preserved plan into immutable route payloads and commits state plus pending outbox records before
any process is launched. The storage boundary synchronizes staged and installed state bytes; Unix
also synchronizes the directory metadata that names the replacement. The drain adapter reads the
stored bytes only.
