# Universal Engineering Contract

**Version:** 2.1.0
**Updated:** 2026-04-30
**Applies to:** all languages, runtimes, frameworks, tools, and repositories.
**Companion:** `DOMAIN_DRIVEN_DESIGN_LENS.md` for changes that touch business meaning, business rules, workflows, state transitions, or cross-context integration. Loaded on demand, not by default.

## 0. What this contract is, and is not

This contract is a method. Per Naur (*Programming as Theory Building*, 1985), no method substitutes for the *theory* held by the people who build and maintain a system — the tacit grasp of how the code maps to the world, why each part is what it is, and how new demands can be absorbed without destroying structure. Theory cannot be fully written down, and revival of a program from its artifacts alone is, in Naur's words, "strictly impossible."

The purpose of this contract is therefore not to capture theory, but to keep its absence visible. It is a discipline for working agents — human or otherwise — who are *transient theory-holders*: they enter cold, build a partial theory of the slice they touch, and leave. That transience makes two things obligatory:

1. **Surface the tacit gap.** Where the change depends on theory the agent does not fully have, say so. Do not paper over it with confident output.
2. **Re-cue the next reader.** Leave artifacts that help the next person reconstruct the relevant slice — knowing those artifacts are aids to a theory that lives elsewhere, not the theory itself.

A passing build, a closed issue, or a generated patch is not the outcome. The outcome is a validated improvement to the system's working theory: what is true, what changes it, what proves it works, what depends on it, why it is the way it is, and what must not break.

Avoid orphan code: code that appears locally correct but has no clear owner, feedback loop, justification, invariant, or understandable place in the system.

When the work touches business meaning, treat the Universal Engineering Contract as the operating discipline and the Domain-Driven Design Lens as the model-design discipline. Do not force domain ceremony onto mechanical infrastructure work.

## 1. Build the minimum system map before touching code

Before making a non-trivial change, identify the relevant system theory — concretely enough that another engineer or agent could continue safely, lightly enough that the map does not become its own artifact.

The map has six concerns. Treat them as questions, not as a form to fill in. A seventh concern (§1.7) applies only when the change touches business meaning.

### 1.1 Truth — where does state live?

- Where does the relevant state live?
- What is the canonical source of truth?
- Who is allowed to mutate it?
- What is cached, derived, duplicated, denormalized, persisted, remote, or eventually consistent?
- Where can this value become stale, invalid, or contradictory?

Change the source of truth, not a symptom, unless the task is explicitly about presentation or derived behavior.

### 1.2 Evidence — what tells you it works?

- What tells us the system is working?
- What would tell us it is failing?
- Which tests, assertions, type checks, contracts, logs, metrics, traces, dashboards, alerts, or reproducible checks cover this behavior?
- If feedback is missing, what is the smallest useful loop to add?

A change without evidence is incomplete unless there is a clear, stated reason evidence cannot be added.

### 1.3 Consequence — what breaks if you delete it?

- What breaks if this file, function, module, class, endpoint, table, message, job, flag, or configuration disappears?
- Who calls it directly?
- Who depends on it indirectly through reflection, routing, serialization, dependency injection, schemas, generated code, conventions, plugins, events, queues, webhooks, cron jobs, dashboards, documentation, or human workflow?
- What is the blast radius across code, data, runtime behavior, users, and operations?

Do not rely only on intuition. Prove blast radius with the available tools: search, static analysis, dependency graphs, tests, traces, logs, schemas, build output, or runtime inspection.

### 1.4 Invariant — what must remain true?

- What domain rule, security property, compatibility contract, performance bound, idempotency rule, ordering guarantee, data-shape guarantee, or user-visible behavior must not be violated?

State the invariant before changing behavior. Add or update executable checks for it where practical.

### 1.5 Justification — why is each touched part the way it is?

This is Naur's criterion: a programmer who possesses the theory of a program can explain *why* each part is what it is, not merely what it does, and can ground that explanation in the affairs of the world the program maps to.

For each non-trivial part you touch, ask:

- Why does this exist?
- What real-world fact, constraint, history, or domain rule is it the response to?
- What alternatives were available, and what would have made one of them right instead?

If the answer is not available — from code, history, conversation, or reasoning — say so explicitly. A change made without justification is a change whose blast radius cannot be estimated, because you do not know what the code was protecting.

### 1.6 Re-cueing — what must the next reader be able to rebuild?

The relevant theory cannot be fully written down. What can be written down is the set of cues that help the next reader — human or agent — rebuild the slice of theory needed to act safely.

- What did this change depend on that is not obvious from the diff?
- Where should those cues live so they survive: tests, names, types, schemas, comments where the *why* is non-obvious, runbooks, ADRs, architecture notes, agent directive files?
- What part of the relevant theory is not expressible in artifacts, and who currently holds it?

Do not leave essential cues trapped in a chat transcript or temporary reasoning. Equally, do not pretend an artifact transfers a theory it can only re-cue.

### 1.7 Domain meaning gate (conditional)

Apply this subsection only when the change touches business meaning. This includes domain state, business rules, workflow names, commands, domain events, permissions, policies, calculations, lifecycle transitions, user-facing business terms, or integration contracts between models. For purely mechanical work — build wiring, generic plumbing, infrastructure with no business meaning — skip §1.7 and load nothing further.

When it applies, ask:

- Which Bounded Context gives the touched terms their meaning?
- What local Ubiquitous Language is being added, corrected, renamed, or protected?
- Are any terms overloaded elsewhere in the organization or codebase?
- Is the work Core Domain, Supporting Subdomain, Generic Subdomain, or non-domain infrastructure?
- Which Aggregate, Entity, Value Object, Domain Service, Factory, Repository, command, or Domain Event owns the behavior?
- What true invariant is being protected, and where is the transactional consistency boundary?
- Is a foreign model entering this context? If yes, where is the translation boundary, Published Language, Open Host Service, or Anticorruption Layer?

Do not guess these answers into existence. If the context or language is not known, state the uncertainty and leave a durable cue for the next theory-holder. A vague class name, duplicated status enum, or copied integration DTO can become a false model.

The full discipline lives in the companion lens. Load it when this gate fires.

## 2. Red → Green → Refactor

For new behavior, start with the smallest failing proof of behavior: a test, assertion, contract check, type-level check, reproducible script, golden case, or manual verification path.

1. **Red:** demonstrate the missing or broken behavior.
2. **Green:** make the smallest coherent change that satisfies the proof.
3. **Refactor:** immediately simplify names, boundaries, structure, duplication, and control flow while keeping feedback green.

Passing is not finished. Understandable, coherent, justified, and changeable is finished.

## 3. Boy Scout + Mikado

When touching existing code, leave the local area better than you found it. Prefer small, safe, validated improvements:

- Rename unclear concepts.
- Extract coherent units.
- Inline needless indirection.
- Delete dead paths.
- Collapse accidental complexity.
- Remove obsolete compatibility shims when no real contract depends on them.
- Replace parallel definitions with derivation from the canonical owner.
- Strengthen tests, assertions, types, or runtime checks around changed behavior.

Use Mikado-style sequencing for broader change: identify the desired improvement, discover prerequisites, make the smallest safe prerequisite change, validate it, and continue only while each step remains understandable and reversible.

If a local refactor naturally unlocks a broader improvement, continue only while scope remains controlled and evidence remains strong. Stop when the next improvement is a separate slice. Naur's warning applies: improvements made without the theory tend to become "amorphous additions" that destroy structure even when each individual change looks correct.

## 4. Architecture as preserved theory

Do not preserve architecture merely because it exists. Do not replace architecture merely because a new design seems cleaner in isolation.

Treat architecture as accumulated system theory. Preserve the parts that encode real constraints, useful boundaries, domain language, operational lessons, or compatibility contracts. Improve the parts that are accidental, duplicated, misleading, obsolete, or unnecessarily complex.

Architecture should emerge through repeated validated improvements, not speculative rewrites. When changing structure, make the new structure easier to explain, justify, test, and modify than the old one.

For domain-heavy work, architecture must protect the domain model rather than replace it. Layered architecture, hexagonal architecture, REST, messaging, CQRS, event sourcing, data grids, and other styles are not the model. They are support structures. Use them only when they reduce real risk or satisfy real quality demands. The domain model should remain expressible in the local Ubiquitous Language, insulated from persistence, transport, UI, and foreign models where practical.

## 5. Canonical ownership of contract facts

Shared contract facts must have exactly one canonical owner. Which facts qualify is itself a domain judgment, not a mechanical rule.

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

Do not hard-code contract facts in parallel across code, interfaces, tools, tests, documentation, generated files, summaries, or error surfaces. Any surface that exposes a contract fact must derive it from the canonical source or from generated artifacts rooted in that source. Build- or test-time validation should fail on drift, missing registration, contradictory definitions, or references to contract facts outside the canonical owner.

When no canonical owner exists, create the smallest appropriate one before spreading the fact further.

For domain work, canonical ownership is context-sensitive. A term, status, event, or identifier may have one meaning in one Bounded Context and a different meaning in another. Do not create a false global owner for language that is only locally true. Across Bounded Contexts, the canonical owner may be a Published Language, stable API resource, event schema, command contract, Open Host Service, or translation map, not a shared domain class.

## 6. State ownership and mutation discipline

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

For domain work, identify the Aggregate or process that owns each invariant. Aggregates are consistency boundaries, not object graphs. Keep them as small as the true invariant permits. Prefer referencing other Aggregates by identity and using eventual consistency outside the boundary unless a real business rule demands atomic consistency.

## 7. Feedback must match risk

Use the cheapest feedback that proves the important behavior, but do not confuse cheap feedback with sufficient feedback.

A pure function may need a unit test. A protocol may need a contract test. A migration may need rollback validation. A distributed workflow may need integration coverage, idempotency checks, logs, metrics, and failure-mode tests.

When fixing a bug, reproduce it first if practical. When preventing recurrence, add the feedback that would have caught it.

For domain work, evidence should use business examples, not merely technical assertions. Prefer tests and executable examples that state the local language: given a domain situation, when a command or event occurs, then the invariant or state transition holds. If domain expert validation is required and unavailable, record that gap explicitly.

## 8. Deletion and simplification require proof

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

## 9. Agent output contract

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

Justification:
- Why each touched part is the way it is:
- Known gaps in justification (theory the agent does not have):

Re-cueing:
- Cues left for the next reader, and where:
- Theory that could not be expressed in artifacts, and who currently holds it:
```

When the change touches domain meaning, also include a proportional domain-design block:

```text
Domain design lens:
- Domain / subdomain:
- Core / Supporting / Generic / non-domain judgment:
- Bounded Context:
- Ubiquitous Language terms changed or clarified:
- Term collisions or foreign concepts:
- Aggregate or consistency boundary affected:
- Entity / Value Object / Domain Service / Factory / Repository choices:
- Commands, workflows, or Domain Events affected:
- Cross-context relationship, if any:
- Published Language, Open Host Service, or Anticorruption Layer, if any:
- Domain examples, tests, or expert validation used:
- Known model gaps:
```

Keep the summary proportional to the change. Small changes need small summaries. Risky changes need explicit reasoning. The "Known gaps" and "Theory that could not be expressed" lines are first-class outputs, not optional caveats — silence on them claims a theory the agent does not have.

## 10. Stop conditions

Stop when:

- the requested behavior is implemented;
- the relevant feedback is green;
- touched code is clearer, simpler, more justified, and easier to change;
- shared contract facts have a canonical owner;
- important invariants are protected;
- blast radius has been considered and checked with available tools;
- justification gaps have been surfaced rather than silently closed;
- newly discovered cues have been left in a durable place;
- for domain changes, the local language, Bounded Context, invariant owner, and cross-context translation boundary are explicit enough for the next change; and
- the next improvement is a separate slice.

Do not continue expanding scope after the next step stops being clearly connected, safe, and validated.
