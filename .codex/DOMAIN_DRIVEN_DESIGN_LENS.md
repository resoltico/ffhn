# Domain-Driven Design Lens

**Version:** 1.0.0
**Updated:** 2026-04-30
**Companion to:** `UNIVERSAL_ENGINEERING_CONTRACT.md` v2.1+
**Loaded:** on demand, when a change fires the UEC §1.7 domain-meaning gate

## 0. Placement

The Universal Engineering Contract governs safe engineering work — truth, evidence, consequence, invariant, justification, re-cueing. This file governs domain-modeling work — language, model boundaries, strategic and tactical design, consistency boundaries, integration between models.

Use it when a change touches business meaning, business rules, business state, workflows, commands, domain events, policies, permissions, calculations, lifecycle transitions, user-facing business terms, or integration between domain models.

Do not use it for purely mechanical work — build-script cleanup, generic plumbing, static UI cosmetics, infrastructure changes with no domain meaning. Domain ceremony on mechanical work is the failure mode this discipline most often produces in agent hands.

This is a reference manual loaded on demand, not a checklist to complete. Apply only what the change actually needs.

## 1. Triage — do you need this lens at all?

Run before reading further. If the answer is "no" or "light", skip the heavy chapters.

### Apply heavy

When most of these are true:

- the domain is business-critical;
- the domain gives the product its competitive advantage;
- domain experts use nuanced language;
- the same word means different things in different parts of the business;
- rules, workflows, policies, permissions, calculations, or state transitions are non-trivial;
- incorrect behavior would harm money movement, compliance, security, identity, authorization, operations, or trust;
- the model must evolve for years;
- multiple systems or teams must integrate without sharing one model;
- the current code shows anemic objects, transaction scripts, unclear service methods, duplicated status values, or DTOs leaking everywhere.

### Apply light

- supporting subdomain;
- moderate complexity;
- a few meaningful rules or state transitions;
- mostly need clean naming, good tests, and one or two explicit boundaries.

### Skip

- low-risk CRUD;
- simple data-entry features with little behavior;
- static content, scaffolding, build tooling, framework glue;
- generic infrastructure;
- a generic product, library, or service solves the problem adequately;
- the ceremony would make the design less clear than the problem.

The discipline is an investment. Spend it where the business return is real.

## 2. The DDD-Lite trap

The most common failure: using Entities, Repositories, Services, and other tactical shapes while skipping Ubiquitous Language, Bounded Contexts, Context Maps, and domain-expert collaboration. This produces the form of the discipline without its value.

If the team cannot answer "which Bounded Context is this in?", "what is the local Ubiquitous Language?", and "which Aggregate owns this invariant?", the work is DDD-Lite. Stop and answer those before more tactical work.

## 3. Mindset

- The model lives in the code and tests, not in glossaries or diagrams. If the code does not use the language, the language is not implemented.
- The model is not the database. Data matters; the database does not get to dictate the language of the domain.
- Domain experts and developers are one team. Do not translate business language into a private technical vocabulary and treat the translation as superior.
- Examples beat abstractions. When a term or rule is unclear, ask for concrete cases.
- Modeling is iterative. Expect the first model to be wrong. Improve through conversation, examples, tests, and refactoring.

## 4. Strategic design

Strategic design decides where models belong and how they relate. Without strategic design, tactical patterns are easily misused.

### 4.1 Domain and subdomains

A Domain is the sphere of business activity being modeled. Divide a large domain into subdomains. Three types:

- **Core Domain** — strategically central, differentiating, worth deep modeling.
- **Supporting Subdomain** — necessary, somewhat specialized, but not the main competitive advantage.
- **Generic Subdomain** — common across many businesses; usually better bought, reused, or implemented simply.

Invest the most modeling effort in the Core Domain. Keep Supporting clean but proportionate. Avoid custom-building Generic Subdomains unless they are strategically important here. Do not let generic concerns pollute the Core Domain.

### 4.2 Bounded Context

A Bounded Context is the boundary inside which a model and its Ubiquitous Language are valid.

It is *not automatically*: a microservice, a database, a repository, a package, a deployment unit, a team, a namespace, a module, or an API. It may align with any of those. Its primary purpose is **semantic boundary**.

Inside a Bounded Context: "When we say this word here, we mean exactly this."

Right size: large enough to contain a coherent model, small enough that the language stays crisp.

- **Too large:** vague terms; teams argue over overloaded words; generic and supporting concerns swamp the Core; everything depends on everything.
- **Too small:** related concepts are split prematurely; integration overhead dominates; a coherent language fragments; every business action becomes a distributed workflow.

Right-sized contexts are discovered by language, cohesion, dependencies, and business capability — not by arbitrary technical slicing.

Subdomains describe the *problem space*. Bounded Contexts describe the *solution-space* model boundaries. They often align but not always; do not confuse business decomposition with deployment topology.

### 4.3 Ubiquitous Language

Team language used by domain experts and developers in a Bounded Context. It includes nouns, verbs, adjectives, lifecycle names, policies, commands, events, examples, invariants, scenarios, and explicitly rejected terms.

It must appear in code names, tests, command names, event names, module names, API resources where the API expresses domain meaning, schemas, and conversation. A glossary is useful but insufficient — if the code does not use the language, the language is not current.

Capture by: scenario workshops, expert interviews, example-based requirements, Given/When/Then tests, command and event catalogs, glossary entries with examples, refactoring names in code, rejecting misleading technical names.

Avoid: a large dictionary no one uses; database table names treated as the business language; generated bean-style objects treated as the model; one enterprise-wide vocabulary that erases local meaning; developers privately renaming business concepts.

### 4.4 Context Map

A Context Map shows relationships between Bounded Contexts. It is both a sketch for discussion and the concrete code, contracts, and translators that implement the relationships. Integration is social, organizational, and technical. Systems fail because teams assume relationships that do not exist.

For every edge ask:

- Which context is upstream? Which is downstream?
- Who controls the model or API?
- Who can request changes?
- What crosses the boundary — commands, events, resources, files, schemas, DTOs?
- Is translation needed?
- What happens when the upstream model changes?
- What is the release coordination model?
- What pattern describes the relationship?

Patterns:

| Pattern | Use when | Risk |
|---|---|---|
| **Partnership** | Two teams must succeed or fail together | Requires strong communication; schedules become linked |
| **Shared Kernel** | Two teams genuinely share a small, governed model subset | Tight coupling; can become a disguised shared enterprise model |
| **Customer–Supplier** | Downstream depends on upstream; downstream needs influence upstream planning | If upstream does not actually commit, becomes Conformist in practice |
| **Conformist** | Downstream adopts upstream's model because it lacks leverage | Avoid for Core Domain; corrupts the local model |
| **Anticorruption Layer (ACL)** | Local model matters; upstream is foreign, muddy, unstable, or too technical | Translation cost |
| **Open Host Service** | Upstream provides a stable protocol for many consumers | Public host interface is itself a contract; do not dump the internal model |
| **Published Language** | Documented shared exchange language (media type, schema, event format) | The exchange language is not the internal model of either side |
| **Separate Ways** | Integration value is low; duplication or independence is cheaper | None — but verify the assumption |
| **Big Ball of Mud** | An existing tangle | Draw a boundary around it; do not pretend DDD lives inside; prevent spread |

## 5. Tactical design

Tactical patterns express the model inside a Bounded Context. Apply *after* strategic design, not instead of it.

Before asking "Is this an Entity or Value Object?", ask "Which Bounded Context are we in, and what does the local language say?"

### 5.1 Aggregate — the most important pattern

An Aggregate is a *transactional consistency boundary* — a cluster of Entities and Value Objects treated as a unit for enforcing invariants. Each Aggregate has a root Entity. External objects reference the Aggregate through the root.

The key question is not "what objects belong together?" but:

> Which objects must be transactionally consistent together to protect a true business invariant?

Rules:

- **Model true invariants inside the boundary.** What must be true immediately after the transaction? What can be corrected eventually? If a rule can become true eventually, it probably does not need one Aggregate boundary.
- **Design small Aggregates.** Large Aggregates load too much, conflict more, scale poorly, and couple unrelated rules. Design as small as the invariants allow.
- **Reference other Aggregates by identity**, not by object reference. Reduces coupling; supports distribution; clarifies transactional boundaries.
- **Use eventual consistency outside the boundary.** When a business process spans multiple Aggregates, coordinate via Domain Events, process managers, sagas, or Application Services.
- **No public setters for important state.** Use intention-revealing command methods.
- **No repository or messaging injection into Aggregates.** That usually means the Aggregate is reaching outside its boundary.
- **Optimistic concurrency** via version field or stream revision.

Common reasons to violate these rules — UI convenience, query performance, fear of eventual consistency, habit from relational modeling — explain the pressure but rarely justify large Aggregates. Use read models, projections, repositories, and process managers instead.

### 5.2 Entity

An Entity has identity that matters across time and state changes.

Use when: object has a lifecycle; can change attributes while remaining the same thing; the business distinguishes one instance from another; identity matters more than the current values.

Avoid making everything an Entity. Many concepts are better as Value Objects.

Identity questions: who assigns it; when it is needed; can it change; is it meaningful or only technical; can equality rely on identity alone; what happens if upstream identity changes.

Behaviors should be intention-revealing:

```text
user.activate()
user.changeEmailAddress(...)
subscription.cancelBecause(...)
backlogItem.commitTo(sprint)
```

over:

```text
user.setActive(true)
user.setEmail(...)
subscription.setStatus("cancelled")
backlogItem.setSprintId(...)
```

Validation belongs where the rule belongs: Entity constructor or factory for local invariants; Aggregate root for aggregate-wide invariants; Domain Service for an operation not owned by one object; Application Service for use-case coordination and authorization. Do not rely only on database constraints for domain validity.

### 5.3 Value Object

Describes, measures, or quantifies something. No conceptual identity. Equal by value. Replaceable. Immutable or treated as immutable.

Use when: the concept is descriptive; its attributes form a conceptual whole; equality by value makes sense; immutability is practical; replacement is safer than mutation.

Examples: Money, EmailAddress, DateRange, Quantity, Address, FullName, Percentage, Coordinates, TenantId-as-typed-value.

Replace primitive obsession when the value has rules:

```text
EmailAddress     over    String email
Money            over    BigDecimal amount
TenantId         over    String tenantId
SprintDuration   over    int days
```

Persistence should not force a Value Object to become an Entity. Embed, serialize, or map; choose the implementation, not the conceptual decision.

### 5.4 Domain Service

A stateless domain operation that does not naturally belong to an Entity or Value Object — usually because it coordinates multiple objects or expresses a domain concept that does not fit on one object.

Not: an Application Service, REST controller, persistence gateway, transaction script, helper class, technical utility, or general dumping ground for business rules.

Name in the Ubiquitous Language. Keep the layer small — if most behavior lives in services, the model is anemic.

### 5.5 Domain Event

Records something meaningful that happened in the domain. Past tense.

Examples: `BacklogItemCommitted`, `SprintScheduled`, `UserRegistered`, `PaymentReceived`, `PolicyActivated`, `OrderShipped`.

Use to: make important happenings explicit; decouple producers from consumers; support eventual consistency; notify other contexts; drive long-running processes; create audit history; support event sourcing; update read models.

Avoid: events for trivial technical steps with no domain meaning ("event noise").

Publishing pattern: the Aggregate records events during command execution; the Application Service publishes after commit. Do not let domain state change succeed while the event silently disappears — use a transactional outbox, event store, notification log, or transaction-synchronized dispatch. Do not publish remote messages directly from inside Aggregates.

For events crossing context boundaries: treat them as integration contracts. Schema versioning, idempotent consumers, ordering, replay, late arrival, missing events, correlation and causation IDs, privacy.

### 5.6 Factory

Creates complex domain objects or Aggregates while protecting invariants and hiding construction complexity. May be a static or named method on the Aggregate root, a standalone Factory object, a Domain Service acting as a Factory, or part of an Anticorruption Layer.

Name in the Ubiquitous Language: `registerTenant(...)`, `scheduleSprint(...)`, `createDiscussionFor(...)` over `newEntity(...)`, `buildObject(...)`, `mapDto(...)`.

### 5.7 Repository

Collection-like access to Aggregates. Abstracts persistence while preserving the illusion of working with domain objects.

One Repository per Aggregate root, not per table or per object.

A Repository is not a DAO. It does not let callers assemble inconsistent object graphs. For complex read views, prefer read models or query services rather than loading large Aggregates for display.

Application Services usually manage transactions; Repositories participate but should not hide broad transaction scripts.

### 5.8 Module

Groups cohesive concepts. Names should use the Ubiquitous Language: `identity.access`, `agilepm.backlog`, `billing.invoice`. Avoid vague containers (`utils`, `helpers`, `managers`, `models`, `services`, `common`) unless they truly express the local model.

If a module cannot be named coherently because language is mixed inside, that may reveal a missing Bounded Context boundary.

## 6. Integration between Bounded Contexts

Cross-context integration is not local object collaboration. Latency, partial failure, versioning, autonomy, deployment mismatch, and organizational boundaries all apply. Do not treat remote calls as normal method calls.

Do not share domain objects across contexts casually. Sharing internal classes creates Shared Kernel or Conformist coupling accidentally. Consumers begin to use foreign concepts as if they were local.

Prefer: Published Language, Open Host Service, DTOs as integration contracts, Anticorruption Layer, local Value Objects created from foreign data, explicit translators.

For REST integration:

- design resources around consumer use cases, not internal Aggregate shape;
- document media types or schemas;
- treat the API as Open Host Service and the schema as Published Language where appropriate;
- translate responses into local model concepts at the boundary;
- a CRUD-per-entity REST surface is usually a Conformist trap for downstream consumers.

For messaging integration: design for durable publication, idempotent consumers, duplicate messages, out-of-order arrival, missing messages, replay, broker outage and catch-up, poison messages, schema versioning.

For local copies of remote data: state who owns the original, why a local copy is needed, how stale it may be, how it is updated, what happens when updates are missed, whether it is a snapshot Value Object or a synchronized local Entity.

For long-running processes (sagas / process managers): hold process state, react to events, send commands, handle timeouts, record progress, tolerate retries. Do not pretend distributed work is one local transaction; do not hide a long-running process inside a single oversized Aggregate.

## 7. Architecture posture

Architecture must protect the domain model rather than replace it. Layered, hexagonal, REST, messaging, CQRS, event sourcing, data grids — these are support structures, not the model.

The recurring principle: **protect the domain model from accidental technical concerns.**

Use a style because it reduces real risk or satisfies real quality demands — maintainability, scalability, reliability, latency, autonomy, testability, deployment independence, integration needs, auditability, regulatory traceability, consistency requirements. Do not adopt for fashion.

The domain layer should remain expressible in the local Ubiquitous Language, insulated from persistence, transport, UI, and foreign models where practical. Application Services coordinate use cases — they should not contain core business rules that belong in the model. Infrastructure (persistence, messaging, REST clients, scheduling, transactions, framework glue) lives outside the domain through dependency inversion, ports, adapters, and repositories.

## 8. Smells and corrections

| Smell | Correction |
|---|---|
| **DDD-Lite** — Entities, Services, Repositories without Bounded Context, Ubiquitous Language, or Context Map | Return to strategic design before more tactical work |
| **Anemic Domain Model** — domain objects are data holders; logic lives in services or controllers | Move behavior to Entities, Value Objects, Aggregates, Domain Services |
| **Entity obsession** — every concept has identity and mutable lifecycle | Favor Value Objects for descriptive concepts |
| **Aggregate bloat** — one Aggregate carries a large object graph for UI or query convenience | Split by true invariants; use identity references, read models, eventual consistency |
| **Repository as DAO** — repositories expose tables, generic queries, partial assembly | Design around Aggregate roots and collection semantics |
| **Shared model leakage** — one context imports another's domain classes | Use Published Language, DTOs, Open Host Service, Anticorruption Layer |
| **False enterprise vocabulary** — one term forced to mean one thing across the org | Preserve local meanings inside contexts; translate at boundaries |
| **Technical module names** — `utils`, `helpers`, `managers`, `services`, `models` | Name modules by cohesive domain concepts |
| **Event noise** — events describe technical steps, not domain occurrences | Rename or remove events business experts would not care about |
| **Infrastructure in domain** — domain depends on HTTP, ORM, brokers, framework annotations | Use ports, adapters, repositories, dependency inversion |
| **Context Map denial** — teams assume integration works but never name the relationship | Draw the Context Map; document upstream/downstream responsibilities |

## 9. Refactoring drift toward the model

For existing systems, look for language drift first:

- generic method names (`save`, `update`, `process`, `handle`);
- status strings duplicated across layers;
- DTO names used as domain names;
- `Manager`, `Helper`, `Util`, `Service` classes with unclear domain meaning;
- entities with only getters and setters;
- business rules in controllers or application services;
- external API models used directly inside domain code;
- one model trying to serve multiple teams with different vocabularies.

Move toward intention. Replace generic operations with domain actions:

```text
saveCustomer(...)
```

may split into:

```text
registerCustomer(...)
changeCustomerAddress(...)
changeCustomerPrimaryEmail(...)
deactivateCustomer(...)
```

— but only when the business actions are actually distinct.

Recover Aggregates from invariants, not from object graphs. For each command: which Aggregate receives it, what invariant must hold immediately, which state is required to decide, which events may be emitted, what can happen eventually.

Move behavior inward. If business rules live in controllers or application services, move them toward the Entity, Value Object, Aggregate, or Domain Service that owns the rule.

Isolate foreign models. When external DTOs leak into domain code, introduce an Anticorruption Layer:

```text
Foreign API DTO → Adapter → Translator → Local Value Object / Entity / command
```

Avoid big-bang rewrites. Use the UEC's Mikado-style sequencing: small step, validate, repeat. Stop when the next step requires a new domain conversation, a new boundary decision, or broader blast-radius proof.

## 10. Cards (use as checklists, not forms)

Proportional to risk. Do not fill in every line for every change.

### 10.1 Triage card

```text
Domain triage:
- Change:
- Business capability:
- Domain experts:
- Core / Supporting / Generic / non-domain:
- Why this investment level:
- Consequence of wrong model:
- Lens depth: full / light / not needed
```

### 10.2 Bounded Context card

```text
Bounded Context:
- Name:
- Purpose:
- Model owner:
- Ubiquitous Language summary:
- Core concepts:
- Commands:
- Events:
- Aggregates:
- Repositories:
- External dependencies:
- Published Language / Open Host Service:
- Anticorruption Layers:
- Known model gaps:
```

### 10.3 Aggregate design card

```text
Aggregate design:
- Bounded Context:
- Aggregate:
- Root:
- True invariant being protected:
- State needed to enforce the invariant:
- Other Aggregates referenced by identity:
- Domain Events emitted:
- Eventual consistency outside the boundary:
- Concurrency rule:
- Tests:
```

### 10.4 Context Map edge card

```text
Context Map edge:
- Upstream:
- Downstream:
- Pattern:
- Business reason for integration:
- Exchange type:
- Published Language:
- Translation location:
- Release coordination:
- Failure modes:
- Versioning:
- Tests/contracts:
```

## 11. Output for domain-touching work

Use the UEC §9 output template plus the proportional domain-design block (also defined in UEC §9). The "Known model gaps" line is a first-class output, not an optional caveat — silence on it claims a model the agent does not have.
