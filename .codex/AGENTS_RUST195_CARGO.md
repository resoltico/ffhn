# Rust 1.95+ / Cargo Agent Protocol

This protocol governs agent work on Rust projects that target Rust 1.95 or newer and build with Cargo.

Scope: libraries, services, CLIs, daemons, backends, systems tools, proc-macro crates, FFI crates, WebAssembly crates, embedded or `no_std` crates, Rust-backed desktop apps, and mixed-language repositories with Rust surfaces.

Primary objective: produce Rust that is sound, explicit, type-driven, verifiable, maintainable, secure at boundaries, and aligned with the repository's actual compatibility contract.

Optimize in this order:

**soundness → invariants → ownership clarity → API compatibility → failure clarity → observability → performance where it matters → terseness**

Terseness loses to explicitness. Local convenience loses to correctness. Borrow-checker workarounds lose to a clear ownership model. Passing `cargo check` is not the finish line.

---

## 1. Repository intake

Before touching Rust code, inspect the repository's actual shape.

Always inspect the relevant subset of:

- `rust-toolchain.toml`, `rust-toolchain`, installed toolchain channel, and whether nightly is pinned;
- workspace and member `Cargo.toml` files;
- `Cargo.lock`, and whether the project is an application, library, or publishable workspace;
- `package.edition`, `package.rust-version`, workspace `resolver`, and workspace inheritance;
- feature flags, optional dependencies, target-specific dependencies, and default-feature policy;
- `.cargo/config.toml`, custom target settings, linker settings, environment assumptions, and aliases;
- `build.rs`, generated code, bindgen/cbindgen/prost/tonic/sqlx/diesel outputs, and checked-in generated artifacts;
- crate boundaries, public exports, module structure, trait definitions, and re-export surfaces;
- `unsafe` blocks, `unsafe fn`, FFI boundaries, `extern` blocks, `repr(...)` types, global state, and manual memory management;
- async runtime, thread ownership, channels, cancellation, shutdown, backpressure, and blocking boundaries;
- existing tests, doc tests, property tests, fuzz targets, Miri/Loom checks, benchmarks, CI, and project-specific verification commands.

Classify the touched crate before designing the change:

- **Published library:** MSRV, public API, SemVer, features, docs, and examples are contracts.
- **Internal library:** API evolution is easier, but invariants and ergonomics still matter.
- **Binary/service/CLI:** operational behavior, config, logs, exit codes, and runtime failure modes are contracts.
- **Proc macro/build tooling:** generated output, diagnostics, determinism, and compile-time cost are contracts.
- **FFI/embedded/WASM/no_std:** layout, panic behavior, allocation, target support, and host integration are contracts.

Do not assume repository state. Verify it.

---

## 2. Change loop

For every non-trivial change, apply the Universal Engineering Contract concretely in Rust terms.

### 2.1 Minimum system map

Before editing, identify:

```text
Truth:
- Source of truth for the relevant state, config, schema, generated artifact, feature flag, or protocol value:
- Mutation paths:
- Derived/cached/generated copies:

Evidence:
- Existing checks: cargo check/test/doc/clippy/fmt, contract tests, integration tests, property tests, fuzz/Miri/Loom, CI:
- Missing feedback worth adding:

Consequence:
- Direct Rust dependencies: callers, trait impls, re-exports, features, cfg arms, tests:
- Indirect dependencies: serialization, FFI, generated code, build scripts, CLI output, docs, dashboards, human workflows:

Invariant:
- Type, ownership, concurrency, memory-safety, protocol, or compatibility rule that must remain true:

Preservation:
- Where the learned theory should live: type, test, rustdoc, safety comment, module name, build check, generated artifact, README, runbook:
```

Keep the map lightweight. For trivial changes, do not turn it into ceremony. For risky changes, do not skip it.

### 2.2 Red → Green → Refactor

For new behavior, start with the smallest failing proof:

- unit test;
- integration test;
- doc test;
- compile-fail test where appropriate;
- property test;
- reproducible CLI invocation;
- fixture or golden file;
- Miri/Loom/fuzz reproduction;
- type-level or compile-time check.

Then make the smallest coherent change, and immediately refactor until the touched surface is simpler, clearer, and easier to change.

### 2.3 Compile-driven iteration

Work in small increments:

1. make one coherent change;
2. run the narrowest useful check, usually `cargo check -p <crate> --all-targets` or the repository's equivalent;
3. read the first real compiler error;
4. fix the root cause;
5. rerun the narrow check;
6. widen verification only after local shape is sound.

Do not pile up cascading errors and try to reason about all of them at once.

### 2.4 Root-cause fixes only

When verification fails:

- read the actual failure output;
- identify the type, ownership, lifetime, feature, cfg, build, dependency, or logic cause;
- fix that cause;
- rerun the narrowest relevant check;
- rerun full required verification before declaring completion.

Do not:

- guess at compiler failures;
- blindly apply compiler suggestions without understanding the ownership or API consequence;
- add `.clone()`, `Arc`, `Mutex`, `Box`, `RefCell`, `unwrap`, `expect`, wildcard matches, or broad trait bounds just to quiet the compiler;
- suppress lints unless the suppression is narrowly scoped, justified, and better than the alternative;
- claim completion while required checks still fail.

---

## 3. Rust 1.95+ baseline posture

### 3.1 Stable toolchain

Use the repository's pinned toolchain when present. Otherwise, assume stable Rust 1.95+ for projects governed by this protocol.

For new crates created under this protocol:

```toml
[package]
edition = "2024"
rust-version = "1.95"
```

For existing crates:

- do not raise `rust-version` without a concrete benefit and explicit compatibility judgment;
- treat `rust-version` as a public contract for published crates;
- preserve the existing edition unless the task is an edition migration or the repository clearly standardizes on Rust 2024;
- if moving to edition 2024, run the appropriate migration checks, then manually review semantics rather than treating `cargo fix --edition` output as design guidance.

Nightly is allowed only when the repository already pins nightly or the task explicitly requires an unstable capability. Nightly use must be isolated, named, justified, and wired consistently in local verification and CI.

### 3.2 Rust 2024 expectations

When using edition 2024, account for the edition's safety and semantics changes:

- `unsafe_op_in_unsafe_fn` warns by default; keep explicit `unsafe {}` blocks inside `unsafe fn`.
- `extern` blocks require `unsafe`.
- `export_name`, `link_section`, and `no_mangle` require unsafe attributes.
- references to `static mut` are denied by default; redesign around atomics, locks, `OnceLock`, or other safe state owners.
- `std::env::set_var`, `std::env::remove_var`, and Unix `CommandExt::before_exec` are unsafe; avoid mutating process environment after concurrency begins.
- `Future` and `IntoFuture` are in the prelude; avoid redundant imports unless they improve local readability.
- migration fixes are conservative. Review temporary lifetime changes, macro fragment changes, and never-type fallback implications deliberately.

### 3.3 Rust 1.95 language and library posture

Rust 1.95 adds useful stable tools. Use them when they make the code clearer, not merely because they are new.

- Prefer `cfg_select!` for readable compile-time configuration selection when the repository baseline is Rust 1.95+ and the pattern would otherwise need ad hoc `#[cfg]` branching or the `cfg-if` crate.
- Use `if let` guards in `match` arms when they make pattern-dependent conditions clearer. Remember that these guards do not contribute to exhaustiveness; the remaining arms must still handle all cases.
- Use collection insertion helpers such as `Vec::push_mut`, `Vec::insert_mut`, and the analogous `VecDeque`/`LinkedList` helpers when they avoid awkward indexing or double lookup while preserving clarity.
- Use `Atomic*::update` and `Atomic*::try_update` when they express an atomic read-modify-write loop more clearly than handwritten compare-exchange loops. State the ordering rationale.
- Use `std::hint::cold_path` only for genuinely cold paths where the intent is clearer than relying on profiling folklore.
- Custom JSON target specifications are not stable on Rust 1.95. If a custom target is required, pin and justify nightly rather than pretending the stable toolchain supports it.

### 3.4 Lint posture

For new crates, prefer a strong but practical lint baseline:

```rust
#![warn(missing_docs)]          // libraries and public API crates
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(unused_must_use)]
```

In `Cargo.toml`, prefer workspace-owned lint configuration where possible:

```toml
[lints.rust]
unsafe_op_in_unsafe_fn = "deny"
unused_must_use = "deny"

[lints.clippy]
all = "warn"
pedantic = "warn"
```

Do not enable noisy lint groups blindly in existing repositories. Match the repository's tolerance for warnings, then strengthen locally when it improves correctness and maintainability.

---

## 4. Cargo and manifest contract

### 4.1 `Cargo.toml` is a design surface

`Cargo.toml` communicates the crate's identity, compatibility contract, feature model, dependency graph, build posture, and publication behavior.

Rules:

- no unused dependencies;
- no invented crate names, versions, or feature flags;
- no accidental default-feature sprawl;
- no duplicated package metadata where the workspace is the canonical owner;
- no path/git/registry dependency changes without compatibility and supply-chain judgment;
- no feature or dependency edits without checking the feature graph and build impact;
- no build-script side effects without explicit `cargo::rerun-if-*` discipline.

Before modifying dependencies, verify actual crate versions and feature names through Cargo metadata, `cargo search`, crates.io, docs.rs, or authoritative upstream documentation. Do not hallucinate.

### 4.2 Resolver, edition, and MSRV

Cargo resolver behavior is part of the compatibility contract.

- Edition 2024 implies resolver `"3"`, which uses Rust-version-aware dependency resolution.
- In virtual workspaces, set `resolver = "3"` explicitly at the workspace root when the workspace intends Rust 2024 resolver behavior.
- `package.rust-version` is an MSRV contract, not decoration.
- Do not run `cargo update` casually in published libraries or applications with locked dependency expectations.
- If a dependency upgrade raises MSRV, surface it explicitly and decide whether that is acceptable.

### 4.3 Feature discipline

Features must be additive capability switches.

Use features for:

- optional integrations;
- optional heavy dependencies;
- platform-specific support;
- `std` vs `alloc` vs `no_std` boundaries;
- runtime choices when the crate genuinely supports more than one.

Do not use features to:

- hide broken code;
- encode negative logic;
- change public API incompatibly;
- silently change serialization formats;
- create untested combinatorial explosions;
- make a dependency optional only in the manifest while code still assumes it exists.

If feature combinations matter, verify them with the repository's feature-matrix tool or add one. `cargo hack` is appropriate when the repository already uses it or the feature matrix is non-trivial.

### 4.4 Lockfiles

Treat `Cargo.lock` according to crate posture:

- applications, services, CLIs, and workspaces with binaries should usually check in `Cargo.lock`;
- published libraries may or may not check it in depending on repository policy;
- do not remove or rewrite the lockfile as incidental cleanup;
- when the lockfile changes, understand whether the change is required by the task or accidental dependency drift.

### 4.5 Build scripts and generated code

`build.rs` is part of the build contract.

Build scripts must be deterministic, minimal, and explicit about inputs and outputs. Generated code must have a canonical source and a reproducible regeneration path.

When touching generated code:

- find the generator and its inputs;
- modify the canonical input where possible;
- regenerate with the repository's command;
- do not hand-edit generated output unless the repository explicitly treats it as source;
- verify that checked-in generated artifacts and source inputs are not drifting.

---

## 5. Type, API, and domain modeling

### 5.1 Make invalid states hard to express

Prefer Rust's type system over runtime conventions.

Use:

- enums for domain alternatives;
- newtypes for IDs, names, tokens, durations, counters, and units;
- structs for coherent state with real invariants;
- smart constructors when a value has validation rules;
- `NonZero*`, bounded numeric types, and domain-specific wrappers where they clarify invariants;
- `PhantomData` only when it encodes a real type-level relationship.

Avoid:

- boolean mode flags in public APIs;
- magic strings for states, capabilities, or protocols;
- parallel enums that shadow a canonical enum without a boundary reason;
- `Option` fields that together encode a hidden state machine;
- `String` where a semantic type or borrowed `str` boundary is clearer;
- widening visibility for tests or convenience.

### 5.2 Public API discipline

Every `pub` item is a promise unless the crate is clearly internal.

- Use the narrowest visibility: private, `pub(super)`, `pub(crate)`, then `pub`.
- Re-export deliberately. A re-export can become part of the public contract.
- Do not expose implementation types that prevent future refactoring.
- Avoid public type aliases that obscure ownership or error semantics.
- For extensibility, prefer sealed traits when downstream implementation would create compatibility hazards.
- For public enums that may grow, consider `#[non_exhaustive]` deliberately and document how callers should match.

### 5.3 Failure modeling

Use `Result` for fallible operations and `Option` for genuine absence.

- Domain/library errors should usually be explicit enums, often implemented with `thiserror`.
- Binary/CLI/glue layers may use `anyhow`/`eyre` when precise downstream matching is not part of the contract.
- Do not use panics for expected domain failures.
- Do not use `unwrap` or `expect` in production paths unless the invariant is obvious, local, and explained by the surrounding code or a short message.
- Error messages at user or API boundaries are contract surfaces; keep them stable or versioned when consumers depend on them.
- Preserve source errors when context matters; do not flatten error chains into strings too early.

### 5.4 Ownership and borrowing

The ownership model is part of the design.

- Prefer borrowing when the caller retains ownership and the callee only observes.
- Prefer owning when the value must outlive the call, move across threads, or become internal state.
- Add `Clone` only when duplication is semantically cheap and meaningful.
- Add `Copy` only for small value types where implicit duplication cannot hide cost or ownership meaning.
- Use `Arc` for shared ownership across threads, not as a borrow-checker escape hatch.
- Use `Rc` only for single-threaded shared ownership.
- Use `Cow` when the API genuinely benefits from accepting borrowed or owned data.
- Use `Box` for indirection, trait objects, or recursive types, not to hide design confusion.

Do not convert everything to owned `String`, `Vec`, `Arc`, or `'static` merely to make lifetimes disappear. If lifetimes are painful, revisit the boundary and state ownership.

### 5.5 Trait bounds and generics

Trait bounds are API contracts.

- Keep bounds as narrow as the implementation requires.
- Do not add `Clone`, `Default`, `Send`, `Sync`, `'static`, `Serialize`, or `Deserialize` bounds unless the function genuinely needs them.
- Prefer `impl Trait` for local API clarity when the concrete type should remain hidden.
- Prefer named generic parameters when callers or documentation need to reason about the relationship between types.
- Avoid blanket impls that block future specialization or create coherence hazards.

---

## 6. Unsafe, FFI, and memory discipline

### 6.1 Default stance

Safe Rust is the default. `unsafe` is an implementation boundary that must buy something concrete: FFI, performance with proven invariants, low-level memory layout, atomics, embedded constraints, or API capabilities impossible in safe Rust.

If a crate does not need unsafe, prefer:

```rust
#![forbid(unsafe_code)]
```

If a crate needs unsafe, require:

```rust
#![deny(unsafe_op_in_unsafe_fn)]
```

### 6.2 Unsafe block contract

Every unsafe block must be small and must have a nearby `SAFETY:` explanation covering:

- the exact invariant required;
- why it holds at that point;
- who maintains it in the future;
- what would make it invalid.

Do not write vague safety comments such as "caller guarantees this" unless the caller contract is also expressed in the function signature and rustdoc.

### 6.3 Unsafe functions

Every `unsafe fn` must document:

- `# Safety` preconditions;
- aliasing, lifetime, initialization, layout, threading, and ownership requirements;
- whether the function may be called concurrently;
- whether panic or unwind across the boundary is allowed.

Inside `unsafe fn`, still use explicit unsafe blocks for unsafe operations.

### 6.4 FFI boundaries

For FFI:

- use `unsafe extern` blocks;
- make ownership transfer explicit;
- define who allocates and who frees;
- avoid unwinding across FFI boundaries unless the ABI and project explicitly support it;
- use `repr(C)` only when layout compatibility is required;
- validate pointers, lengths, alignment, initialization, and lifetime assumptions;
- keep conversion between raw and safe types narrow and tested;
- consider Miri, sanitizer, or integration tests when memory invariants are subtle.

### 6.5 Global state

Global state must have an owner and a mutation policy.

Prefer `OnceLock`, `LazyLock`, atomics, or scoped dependency injection over mutable statics. Avoid `static mut`. Avoid process-wide environment mutation after threads, async runtimes, or libraries may have started.

---

## 7. Async, concurrency, and cancellation

### 7.1 Runtime ownership

Do not add an async runtime casually.

- Binaries and services may own a runtime.
- Libraries should usually expose async functions without constructing a runtime internally.
- Runtime choice is a contract when it appears in public types, features, or docs.
- Do not block inside async tasks unless using an explicit blocking boundary such as `spawn_blocking`.

### 7.2 Task lifecycle

Every spawned task must have an owner, purpose, and shutdown path.

Do not launch fire-and-forget work without:

- a retained `JoinHandle` or supervised task set;
- cancellation or shutdown signaling;
- error propagation or logging;
- backpressure where input can outpace processing.

### 7.3 Cancellation safety

For async code, identify what happens when a future is dropped.

- Do not hold locks across `.await` unless the lock type and scope are deliberately async-safe.
- Do not assume `select!` branches are cancellation-safe; verify the operation.
- Keep transactions, locks, and partial writes scoped so cancellation cannot leave corrupt state.
- Prefer explicit state machines when retry, rollback, or idempotency matters.

### 7.4 Channels and shared state

- Prefer bounded channels unless unbounded growth is proven safe.
- Document message ownership and shutdown semantics.
- Use `Mutex`, `RwLock`, atomics, or channels according to the invariant, not habit.
- Do not use `Arc<Mutex<T>>` as a default architecture. Sometimes it is right; often it is an unmodeled ownership problem.
- For atomics, state the memory ordering rationale. Do not use `SeqCst` as a substitute for understanding.

### 7.5 Testing concurrency

For concurrency-sensitive code, ordinary tests are often insufficient. Use the strongest practical feedback:

- Loom for interleaving-sensitive synchronization logic;
- Miri for undefined behavior and aliasing-sensitive unsafe code;
- stress tests for operational timing bugs;
- deterministic fake clocks or schedulers where available;
- integration tests for shutdown and cancellation paths.

---

## 8. Boundaries, protocols, and observability

### 8.1 Serialization is a contract

Serialization shape is not an implementation detail once external systems, stored data, or users depend on it.

- Do not derive `Serialize`/`Deserialize` on domain types when the wire format should evolve independently.
- Use DTOs or wire types when the external shape differs from the domain model.
- Make enum-to-wire mapping explicit where spelling, casing, aliases, or compatibility matter.
- Preserve backward compatibility for stored or external formats unless the task explicitly changes the contract.
- Add golden tests for important wire formats.

### 8.2 CLI and process boundaries

For CLIs and process integration:

- exit codes are contracts;
- stdout/stderr separation is a contract;
- human output and machine-readable output should not be casually mixed;
- environment variables and config keys must have canonical owners;
- secrets must not appear in logs, panic messages, debug output, or error chains.

### 8.3 Configuration and platform gates

Configuration facts must be canonical.

- Use typed config structs at the boundary.
- Validate config once, early, and explicitly.
- Use `cfg_select!`, `#[cfg]`, and target-specific dependencies deliberately.
- Do not duplicate platform names, feature names, environment variable names, or protocol constants across code and docs.
- Test platform-specific code paths where feasible. If not feasible locally, preserve the verification story in CI or docs.

### 8.4 Observability

For services and operational tools, feedback must survive production.

- Prefer structured logging/tracing at boundaries and state transitions.
- Do not log secrets or high-cardinality values casually.
- Attach context to errors close to where information is available.
- Add metrics or traces for behavior whose correctness cannot be inferred from tests alone.
- When fixing an incident-prone path, add the signal that would reveal recurrence.

---

## 9. Testing and verification

### 9.1 Verification ladder

Use the cheapest check that proves the relevant behavior, then widen according to risk.

Common ladder:

```bash
cargo fmt --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace --doc
cargo doc --workspace --no-deps
```

Adapt the ladder to the repository. Do not force `--all-features` when features are intentionally mutually exclusive; use the repository's feature matrix instead.

### 9.2 What to test

Test behavior and invariants, not implementation trivia.

Prioritize:

- domain invariants and edge cases;
- parser/serializer round trips and golden outputs;
- error cases and failure messages at user/API boundaries;
- feature-flag combinations that change behavior;
- concurrency cancellation and shutdown;
- FFI safety contracts;
- migration and backward compatibility behavior;
- bug reproductions before fixes.

### 9.3 Property, fuzz, and snapshot tests

Use stronger test forms when ordinary examples miss the risk:

- property tests for algebraic invariants, parsers, encoders, and state transitions;
- fuzz tests for parsers, protocol inputs, FFI boundaries, and unsafe code;
- snapshot/golden tests for user-facing output or wire formats, with deliberate review of changes;
- compile-fail tests for macros, public API constraints, and type-level guarantees.

### 9.4 Rustdoc and examples

Rustdoc is executable documentation when examples are doc tests.

Public APIs should document:

- purpose;
- errors;
- panics;
- safety preconditions;
- cancellation behavior for async APIs where relevant;
- feature flags or platform limitations;
- examples for non-obvious use.

Do not write examples that require unstated global state, network availability, or timing assumptions unless marked and justified.

---

## 10. Refactoring, deletion, and module design

### 10.1 Coherent repair

When a local patch exposes an incoherent module boundary, type model, or feature contract, fix the smallest coherent area rather than stacking workarounds.

Examples of coherent repair:

- replace stringly typed state with an enum and update the affected matches;
- move validation into a smart constructor and remove scattered checks;
- split a DTO from a domain type when serialization concerns are leaking inward;
- extract a module when a file mixes unrelated responsibilities;
- collapse a trait that has only one implementation and no current abstraction value.

### 10.2 Compatibility-aware refactoring

Refactor private/internal code aggressively when evidence stays green. Refactor public or published surfaces deliberately.

Before changing public API:

- check downstream compatibility promises;
- preserve SemVer where applicable;
- add deprecation paths when needed;
- update rustdoc and examples;
- verify feature flags and re-exports.

### 10.3 No god constructs

A god construct concentrates unrelated responsibilities.

Refactor:

- god modules that mix parsing, validation, storage, transport, and presentation;
- god structs with many optional fields representing multiple states;
- god enums that collapse unrelated protocols into a single catch-all type;
- god traits with broad, unrelated method sets;
- god functions with named comment phases that should be named helpers.

Extraction must improve cohesion, not merely reduce line count.

### 10.4 Safe deletion

Before deleting Rust code, check:

- direct references with search and compiler feedback;
- public exports and downstream API implications;
- feature-gated or cfg-gated references;
- proc macro or generated references;
- serialization formats and stored data;
- FFI symbols, `no_mangle`, exported names, and linker scripts;
- build scripts, examples, tests, benches, docs, CI, and human workflows.

Deleting dead code is good. Deleting untraced contract surface is breakage.

---

## 11. CI and project automation

### 11.1 CI mirrors local verification

The canonical verification path must be runnable locally and in CI with the same strictness. Do not create CI-only checks that developers or agents cannot reproduce.

### 11.2 Toolchain pinning

Use `rust-toolchain.toml` for repository toolchain policy when the project needs a specific toolchain, components, or targets.

CI should install the same toolchain and components used locally, such as:

- `rustfmt`;
- `clippy`;
- target triples;
- Miri/nightly only when explicitly part of the project policy.

### 11.3 Supply-chain discipline

- Pin third-party CI actions to immutable commit SHAs where repository policy requires supply-chain hardening.
- Do not add Git dependencies casually.
- Use `cargo audit`, `cargo deny`, SBOM generation, or equivalent checks when the repository already has them or the risk profile justifies them.
- Treat dependency updates as behavior changes unless proven otherwise.

### 11.4 Build reproducibility

- Avoid build scripts that depend on ambient machine state.
- Keep generated files reproducible.
- Use `--locked` or `--frozen` in CI when the lockfile is a contract.
- Do not rely on globally installed tools when the repository provides `just`, `xtask`, `cargo make`, `mise`, `nix`, or another pinned workflow.

---

## 12. Documentation and self-containment

### 12.1 Rustdoc requirements

For public API crates:

- public types, traits, functions, modules, and macros need rustdoc unless repository policy says otherwise;
- unsafe APIs require `# Safety`;
- fallible APIs should document errors;
- panicking APIs should document panics;
- async APIs with non-obvious cancellation behavior should document cancellation safety;
- feature-gated APIs should document the feature.

### 12.2 Comments

Comments should explain non-obvious invariants, safety, compatibility, or operational constraints. Do not comment what the code already says.

Good comments explain why a seemingly simpler change is wrong, where an invariant is maintained, or what external contract constrains the implementation.

### 12.3 Self-containment

Source code, rustdoc, comments, and project documentation must never reference the agent directive file by name, section, or as justification for a design decision.

Agent directive files are operational instructions for agents. Code and docs must stand on their own.

```rust
// Forbidden: references the agent protocol as justification.
// Per AGENTS.md, do not use a wildcard match here.

// Correct: self-contained engineering reason.
// No wildcard arm: adding a new state must force every transition table to be reviewed.
```

---

## 13. Incidental observation protocol

When reading a file surfaces a defect, rule violation, or clear improvement opportunity unrelated to the active task, record it in the project's designated observation log and continue the active task.

Do not fix unrelated observations in the current change unless they are prerequisites for correctness. Do not interrupt the workflow to discuss every incidental finding.

Each observation should record:

- stable ID;
- date;
- status;
- file and line range;
- category;
- what is wrong and why it matters;
- current pattern or excerpt;
- resolving change;
- effort level.

If the project has no observation log, include only high-value observations in the final summary when they affect future safety or maintainability.

---

## 14. Pre-output checklist

Run this before declaring completion.

### System theory

- Truth: is the source of truth identified and changed at the right layer?
- Evidence: did you add or run feedback proportional to risk?
- Consequence: did you trace direct and indirect blast radius?
- Invariant: is the important invariant protected by type, test, assertion, or documented contract?
- Preservation: did important theory land somewhere durable?

### Rust semantics

- Are domain alternatives modeled with types rather than strings, flags, or scattered conventions?
- Are ownership and borrowing choices semantically justified?
- Are trait bounds no wider than needed?
- Are public APIs narrow, documented, and compatible with crate posture?
- Are `Option`, `Result`, and panic behavior used for their proper meanings?

### Cargo and features

- Are edition, resolver, MSRV, and feature changes deliberate?
- Are dependency versions and feature names verified?
- Are features additive and tested where meaningful?
- Did lockfile changes happen only when justified?
- Are generated artifacts and build scripts in sync with their canonical inputs?

### Unsafe and concurrency

- Is unsafe absent where unnecessary?
- Does every unsafe block or unsafe function have a real safety contract?
- Are task lifecycles, cancellation, blocking, locks, channels, and shutdown paths explicit?
- Are atomic orderings justified?
- Did you avoid global mutable state or give it a clear owner?

### Verification

- Did the narrow relevant check pass?
- Did verification widen when the change widened?
- Are formatting, linting, tests, doc tests, or stronger tools run as appropriate?
- Are remaining failures unrelated and explicitly stated?
- Is the touched Rust surface clearer and easier to change than before?
