# Critical

1. Always load and use the .codex/AGENTS_EXTRA.md, if it exists, when working on the project. AGENTS_EXTRA.md contains specialized project-tailored information.
2. Before starting actual work on documentation, but not earlier than that, load the .codex/PROTOCOL_AFAD.md, and use it for all your work on the documentation.

# Rust Agent Protocol

Operating standard for AI agents on modern Rust projects.

Not boilerplate. Not motivational prose. An execution doctrine for production-grade Rust: correct architecture, strong types, and strict verification.

Applies to: libraries, services, CLIs, daemons, backends, systems tools, Rust-backed desktop apps, Tauri projects.

Local repository rules take precedence. Otherwise, follow this strictly.

# Systems Over Goals: The Non-Negotiable Coding Rule

Live inside the unbreakable loops of Red → Green → Refactor and Boy Scout + Mikado. When writing new code, start with the smallest failing proof of behavior—test, assertion, or reproducible check—write the minimal implementation that makes it pass, then refactor immediately until the result is obvious, simple, expressive, and easy to change. When touching existing code, even for a one-line fix, leave the surrounding system strictly better than you found it: rename for clarity, extract coherent units, delete dead paths, collapse unnecessary complexity, remove compatibility shims that no longer serve a real contract, and pay down technical debt through the smallest safe sequence of validated steps. If a local refactor naturally unlocks a broader system-wide improvement, continue it while each step remains safe and well-proven. Do not treat architecture as something to preserve, defer, or design in advance; let it emerge from never leaving touched code worse than you opened it. The requested task defines only the entry point. The loops define the standard. Stop only when the touched code is clearly better and the next improvement is a separate slice.

# Shared Contract Facts Have One Canonical Owner

All contract-defining facts must have exactly one canonical owner and must be declared there in a form other parts of the system can derive from rather than redefine. Identifiers, limits, labels, rules, capabilities, and other externally meaningful contract facts must not be hard-coded in parallel across interfaces, tools, docs, summaries, or error surfaces. Any surface that exposes contract facts must derive them from the canonical source or from generated artifacts rooted in it. Build-time validation must fail on drift, missing registrations, contradictory definitions, or references to contract facts outside the canonical registry.

---

## 0. Prime Directive

Produce Rust that is:

- correct
- explicit
- type-driven
- test-proven
- architecturally clean
- secure at boundaries
- maintainable under change
- performant where it matters
- mechanically understandable

Do not optimize for:

- minimizing diffs at the expense of design
- preserving weak abstractions
- speculative generalization
- hiding complexity behind indirection
- "works for now"
- suppressing warnings instead of fixing causes
- cloning, boxing, or sharing merely to escape ownership pressure
- widening public APIs or visibility for local convenience

A fast wrong change is failure.
A messy working change is deferred failure.
A "green enough" change that leaves the type model, manifest contract, or verification story incoherent is failure.

---

## 1. Execution Protocol

### 1.1 Inspect Before Designing

Before writing or revising code, map the repository state.

Always inspect:

- workspace layout and relevant `Cargo.toml` files
- crate and module boundaries affected by the task
- `rust-toolchain.toml`, `package.rust-version`, workspace `resolver`, `.cargo/config.toml`
- existing types, traits, public exports, feature flags, and `build.rs`
- existing tests for the affected area
- repo-specific verification commands, `justfile`, `xtask`, CI mirrors, or coverage gates

Do not assume repository state. Verify it.

### 1.2 Pre-Implementation Plan

For any non-trivial task, emit one visible `<PLAN>` block before editing code.

Requirements:
- maximum 150 words
- no pleasantries, no essay, no restating the prompt
- only real uncertainties discovered from the repo

```text
<PLAN>
Goal: ...
Current shape: ...
Boundaries: ...
Type/ownership model: ...
Failure model: ...
Tests: ...
Verification: ...
</PLAN>
```

If the task is trivial, skip the plan.

### 1.3 Compile-Driven Iteration

Work in small coherent increments:

1. make the smallest meaningful change
2. run `cargo check` (narrow)
3. read the first real compiler error
4. fix the root cause
5. rerun `cargo check`
6. widen verification only after local shape is sound

Do not pile up cascading errors and try to reason about them all at once.

### 1.4 Root-Cause Fixes Only

When verification fails:

- read the actual failure output
- identify the structural, typing, ownership, or logic cause
- fix that cause
- rerun the narrowest relevant check
- rerun full required verification before declaring completion

Do not:

- guess at failures
- cargo-cult compiler suggestions without understanding them
- suppress warnings with `#[allow(...)]` unless the warning is a proven false positive, narrowly scoped, and documented
- bypass failing tests unless the task explicitly requires quarantining a known issue
- claim completion while required checks still fail

### 1.5 Edit Discipline

Before editing existing code:

- read the relevant files first
- preserve sound local conventions
- improve design when needed rather than layering hacks
- treat `Cargo.toml`, feature, export, and `cfg` changes as architectural changes

Do not edit blind.

---

## 2. Modern Rust Baseline

Assume the modern stable Rust stack unless the repository explicitly overrides it.

Defaults:

- Rust: latest stable
- Edition: `2024`
- Async runtime: Tokio (when async is actually needed)
- Serialization: Serde (when serialization is actually needed)
- Observability: `tracing` (when structured observability is actually needed)
- Typed domain/library errors: `thiserror`
- Binary/CLI/glue errors: `anyhow`

These are baseline defaults, not mandatory dependencies for every crate.

When creating or revising `Cargo.toml`, default to:

```toml
[package]
edition = "2024"

[lints.rust]
unsafe_code = "warn"

[lints.clippy]
all = "warn"
```

Strengthen lint policy further if the repository already does so.

---

## 3. Cargo and Manifest Contract

### 3.1 `Cargo.toml` Is a Design Surface

`Cargo.toml` communicates what the crate is, what it depends on, which features are real, what the compatibility contract looks like, and what lint posture the crate expects.

Rules:

- no unused dependencies
- no accidental default-feature sprawl
- no duplicate sources of truth for package identity, version, or description
- if the workspace owns version and description, do not duplicate them in members

### 3.2 Cargo Facts That Matter

Treat these as contract surfaces, not trivia:

- `edition = "2024"` implies resolver `"3"` — this changes feature resolution for dev-dependencies and incompatible-version handling; set it explicitly in the workspace rather than relying on inference
- virtual workspaces must set `resolver` explicitly; there is no root package edition to infer it from
- `package.rust-version` is part of the compatibility contract
- features are additive; enabling one must not disable functionality or introduce a SemVer-incompatible change
- inherited `workspace.dependencies` features are additive with member dependency features
- `build.rs` is part of the build contract, not "just a script"

For published or reused crates, also treat as compatibility decisions: public API, public feature names, default features, examples, and MSRV.

### 3.3 Anti-Hallucination Protocol

Never guess dependency versions, crate names, or feature flags.

Before modifying `Cargo.toml`:

- verify crate existence and current version via `cargo search`, docs.rs, crates.io, or authoritative upstream docs
- verify feature names actually exist in that version
- verify whether default features are acceptable
- verify the correct section: `[dependencies]`, `[dev-dependencies]`, `[build-dependencies]`, target-specific, or `[workspace.dependencies]`

Do not invent:

- version numbers
- feature flags
- resolver behavior
- crate rename assumptions
- "probably correct" optional dependency wiring

### 3.4 Dependency Discipline

Every dependency adds maintenance cost, security surface, compile time, and compatibility pressure.

Prefer crates that are mature, widely used, actively maintained, well documented, and idiomatic in modern Rust.

Do not reinvent commodity infrastructure without a strong reason.

### 3.5 Feature Discipline

Use feature flags for real optional capability boundaries:

- optional integrations
- platform-specific support
- optional heavy dependencies
- truly optional runtime modes

Do not use feature flags to:

- hide broken code or core correctness issues
- encode negative logic
- create combinatorial chaos
- patch over bad architecture

If a feature materially changes behavior, tests must cover that mode.

### 3.6 Build Script Discipline

A `build.rs` is a boundary with real cost.

Rules:

- keep it minimal
- use explicit change detection: `cargo::rerun-if-changed`, `cargo::rerun-if-env-changed`
- do not bury core logic in `build.rs`
- if a build script changes generated code or linked behavior, test the contract it creates

### 3.7 `cargo-deny` Multi-Version Policy Requires Active Maintenance

`multiple-versions = "deny"` is valuable for keeping the dependency graph lean and ensuring security patches propagate uniformly, but it collapses during normal upgrades.

Complex transitive crates (`syn`, `regex-syntax`, `indexmap`, `rustls`) frequently have multiple coexisting versions during upgrade windows. Without `skip` entries for known-safe version pairs, bumping a single top-level dependency can hard-block CI until every other crate in the tree catches up.

Rules:

- populate `skip` when a transitive conflict is known-safe and temporary
- document why each `skip` entry exists
- remove `skip` entries once the dependency graph converges
- treat a growing `skip` list as a signal to audit whether the dep graph needs active pruning

---

## 4. Project Structure

Structure is part of the design, not boilerplate.

### 4.1 Shape Must Match Responsibility

Good signs:

- modules map to cohesive responsibilities
- dependency direction is clear
- tests have an obvious home
- crate roots stay thin
- transport/UI layers do not own domain rules

Bad signs:

- everything in `main.rs` or `lib.rs`
- flat `src/` with unrelated concerns mixed together
- "utils", "helpers", "common", "shared" dumping grounds
- one module everything depends on for unrelated reasons
- structure chosen by file length rather than responsibility

### 4.2 Typical Project Shapes

Larger projects:

```text
src/
  domain/
  application/
  infrastructure/
  interfaces/
  support/
```

Medium projects:

```text
src/
  lib.rs
  core/
  io/
  api/
  config/
```

CLI-heavy projects:

```text
src/
  main.rs
  cli/
  commands/
  core/
  infra/
```

Do not cargo-cult these. Choose a shape that matches actual responsibilities.

### 4.3 Crate Roots Are Composition Roots

`main.rs`, `lib.rs`, and equivalent roots may:

- declare modules
- wire dependencies
- expose deliberate public surfaces

They must not:

- accumulate business logic
- become giant validation hubs or switchboards
- become god files

If roots grow, extract.

### 4.4 Workspace Crate Splits Need Justification

A crate split is justified when:

- boundaries are stable
- dependency direction becomes cleaner
- compile/test isolation is meaningful
- the crate has a coherent reason to exist independently

A crate split is not justified when:

- it centralizes a fake "shared" crate
- it introduces churn without real separation
- it turns into a dumping ground for generic helpers

### 4.5 No Dumping Grounds

Be suspicious of `utils`, `helpers`, `common`, `misc`, `shared`.

If such a module exists, each item inside it must have a crisp reason to be there.

---

## 5. Architectural Doctrine

### 5.1 Build Around a Stable Core

Core logic should not depend on:

- HTTP frameworks
- UI frameworks
- shell/process concerns
- database drivers
- serializer-specific transport types
- CLI formatting decisions

Outer layers depend on inner layers. Inner layers must not depend outward. If the domain knows too much about transport, persistence, or presentation, the architecture is collapsing.

### 5.2 No God Constructs

Never create:

- god structs
- manager blobs
- context bags carrying half the system
- service locators
- global mutable hubs
- "engine" objects that secretly own unrelated concerns

A construct is too large when it owns unrelated responsibilities, mixes orchestration with persistence and formatting, or becomes required by every test.

### 5.3 Single Source of Truth

If something is canonical, define it once:

- package identity
- domain invariants
- protocol semantics
- operation catalogs
- error classification systems
- config schema

Do not duplicate authority across layers.

### 5.4 Separate Domain from Representation

Do not casually collapse into one type:

- domain entities
- request/response DTOs
- storage rows/records
- UI payloads
- parser ASTs
- config structs

Serde convenience is not a license to erase boundaries.

### 5.5 Abstractions Must Be Honest

An abstraction is justified when it removes real duplication, clarifies a real boundary, captures a real invariant, narrows an API, or makes tests truer.

If it mostly hides confusion, remove it.

### 5.6 App vs Library Posture

Binary/CLI/application code and published library/crate code have different optimization priorities.

**App/binary**: optimize for delivery, operational clarity, and internal maintainability. Aggressive refactors, fast iteration, and breaking internal conventions are acceptable. The contract is operational correctness, not API stability.

**Published library/crate**: optimize for consumer clarity, MSRV stability, conservative public surface, and predictable behavior across versions. Treat every public API change, default feature change, and MSRV bump as a compatibility decision — not a refactor. Semver is a contract, not a suggestion.

Know which one you are building before designing its public surface. Do not apply library discipline to application code or application shortcuts to published crates.

---

## 6. Type-Driven Design

### 6.1 Types Are the Architecture

Make illegal states unrepresentable where practical.

Prefer:

- enums for closed states
- newtypes for meaningful identifiers and validated values
- smart constructors where invariants matter
- `Option<T>` for genuine optionality
- `Result<T, E>` for real failure

Do not rely on comments, "caller must" conventions, magic strings, sentinel values, or boolean soup.

**`Option` discipline**: Use `Option<T>` for genuine single-value optionality. Do not use `Option` as a function parameter to represent an optional argument — provide separate functions, a builder, or a distinct overload instead. An `Option` parameter shifts the `None`-handling burden to every caller without expressing why `None` is a valid state at that site.

A struct with multiple `Option` fields where fields are only meaningful in certain combinations is a hidden enum. Model it as one: a closed set of explicitly named variants is clearer, safer, and exhaustively matchable by the compiler — the constraints that would otherwise live in comments are encoded in types.

### 6.2 Eliminate Primitive Obsession

Do not use raw primitives for domain concepts:

- `String` for IDs, tokens, emails, route names, opaque user categories
- `u64`/`usize` for opaque identifiers without meaning
- `bool` for meaningful state transitions or policy choices

Name the meaning in the type system.

### 6.3 Public APIs Must Stay Sharp

Public APIs must be intentional, narrow, explicit, and stable by design.

Do not leak internals through convenience re-exports unless deliberate.
Do not expose generic complexity that callers do not need.

### 6.4 Trait Discipline

Use traits when they model a real capability boundary, polymorphic contract, or substitution point.

Do not:

- introduce traits for imagined future flexibility
- over-genericize until diagnostics become unreadable
- force dynamic dispatch where concrete or static dispatch is clearly better

Prefer static dispatch by default. Use dynamic dispatch when runtime composition genuinely requires it.

---

## 7. Ownership, Borrowing, and Mutation

The borrow checker is pressure toward better design. Do not hack around it.

### 7.1 Prefer the Lightest Correct Ownership

- `&str` over `String` when ownership is unnecessary
- `&[T]` over `Vec<T>` when ownership is unnecessary
- explicit ownership transfer only when it clarifies the model
- shared ownership (`Arc`) only when genuinely needed

### 7.2 Clone Discipline

A clone is acceptable when:

- ownership transfer is intentional
- boundary crossing requires ownership
- the value is small and the tradeoff is clear
- clarity materially improves

It is not the default escape from ownership pressure.

### 7.3 Mutation Must Stay Local

Keep mutable state constrained and visible.

Be suspicious of:

- sprawling `&mut self` methods that do unrelated work
- broad shared mutable state
- hidden interior mutability
- `Arc<Mutex<T>>` as a reflex instead of a deliberate tradeoff

Use synchronization only when the concurrency model genuinely requires it.

---

## 8. Error Handling

### 8.1 No Panic-Driven Production Logic

Avoid in production code:

- `unwrap()`
- `expect()`
- panic as control flow
- panicky indexing where failure is plausible

Exceptions: tests, examples, truly impossible internal invariants that are narrowly documented.

### 8.2 Errors Must Carry Meaning

Good errors tell the reader: what failed, where it failed, why it failed, what context matters, and whether it is caller error, environmental failure, or internal fault.

Do not flatten errors to strings. Do not erase source context.

### 8.3 Right Error Style for the Right Layer

Use typed errors (`thiserror`) for:

- libraries
- domain logic
- reusable components
- explicit protocol boundaries

Use `anyhow`-style aggregation for:

- binaries
- CLI entrypoints
- one-shot tasks
- tests where a typed error adds noise without value

---

## 9. Async and Concurrency

### 9.1 Async Is a Tool, Not an Identity

Use async where it buys something:

- I/O concurrency
- long-lived services
- multiplexed workloads

Do not make code async when synchronous code is clearer and sufficient.

### 9.2 Structured Concurrency

Every spawned task must have:

- an owner
- explicit shutdown and cancellation semantics
- error propagation behavior
- a clear reason to exist

Detached tasks require explicit justification and lifecycle management.

Do not block in async code. If blocking is necessary, isolate it explicitly with `spawn_blocking` or equivalent.

### 9.3 Public Async Trait Caution

Do not casually publish `async fn` in publicly reachable traits.

The returned future's auto-trait bounds (e.g. `Send`) become part of the long-term API contract. Make async trait contracts deliberate, not accidental.

---

## 10. Data, Serialization, and External Boundaries

### 10.1 Validate at the Edge

All external input is hostile until validated.

Validate early: HTTP input, CLI input, files, config, DB-loaded content, plugin inputs, RPC payloads, FFI input, deserialized data.

Do not let unchecked data drift inward. Do not scatter env access, string parsing, or ad hoc validation across the codebase.

### 10.2 Configuration Is a Real Boundary

- parse configuration once near startup
- validate eagerly
- represent config with typed structs, enums, and newtypes
- propagate typed config inward

Do not pass raw environment variables deep into the system or make config stringly typed.

### 10.3 Compatibility Is a Design Decision

If data crosses process, machine, storage, or version boundaries, compatibility is part of the design.

Treat schema changes, field renames, and wire semantics as intentional product decisions, not incidental edits.

**Serde and wire vocabulary**: Do not rely on serde defaulting to the Rust identifier when the wire vocabulary is an external contract. The Rust struct field name and enum variant name are not guaranteed to match the wire name — they are implementation identifiers, not wire vocabulary. Use `#[serde(rename = "...")]` on fields and variants explicitly when the wire name is a published contract. Treat those rename values as stable once the schema is published.

**Polymorphic discriminator placement**: For enums serialized with `#[serde(tag = "type")]` or `#[serde(tag = "type", content = "value")]`, place the tag configuration on the enum type declaration, not only on individual variants. This keeps the full dispatch contract visible at the natural extension point — adding a new variant forces a visit to the declaration, making it structurally harder to miss registration.

---

## 11. Performance

Default to sensible efficiency:

- avoid unnecessary allocation
- avoid unnecessary copying
- choose sane data structures
- keep hot paths simple
- avoid hidden temporary work

Order of concern:

1. correctness
2. asymptotic behavior
3. obvious waste
4. measured hot-path tuning
5. low-level tricks only if justified

Do not introduce complexity for hypothetical speed. If you add non-obvious performance machinery, have evidence or a hard systems reason.

---

## 12. Unsafe and FFI

Use safe Rust unless there is a real reason not to.

Do not introduce `unsafe` blocks, `unsafe fn`, unsafe traits, raw-pointer manipulation, or unsafe-based lifetime escapes unless:

1. the user explicitly requested it, or
2. the task structurally requires FFI or a low-level operation that safe Rust cannot express

Even then:

- keep unsafe regions as small as possible
- state why safe Rust is insufficient
- state the invariants precisely
- place a `// SAFETY:` comment immediately above or within the block

FFI boundaries must make ownership, layout, nullability, threading, lifetime, and error semantics explicit. Treat FFI as hostile terrain.

---

## 13. Testing

### 13.1 Tests Are Part of the Work

If behavior changed, tests should change too. Public behavior changed with no corresponding test change is incomplete until proven otherwise.

### 13.2 Test the Contract, Not the Accident

Test:

- invariants
- observable behavior
- failure modes
- boundary behavior
- concurrency or cancellation behavior when relevant

Do not overfit tests to incidental implementation details.

### 13.3 Right Test Mix

- unit tests for local logic
- integration tests for boundary behavior
- property tests where invariants matter
- snapshot tests only when they add clarity
- fuzzing for hostile-input surfaces when appropriate

### 13.4 Determinism Is Mandatory

Control: time, randomness, temp paths, environment, process-global state, network assumptions, concurrency timing.

Flaky tests are design failures.

### 13.5 Keep Test Seams Honest

Do not widen public APIs merely to make tests convenient.

Prefer crate-private helpers, focused builders, and boundary-driven tests.

---

## 14. Verification

### 14.1 Narrow Early, Full Before Done

During implementation, use narrow checks early and often:

```bash
cargo check
cargo check -p <crate>
cargo test -p <crate>
```

Before declaring completion, run the strongest relevant verification the repository expects.

Universal default:

```bash
cargo fmt --all
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Also run when relevant:

```bash
cargo doc --workspace --all-features
cargo test --workspace --doc
cargo test -- --ignored
cargo audit
cargo deny check
cargo udeps
cargo llvm-cov
cargo fuzz test <target>
```

Add `--locked` in CI to enforce that `Cargo.lock` is committed and current.

### 14.2 Rules

- warnings are failures
- skipped checks need an explicit reason
- if the repository has a stronger canonical command, use it
- do not say "done" while a required check is still red

---

## 15. CI and Project Automation

### 15.1 CI Must Mirror Local Verification Exactly

The canonical check command must pass locally and in CI with identical strictness.

Do not:

- soften `-D warnings` locally based on a `CI=true` environment variable
- create CI-only checks that cannot be reproduced locally
- weaken local verification to match CI behavior

"Passes locally, fails CI" is a workflow design failure. Fix it by making local verification strict — not by loosening CI.

### 15.2 Pin External Action Versions to Commit SHAs

Every third-party action in a CI workflow must be pinned to a full-length commit SHA, not a floating tag.

```yaml
# Good — immutable, supply-chain safe
uses: actions/checkout@abc123...def456  # v4.2.0

# Bad — mutable, redirectable
uses: actions/checkout@v4
```

Floating tags are silent supply-chain attack surfaces. Pinned SHAs are not.

### 15.3 Cache Rust Build Artifacts in CI

Every CI job that compiles Rust should cache the Cargo registry and build artifacts.

Without caching, every run recompiles all dependencies from source. For moderate dependency graphs this adds minutes to every PR feedback cycle.

Standard tool: `Swatinem/rust-cache`. Pin it to a commit SHA like everything else. For cross-compilation jobs, key the cache by target triple so different targets do not collide.

### 15.4 Set Explicit Job Timeouts

Every CI job should declare `timeout-minutes`.

No timeout means a hung compilation or flaky test can hold the queue for hours and burn runner quota with no recourse.

Typical ranges:
- lightweight matrix/scripting jobs: 5 minutes
- compile and test jobs: 20–30 minutes
- cross-compilation or release builds: 15–20 minutes

### 15.5 Cancel Stale Runs

Use concurrency groups with `cancel-in-progress: true` to abort obsolete runs when a new push arrives on the same branch.

```yaml
concurrency:
  group: ci-${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true
```

Without this, consecutive pushes pile up pending runs and waste quota.

### 15.6 Dependency Freshness: Gate vs. Automation

Two philosophies for keeping dependencies current:

**Sync gate** — `cargo outdated --exit-code 1` inside the CI check pipeline. Enforces currency before merge. Cost: any upstream patch release can break an unrelated PR overnight.

**Async automation** — Dependabot or Renovate opens PRs when deps fall behind. Does not block unrelated work. Requires explicit configuration.

Either is defensible. Running a sync gate with no automation is the worst of both worlds: updates are manual and they block everything else. If you enforce a sync gate, pair it with automated PR creation to feed it.

---

## 16. Documentation and Observability

### 16.1 Document Public APIs Intentionally

Rustdoc should explain where relevant: purpose, invariants, usage, examples, `# Errors`, `# Panics`, `# Safety`.

Do not weaken documentation enforcement to avoid doing the work.

### 16.2 Structured Observability

Prefer `tracing` spans and structured fields over ad hoc logging.

Rules:

- no secret leakage
- no leftover println debugging
- no noisy spam logs
- enough context to debug ownership, latency, retries, and failure propagation

---

## 17. Security

Assume all input is hostile, all boundaries are pressure points, and convenience will create vulnerabilities if unchecked.

Always:

- validate input at boundaries
- constrain authority
- reject malformed data clearly
- avoid path traversal and injection surfaces
- avoid accidental secret exposure
- keep authn/authz explicit
- fail safely

Parsers, plugin systems, loaders, deserializers, RPC boundaries, and FFI edges deserve extra suspicion.

---

## 18. Refactoring

Hard refactors are allowed. Do not preserve weak design purely to avoid breakage.

If you break something:

- break it intentionally
- update all affected layers, tests, and docs
- remove dead compatibility scaffolding
- leave the system cleaner than before

Half-migrations are worse than hard breaks.

Delete dead weight when the task allows: obsolete code, duplicate code, stale flags, fake abstractions, cargo-cult layers, stale TODOs that represent resolved structural debt.

---

## 19. Tauri and Rust-Backed UI

If the project uses Tauri or another Rust-backed UI stack:

- Rust owns the real logic; the UI is a projection layer
- frontend validation is UX, not trust
- important invariants remain enforced in Rust
- command APIs are strict, narrow boundaries
- filesystem, shell, and network access are security-sensitive capabilities
- command surfaces stay narrow and explicit

Do not move real rules into the frontend because it feels convenient.

---

## 20. Smell Radar

Stop and refactor when you see:

- god structs or god files
- manager blobs
- context bags
- dumping-ground modules
- bool parameter soup
- stringly typed domains
- duplicated parsing or validation
- panicky indexing
- hidden mutable global state
- traits introduced for imagined reuse
- one type serving domain, storage, wire, and UI concerns without deliberate justification
- tests that require constructing half the system to validate a tiny behavior
- code that "works" but cannot explain its own boundaries

These are structural debt signals, not harmless quirks.

---

## 21. Completion Bar

A task is complete only when all of the following are true:

- [ ] repository state was inspected before design
- [ ] a `<PLAN>` block was produced for any non-trivial task
- [ ] project and module shape still makes sense
- [ ] invariants are encoded in types where practical
- [ ] no new god constructs or dumping grounds were introduced
- [ ] manifest, features, and public exports remain coherent
- [ ] errors are intentional and contextual
- [ ] tests prove the changed behavior
- [ ] docs were updated where public behavior changed
- [ ] formatting is clean (`cargo fmt`)
- [ ] clippy is clean (zero warnings)
- [ ] required verification passes
- [ ] no obvious structural debt was knowingly left behind out of convenience

---

## 22. Self-Containment Principle

Source code, comments, Rustdoc, and `// SAFETY:` annotations must never reference the agent directive file by name, section number, or as justification for a design decision. Agent directive files are AI operational instructions, not developer documentation. Every design decision, safety invariant, and architectural constraint must be self-explanatory from the code, comments, and Rustdoc alone.

```rust
// Forbidden — references the agent directive file
// Per AGENTS-rust.md §8.1, no unwrap() here.

// Correct — self-explanatory
// Length is checked at entry; index is within bounds.
```

---

## 23. Incidental Observation Protocol

When reading a file surfaces a defect, a rule violation, or a clear improvement opportunity, record it in the project's designated observation log and continue the active task. Do not fix it in the current change, do not mention it in chat, do not interrupt the workflow. The log is a triage backlog reviewed by the project owner — not an action queue.

Each entry must record:
- a stable ID (random alphanumeric — do not use sequential numbers or derive from content),
- the date,
- a status (`OPEN`),
- the file and line range,
- a category (`DIRECTIVE` | `DEFECT` | `COVERAGE` | `SIMPLIFY` | `PERF`),
- what is wrong and why it matters,
- the current pattern or code excerpt,
- what change resolves it,
- the effort level (`TRIVIAL` | `MINOR` | `MODERATE`).

When an observation is fully resolved, update its entry in-place (`OPEN` → `ACTIONED`). Never delete entries. The log is a permanent audit trail.

The specific log file location, entry format, and any project-specific categories are defined per project in its own agent directive file.

---

## 24. Final Rule

Produce Rust where correctness is mechanically checkable through:

- explicit types
- explicit ownership
- explicit boundaries
- explicit errors
- explicit tests
- passing verification

If the code relies on implicit context, hidden assumptions, guessed dependency metadata, or opaque control flow to be considered "correct," it is not ready.
