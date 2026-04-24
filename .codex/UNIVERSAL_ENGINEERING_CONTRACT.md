# Universal Engineering Contract

This contract applies to all languages, runtimes, frameworks, tools, and repositories.

## 1. Systems over goals

The requested task is the entry point. The standard is to leave the touched system more coherent, more observable, and easier to change than it was before.

Do not treat generated code, a passing build, or a closed issue as the whole outcome. The outcome is a validated improvement to the system's working theory: what is true, what changes it, what proves it works, what depends on it, and what must not break.

Avoid orphan code: code that appears locally correct but has no clear owner, feedback loop, invariant, or understandable place in the system.

## 2. Build the minimum system map before touching code

Before making a non-trivial change, identify the relevant system theory. Keep this lightweight, but make it concrete enough that another engineer or agent could continue safely.

### 2.1 Truth

Ask:

- Where does the relevant state live?
- What is the canonical source of truth?
- Who is allowed to mutate it?
- What state is cached, derived, duplicated, denormalized, persisted, remote, or eventually consistent?
- Where can this value become stale, invalid, or contradictory?

Change the source of truth, not a symptom, unless the task is explicitly about presentation or derived behavior.

### 2.2 Evidence

Ask:

- What tells us the system is working?
- What would tell us it is failing?
- Which tests, assertions, type checks, contracts, logs, metrics, traces, dashboards, alerts, or reproducible checks cover this behavior?
- If feedback is missing, what is the smallest useful feedback loop to add?

A change without evidence is incomplete unless there is a clear, stated reason evidence cannot be added.

### 2.3 Consequence

Ask:

- What breaks if this file, function, module, class, endpoint, table, message, job, flag, or configuration disappears?
- Who calls it directly?
- Who depends on it indirectly through reflection, routing, serialization, dependency injection, schemas, generated code, conventions, plugins, events, queues, webhooks, cron jobs, dashboards, documentation, or human workflow?
- What is the blast radius across code, data, runtime behavior, users, and operations?

Do not rely only on intuition. Prove blast radius with the available tools: search, static analysis, dependency graphs, tests, traces, logs, schemas, build output, or runtime inspection.

### 2.4 Invariant

Ask:

- What must remain true after this change?
- What domain rule, security property, compatibility contract, performance bound, idempotency rule, ordering guarantee, data-shape guarantee, or user-visible behavior must not be violated?

State the invariant before changing behavior. Add or update executable checks for it where practical.

### 2.5 Preservation

Ask:

- Where should the discovered theory live after this work?

Preserve important knowledge in the most durable appropriate place: tests, names, types, schemas, comments, documentation, runbooks, architecture decision records, generated artifacts, or agent directive files. Do not leave essential system knowledge trapped in a chat transcript or temporary reasoning.

## 3. Red → Green → Refactor

For new behavior, start with the smallest failing proof of behavior: a test, assertion, contract check, type-level check, reproducible script, golden case, or manual verification path.

Then:

1. **Red:** demonstrate the missing or broken behavior.
2. **Green:** make the smallest coherent change that satisfies the proof.
3. **Refactor:** immediately simplify names, boundaries, structure, duplication, and control flow while keeping feedback green.

Passing is not finished. Understandable, coherent, and changeable is finished.

## 4. Boy Scout + Mikado

When touching existing code, leave the local area better than you found it.

Prefer small, safe, validated improvements:

- Rename unclear concepts.
- Extract coherent units.
- Inline needless indirection.
- Delete dead paths.
- Collapse accidental complexity.
- Remove obsolete compatibility shims when no real contract depends on them.
- Replace parallel definitions with derivation from the canonical owner.
- Strengthen tests, assertions, types, or runtime checks around changed behavior.

Use Mikado-style sequencing for broader change: identify the desired improvement, discover prerequisites, make the smallest safe prerequisite change, validate it, and continue only while each step remains understandable and reversible.

If a local refactor naturally unlocks a broader system-wide improvement, continue only while the scope remains controlled and evidence remains strong. Stop when the next improvement is a separate slice.

## 5. Architecture as preserved theory

Do not preserve architecture merely because it exists. Do not replace architecture merely because a new design seems cleaner in isolation.

Treat architecture as accumulated system theory. Preserve the parts that encode real constraints, useful boundaries, domain language, operational lessons, or compatibility contracts. Improve the parts that are accidental, duplicated, misleading, obsolete, or unnecessarily complex.

Architecture should emerge through repeated validated improvements, not speculative rewrites. When changing structure, make the new structure easier to explain, test, and modify than the old one.

## 6. Canonical ownership of contract facts

Shared contract facts must have exactly one canonical owner.

Contract facts include externally meaningful:

- identifiers;
- names and labels;
- limits and quotas;
- permissions and capabilities;
- status values and state-machine transitions;
- routes, event names, message types, and schema fields;
- error codes and user-visible contract text;
- configuration keys and feature flags;
- protocol, API, CLI, UI, database, and integration contracts.

Do not hard-code contract facts in parallel across code, interfaces, tools, tests, documentation, generated files, summaries, or error surfaces.

Any surface that exposes a contract fact must derive it from the canonical source or from generated artifacts rooted in that source. Build-time or test-time validation should fail on drift, missing registration, contradictory definitions, or references to contract facts outside the canonical owner.

When no canonical owner exists, create the smallest appropriate one before spreading the fact further.

## 7. State ownership and mutation discipline

Every meaningful piece of state needs an owner and a mutation policy.

Before changing stateful behavior, identify:

- the source of truth;
- all mutation paths;
- all readers;
- derived or cached copies;
- invalidation and reconciliation paths;
- concurrency, ordering, and idempotency assumptions;
- persistence, migration, and rollback implications.

Do not introduce a second source of truth. Do not patch derived state when the canonical state or mutation path is wrong. Do not add hidden state that future maintainers cannot locate or reason about.

## 8. Feedback must match risk

Use the cheapest feedback that proves the important behavior, but do not confuse cheap feedback with sufficient feedback.

A pure function may need a unit test. A protocol may need a contract test. A migration may need rollback validation. A distributed workflow may need integration coverage, idempotency checks, logs, metrics, and failure-mode tests.

When fixing a bug, reproduce it first if practical. When preventing recurrence, add the feedback that would have caught it.

## 9. Deletion and simplification require proof

Deleting code is good when the dependency theory is sound.

Before deleting or simplifying, check for:

- static references;
- dynamic references;
- generated references;
- serialized or persisted formats;
- migrations and historical data;
- external consumers;
- scheduled jobs and asynchronous workers;
- observability, alerting, and operations dependencies;
- documentation and human processes.

If safe deletion cannot be proven fully, reduce uncertainty with tooling and make the smallest reversible change.

## 10. Agent output contract

For non-trivial changes, produce more than a patch. Include a compact summary covering:

```text
Truth:
- Source of truth:
- Mutation paths:
- Derived/cached state:

Evidence:
- Existing feedback:
- Added or updated feedback:
- Manual verification, if any:

Consequence:
- Direct dependencies:
- Indirect or operational dependencies:
- Blast-radius judgment:

Invariant:
- Must remain true:
- How it is protected:

Preservation:
- Where the relevant theory was recorded:
```

Keep the summary proportional to the change. Small changes need small summaries. Risky changes need explicit reasoning.

## 11. Stop conditions

Stop when:

- the requested behavior is implemented;
- the relevant feedback is green;
- touched code is clearer, simpler, and easier to change;
- shared contract facts have a canonical owner;
- important invariants are protected;
- blast radius has been considered and checked with available tools;
- newly discovered system knowledge has been preserved in a durable place; and
- the next improvement is a separate slice.

Do not continue expanding scope after the next step stops being clearly connected, safe, and validated.
