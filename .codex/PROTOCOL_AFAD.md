# PROTOCOL_AFAD.md — Agent-First Documentation Protocol

Protocol: `AGENT_FIRST_DOCUMENTATION`  
Version: `4.0`

This protocol governs documentation that agents must maintain, retrieve, validate, or keep synchronized with code and system behavior. It is optimized for documentation that can be used by humans, retrieval systems, and future coding agents without requiring hidden context.

It inherits the Universal Engineering Contract. Documentation work must still identify truth, evidence, consequence, invariants, and preservation.

---

## 0. Agent routing and scope

Do not maintain a long section index at the top of this file. Agents need a compact routing gateway more than a table of contents. Use this section to decide what kind of document you are touching and which rules apply.

| Situation | Apply |
|---|---|
| Public API, exported symbol, schema, route, event, config key, error, or test fixture must be documented | AFAD reference atom rules |
| Existing reference docs drift from code | AFAD sync loop |
| Guide, runbook, ADR, tutorial, or nested component README needs improvement | AFAD auxiliary-doc rules, adjusted to the document's purpose |
| Code change alters public behavior, compatibility, architecture, operation, or tooling | Update docs in the same change when the existing docs cover that surface |
| Repository root `README.md` is the only touched document | Do not apply AFAD; use the root README storefront rule in `AGENTS.md` |
| `CHANGELOG.md`, `LICENSE`, `NOTICE`, `SECURITY.md`, `CONTRIBUTING.md`, release notes, or legal/governance files | Follow their own conventions unless the repository opts them into AFAD |

### Root README boundary

The root `README.md` exemption is defined in `AGENTS.md` because it is a repository-wide routing rule. This protocol repeats the boundary only to prevent accidental over-application: do not force AFAD frontmatter, atom schemas, exhaustive API signatures, or routing metadata into the repository root `README.md`.

Nested README files are not automatically exempt. Classify them by function:

- user-facing landing page for a package/example/integration: keep human-first and light;
- component guide or operational documentation: use the auxiliary-doc rules;
- API/reference material disguised as a README: convert or link to AFAD reference docs.

---

## 1. Documentation theory

AFAD documentation is not prose storage. It is a durable theory surface for the system.

Before changing non-trivial documentation, answer the documentation form of the Universal Engineering Contract:

```text
Truth:
- What is the canonical source for this fact: code, schema, config, generated artifact, ADR, release policy, runtime behavior, or this document?
- Is this document allowed to define the fact, or must it derive/link from another owner?

Evidence:
- What proves the documentation is accurate: tests, signatures, schemas, examples, generated docs, CI, runtime traces, release artifacts, or manual repro?

Consequence:
- What breaks if this documented concept is removed, renamed, or changed?
- Which users, agents, tools, generated artifacts, docs, examples, or workflows depend on it?

Invariant:
- What must remain true about the documented behavior, API, procedure, or constraint?

Preservation:
- Where should the knowledge live after this change: code, type, test, generated file, reference atom, guide, runbook, ADR, comment, or README link?
```

A documentation change is incomplete when it makes text nicer but leaves truth ownership, verification, or drift risk unclear.

---

## 2. Documentation classes

AFAD distinguishes reference documentation from narrative documentation. Do not use one shape for all documents.

### 2.1 Reference documents

Reference documents are retrieval-oriented and schema-driven. They describe stable contract surfaces such as exported symbols, data types, config keys, routes, events, errors, generated schemas, test fixtures, and operational interfaces.

Recommended naming:

```text
docs/DOC_00_Index.md
docs/DOC_01_Core.md
docs/DOC_02_Types.md
docs/DOC_03_<Domain>.md
docs/DOC_04_<Domain>.md
docs/DOC_05_Errors.md
docs/DOC_06_Testing.md
```

`DOC_*.md` files are strict. They use frontmatter, atom schemas, short self-contained entries, and sync validation.

### 2.2 Auxiliary documents

Auxiliary documents are human-guided but still agent-maintainable. They may be narrative when narrative improves understanding.

Examples:

```text
docs/GUIDE_<Topic>.md
docs/RUNBOOK_<Operation>.md
docs/ADR_<Number>_<Decision>.md
docs/TUTORIAL_<Task>.md
examples/<name>/README.md
packages/<name>/README.md
```

Auxiliary docs should not duplicate full reference atoms. They may link to reference docs, show runnable examples, explain workflows, record decisions, and preserve operational theory.

### 2.3 Special documents

The following are not AFAD reference docs by default:

- root `README.md`;
- `CHANGELOG.md`;
- `LICENSE`, `NOTICE`, and legal files;
- `SECURITY.md`;
- `CONTRIBUTING.md`;
- release notes;
- governance documents.

They must still be accurate and coherent, but their native conventions outrank AFAD structure unless project-specific instructions say otherwise.

---

## 3. Core invariants

The following invariants apply to AFAD-managed documents.

### INV-1 Scope completeness

Every documented public contract surface has exactly one canonical AFAD home.

For published libraries and public APIs, the documented surface usually includes every externally visible public export. For applications, CLIs, services, and internal repositories, the documented surface is the contract users or operators rely on: commands, config, routes, schemas, events, error codes, public modules, runbooks, and externally meaningful behavior.

Do not blindly document every language-level `public` or exported symbol when it is not part of the intended contract. Do not omit a public contract merely because the language visibility is narrow.

### INV-2 Accuracy

Documentation must match the canonical source of truth.

- Signatures match code.
- Config keys, routes, event names, status values, limits, labels, and error codes match their canonical owner.
- Examples run as shown or are explicitly marked as conceptual.
- Operational procedures match current tooling and deployment shape.

### INV-3 Canonical ownership

Shared contract facts have one owner. Documentation may expose a contract fact, but it must not become an unmaintained parallel definition.

If the owner is code, schema, generated artifact, config, or ADR, docs should derive from it, link to it, quote it minimally, or state the owner. If documentation is the owner, state that clearly and ensure code/tools derive from it or are validated against it.

### INV-4 Atomicity

Reference atoms are self-contained and retrieval-sized.

- One concept per entry.
- First sentence states what the thing is.
- No `see above` or `see below` as required context.
- 200-400 tokens is the target for substantial entries.
- 600 tokens is the normal maximum for a reference atom.
- Split oversized entries unless splitting would damage correctness.

The 600-token rule is about semantic precision and retrieval quality, not model context length.

### INV-5 Current state

Reference docs describe the current API and current behavior.

Historical provenance such as `Added in vX.Y` belongs in `CHANGELOG.md`, release notes, or migration docs. Deprecation notices are allowed because they affect current user decisions.

Preferred deprecation form:

```text
Deprecated: vX.Y. Use <replacement>. Removal: vZ.0.
```

### INV-6 Renderability

Markdown must render cleanly on the repository's normal platform.

- Use language tags on code fences.
- Avoid decorative emoji in AFAD-managed docs.
- Avoid pseudo-code unless explicitly labeled conceptual.
- Avoid frontmatter in files where it degrades the user-facing rendering, especially root `README.md`.

---

## 4. Metadata

### 4.1 Reference frontmatter

Every AFAD reference document should start with frontmatter.

```yaml
---
afad: "4.0"
domain: CORE
updated: "YYYY-MM-DD"
scope:
  paths: ["src/path/or/package"]
  symbols: ["OptionalSymbolOrNamespace"]
route:
  keywords: [distinctive, terms]
  questions: ["natural language query this file uniquely answers"]
---
```

Field semantics:

| Field | Meaning |
|---|---|
| `afad` | Protocol version used by this document. |
| `domain` | Semantic cluster such as `CORE`, `TYPES`, `ERRORS`, `TESTING`, `CONFIG`, `OPERATIONS`, or a project domain. |
| `updated` | Last meaningful documentation update, ISO date. |
| `scope.paths` | Source paths, packages, modules, schemas, or generated artifacts covered by the file. |
| `scope.symbols` | Optional major symbols/namespaces covered by the file. Use when helpful, not as a complete export list. |
| `route.keywords` | Distinctive retrieval terms. Avoid generic terms. |
| `route.questions` | Natural-language questions this file should answer. |

Do not require `project.version` in every doc. If a repository has a versioned public API and the doc is version-specific, add a project-specific field such as `project_version`, but do not create a second drifting source of release truth.

### 4.2 Auxiliary metadata

Auxiliary docs may use the same frontmatter when it renders cleanly and helps routing. If frontmatter would harm presentation, use a short HTML comment instead:

```html
<!--
AFAD:
  domain: OPERATIONS
  updated: YYYY-MM-DD
  route:
    keywords: [deploy, rollback, healthcheck]
    questions: ["how do I roll back the service?"]
-->
```

Never add AFAD metadata to the root `README.md` unless the repository already has a deliberate convention for hidden metadata there.

### 4.3 Route guidance

Route metadata is for disambiguation. It is not a substitute for clear content.

- Use 5-10 distinctive keywords when possible.
- Avoid generic keywords such as `function`, `class`, `method`, `handler`, `service`, `docs`.
- Use 2-5 questions only when they uniquely route to the file.
- Do not duplicate the same route questions across files.
- If you cannot find distinctive route terms, the file may be too broad or too vague.

---

## 5. File architecture

### 5.1 Index file

`DOC_00_Index.md` is the routing table for reference docs. Agents should consult it before guessing where an atom belongs.

Minimum structure:

~~~markdown
| Contract surface | Kind | Canonical doc | Source owner |
|:--|:--|:--|:--|
| `Registry.resolve` | callable | `DOC_01_Core.md#registryresolve` | `src/registry.*` |
| `MAX_RETRIES` | constant | `DOC_04_Config.md#max_retries` | `src/config.*` |
~~~

Do not turn the index into a narrative guide. It is a route map.

### 5.2 Domain files

Use one coherent domain per reference file. Place high-frequency and high-risk atoms early, and rare edge cases later.

Restructuring heuristics:

| Action | Trigger | Reason |
|---|---|---|
| Create a domain file | More than 20 related contract surfaces | Improves routing and chunk precision |
| Merge files | Fewer than 8 sparse entries with no distinct domain | Avoids fragmented retrieval |
| Split files | More than 60 entries or repeated retrieval confusion | Reduces context and routing noise |

Adjust thresholds to token density. Dense atoms require smaller files. Sparse atoms can tolerate larger files.

### 5.3 Language adaptation

AFAD is language-agnostic. Use the repository language in signatures and examples.

```text
Java:        public Result resolve(Key key) throws MissingKeyException
Rust:        pub fn resolve(&self, key: &Key) -> Result<Item, ResolveError>
Python:      def resolve(self, key: Key) -> Item | None:
TypeScript:  resolve(key: Key): Item | undefined
Go:          func (r *Registry) Resolve(key Key) (*Item, error)
```

Rules:

- Code fence language tags must match the language or artifact: `java`, `rust`, `python`, `typescript`, `go`, `bash`, `yaml`, `json`, `toml`, `sql`, etc.
- Do not force Python terminology onto non-Python ecosystems.
- Translate `exception`, `error`, `fixture`, `property`, `type alias`, and `enum` into the language's actual constructs.
- Prefer the repository's domain vocabulary over generic schema names.

---

## 6. Reference atom rules

All reference atoms share the following shape unless a specific schema says otherwise.

~~~markdown
## `ContractName`

One sentence stating what this thing is.

### Signature
```language
exact signature, declaration, schema fragment, route, config key, or event shape
```

### Constraints
- Return/Output: What is produced, including empty, null, sentinel, or error cases.
- State: Pure, read-only, mutates X, persists Y, emits Z, or derived from owner.
- Failure: Error, exception, result variant, status code, or never-fails rule.
- Thread/Async/Concurrency: Safety, blocking, cancellation, ordering, or not applicable.
- Compatibility: Public contract, internal, experimental, deprecated, or migration note.

---
~~~

General rules:

- Heading uses backticks for named symbols and contract facts.
- First sentence says what the thing is, not a vague action phrase.
- Signature or definition is required for symbol, schema, route, config, and event atoms.
- Constraints are semantic; they preserve the invariant users and agents need.
- Optional sections may be added when they aid decisions: `Parameters`, `Members`, `Fields`, `Usage`, `Example`, `Recovery`, `Operations`, `Deprecation`.
- Examples in reference atoms must be minimal: usually 5 lines or fewer.

---

## 7. Schema selection

Choose the narrowest schema that fits the documented contract.

| Contract kind | Schema |
|---|---|
| Function, method, constructor, command handler | Callable |
| Record, class, struct, interface, trait, data object | Type / data object |
| Enum, sealed hierarchy, algebraic data type, status set | Enum / variant set |
| Type alias, newtype, semantic wrapper | Alias / semantic type |
| Constant, config key, feature flag, limit | Constant / configuration |
| Route, endpoint, event, message, generated schema | Protocol surface |
| Error, exception, result variant, status code | Failure surface |
| Fixture, marker, test extension, shared test utility | Test infrastructure |
| Guide, runbook, ADR, tutorial | Auxiliary document |

When a thing fits multiple schemas, use the schema that represents the user's decision point. For example, an HTTP endpoint is a protocol surface even if implemented by a method.

---

## 8. Reference schemas

### 8.1 Callable

~~~markdown
## `Registry.resolve`

Method that resolves a registered item by key.

### Signature
```language
<exact function, method, or constructor signature>
```

### Parameters
| Name | Req | Semantics |
|:--|:--:|:--|
| `key` | Y | Registration key; non-empty |
| `strict` | N | Fail on missing key |

### Constraints
- Return/Output: Registered item, optional value, result, response, or status.
- Failure: Exact failure mode and trigger; state `Never fails` only when true.
- State: Pure, read-only, mutates, persists, emits, or invalidates.
- Concurrency: Safe, unsafe, synchronized, async, blocking, cancellation-aware, or not applicable.
- Compatibility: Public, internal, deprecated, experimental, or migration-sensitive.

### Usage
- Prefer when: Decision condition.
- Avoid when: Anti-pattern and reason.

### Example
```language
minimal runnable example
```

---
~~~

Parameter table rules:

- `Name` is the exact parameter name in backticks.
- `Req` is `Y` or `N` only.
- `Semantics` is a short phrase, ideally 10 words or fewer.
- Do not include a Type column. Types live in the signature.

Omit `Usage` and `Example` when they do not add decision value.

### 8.2 Type / data object

Use for records, structs, classes, interfaces, traits, DTOs, messages, and semantic wrappers.

~~~markdown
## `UserRecord`

Record representing an authenticated user visible to the API.

### Signature
```language
<type declaration>
```

### Fields / Members
| Name | Req | Semantics |
|:--|:--:|:--|
| `id` | Y | Stable user identifier |
| `email` | Y | Normalized contact address |

### Constraints
- Invariant: Rule that every instance must satisfy.
- Ownership: Who creates, mutates, serializes, or persists it.
- Compatibility: Wire format, database shape, public API, or internal-only.

---
~~~

For behavior-rich classes, document the class concept separately from major public methods. Do not pack every method into one atom.

### 8.3 Enum / variant set

~~~markdown
## `OrderStatus`

Enumeration of externally visible order lifecycle states.

### Signature
```language
<enum, sealed hierarchy, or variant declaration>
```

### Members
| Member | Value | Semantics |
|:--|:--|:--|
| `PENDING` | `"pending"` | Accepted, not fulfilled |
| `CANCELLED` | `"cancelled"` | Terminated before fulfillment |

### Constraints
- Invariant: Allowed transitions, if applicable.
- Compatibility: Serialized values are stable public contract.

---
~~~

Use this schema for Rust enums, Java enums/sealed types, TypeScript union literals, Go tagged values, and similar closed vocabularies.

### 8.4 Alias / semantic type

~~~markdown
## `UserId`

Semantic type representing a stable user identifier.

### Definition
```language
<alias, newtype, wrapper, or typedef declaration>
```

### Constraints
- Purpose: Prevents confusion with other identifiers.
- Validation: Format, normalization, or accepted range.
- Compatibility: Serialization or database representation.

---
~~~

Group related aliases only when they form one semantic family and remain under the token target.

### 8.5 Constant / configuration

~~~markdown
## `MAX_RETRIES`

Constant defining the maximum retry attempts for transient delivery failures.

### Definition
```language
<constant, config key, feature flag, or limit declaration>
```

### Constraints
- Owner: Canonical source that defines the value.
- Effect: Behavior controlled by the value.
- Range: Valid values or units, if applicable.
- Compatibility: User-visible, operational, internal, or generated.

---
~~~

Use this schema for constants, limits, feature flags, environment variables, config file keys, and externally meaningful labels.

### 8.6 Protocol surface

Use for routes, endpoints, events, messages, queue payloads, generated schemas, CLI commands, and wire contracts.

~~~markdown
## `POST /v1/orders`

Endpoint that creates an order from a validated checkout request.

### Shape
```language
<route, command, event name, schema fragment, or payload shape>
```

### Constraints
- Input: Required fields, validation, authentication, or permissions.
- Output: Response, emitted event, persisted state, or side effect.
- Failure: Status codes, errors, retries, idempotency, or dead-letter behavior.
- Compatibility: Versioning, migration, backward compatibility, or deprecation.

---
~~~

The protocol surface is often the real public contract even when implementation symbols are internal.

### 8.7 Failure surface

Use for errors, exceptions, result variants, status codes, and recoverable operational failures.

~~~markdown
## `ResolveError.MissingKey`

Failure raised or returned when a registry key has no registered item.

### Signature
```language
<error declaration, status code, result variant, or exception class>
```

### Constraints
- Trigger: Exact condition that produces this failure.
- Recovery: Caller or operator action.
- State: Whether anything was mutated before failure.
- Compatibility: Public error contract, internal diagnostic, or deprecated.

---
~~~

If there is an error hierarchy, include one compact hierarchy atom near the start of the errors file.

### 8.8 Test infrastructure

Use for shared fixtures, markers, tags, extensions, hooks, test containers, test utilities, and golden data conventions.

~~~markdown
## `databaseFixture`

Test fixture providing an isolated database for integration tests.

### Signature
```language
<fixture, extension, hook, helper, or marker declaration>
```

### Constraints
- Scope: Per-test, per-class, per-suite, per-session, or repository-wide.
- Provides: Resource, state, or behavior made available.
- Cleanup: Teardown, rollback, deletion, or none.
- Concurrency: Parallel-safe or serial-only.

---
~~~

Terminology adapts to the framework: pytest fixtures, JUnit extensions/tags, Rust test helpers, Go test helpers, Jest setup hooks, property-test strategies, fuzz harnesses.

---

## 9. Auxiliary document rules

Auxiliary docs may be narrative, but they must not become stale parallel reference manuals.

### 9.1 Guide

Use for task-oriented explanation.

~~~markdown
# <Topic> Guide

Purpose: What the reader will be able to do.
Prerequisites: What must already exist or be understood.

## Overview

Short context that explains the decision or workflow.

## Procedure

Concrete steps with runnable commands or examples.

## Verification

How the reader knows it worked.

## Troubleshooting

Common failures and recovery.

## Related reference

Links to AFAD reference atoms or canonical source owners.
~~~

### 9.2 Runbook

Use for operational procedures.

~~~markdown
# <Operation> Runbook

Purpose: Operational outcome.
When to use: Trigger condition.
When not to use: Unsafe or irrelevant cases.

## Preconditions

Access, environment, health checks, and safety checks.

## Procedure

Ordered steps with commands.

## Verification

Metrics, logs, traces, alerts, or user-visible checks.

## Rollback

How to restore the previous safe state.

## Escalation

Who or what to consult next.
~~~

### 9.3 ADR

Use for architectural decisions that preserve theory.

~~~markdown
# ADR <number>: <Decision>

Status: Proposed | Accepted | Superseded | Rejected
Date: YYYY-MM-DD

## Context

Forces, constraints, and problem shape.

## Decision

Chosen direction.

## Consequences

Benefits, costs, risks, and follow-up work.

## Alternatives considered

Rejected options and why.
~~~

### 9.4 Nested README

Nested READMEs should serve their directory's user.

- Package/example landing page: short, human-first, no AFAD atom structure.
- Component guide: guide structure is appropriate.
- Reference material: move or link to `DOC_*.md` instead of embedding full API docs.

---

## 10. Examples and snippets

Examples are contract surfaces when users copy them.

Rules:

- Every code fence has a language tag.
- Prefer runnable examples over illustrative fragments.
- Keep reference atom examples short.
- Put larger examples in `examples/`, integration tests, doctests, or guide docs.
- Do not use placeholder ellipses in commands or code unless the text explicitly says the example is partial.
- Update examples when APIs, config, routes, flags, package names, or build commands change.

If an example cannot be made runnable, label it clearly:

~~~markdown
Conceptual sketch, not directly runnable:
~~~

---

## 11. Sync loop

Run this loop when code changes may affect AFAD-managed docs, or when docs are suspected stale.

```text
1. Inventory
   Build the relevant contract map from code, schemas, generated artifacts, configs, routes, events, tests, and docs.

2. Compare
   Classify each contract surface:
   - MATCH: doc and source agree.
   - DRIFT: signature, shape, name, value, or behavior differs.
   - ORPHAN-CODE: contract exists without required doc coverage.
   - ORPHAN-DOC: doc exists for removed or non-contract surface.
   - MOVE/RENAME: same concept moved or renamed.
   - SEMANTIC-DRIFT: signature matches but behavior or invariant changed.

3. Reconcile
   Update, create, move, merge, split, or delete atoms.

4. Validate
   Check metadata, signatures, links, examples, routes, and token-sized atoms.

5. Preserve
   Put newly discovered theory in the most durable place: test, type, schema, doc atom, guide, runbook, ADR, or root README link.
```

### Co-evolution rule

Docs and code should change together when a code change affects documented public behavior, public API, configuration, operational procedure, architecture boundary, generated schema, CLI contract, route, event, error, or user-visible example.

Do not update docs for purely internal implementation changes unless the implementation change alters the theory users, operators, or future agents need.

### Move detection

When a doc atom appears orphaned, check for moves and renames before deleting it.

```text
ORPHAN-DOC(A) + ORPHAN-CODE(B) with same concept, matching behavior, or compatible signature
→ classify as MOVE/RENAME
→ preserve useful constraints, examples, deprecation notes, and rationale
```

### Generated docs and hashes

If the repository has tooling that generates docs, signatures, source maps, or implementation hashes, use that tooling. Do not invent manual hashes. Do not manually update generated docs without also updating the generator or source owner.

---

## 12. Validation

AFAD validation is layered. Block on correctness before style.

| Level | Check | Blocking |
|---|---|:---:|
| L0 | File is in scope for AFAD | Yes |
| L0 | Metadata is valid when metadata is required | Yes |
| L1 | Reference atoms have required heading, first sentence, signature/shape, and constraints | Yes |
| L1 | Code fences have language tags | Yes |
| L2 | Signatures, shapes, routes, config keys, event names, errors, and examples match canonical owners | Yes |
| L2 | No required contract surface is undocumented | Yes |
| L2 | No AFAD atom documents a removed or non-contract surface without explanation | Yes |
| L2 | Links and backtick references resolve where practical | Yes |
| L2 | Reference atoms stay within retrieval-sized bounds or have justified split exceptions | Yes |
| L3 | Parameter fragments are concise | No |
| L3 | Route keywords are distinctive | No |
| L3 | Style is economical and non-repetitive | No |

Recovery:

| Failure | Recovery |
|---|---|
| Out-of-scope file treated as AFAD | Remove AFAD structure and apply the file's native convention |
| Invalid metadata | Fix or remove metadata according to file class |
| Signature/shape drift | Update doc from canonical owner and verify examples |
| Missing doc atom | Create minimal accurate atom, then refine |
| Orphan doc atom | Classify as move/rename or delete |
| Oversized atom | Split by concept, not by arbitrary length |
| Stale example | Update, move to test/example, or delete if no longer useful |

---

## 13. Anti-patterns

| Anti-pattern | Why it is wrong | Fix |
|---|---|---|
| Long quick index listing every section | Consumes tokens and goes stale | Use compact routing gateway and clear headings |
| Root `README.md` forced into AFAD | Damages storefront role | Keep root README human-first and link to docs |
| Types repeated in parameter tables | Creates second drift source | Keep types in signature only |
| Full API reference inside a guide | Duplicates reference docs | Link to `DOC_*.md` atoms |
| Generic route keywords | Poor retrieval disambiguation | Use distinctive domain terms |
| `see above` as required context | Breaks atom self-containment | Repeat the minimal needed fact |
| Decorative emoji in AFAD docs | Adds noise and rendering variance | Use plain text |
| Historical `Added vX.Y` in reference atom | Wrong current-state surface | Put history in changelog/release notes |
| Pseudo-code presented as runnable | Misleads users and agents | Make it runnable or label conceptual |
| Documentation as second source of contract truth | Creates drift | Derive from or identify canonical owner |
| Blindly documenting every language-public symbol | Bloats docs and hides real contracts | Document intended contract surfaces |
| Deleting orphan docs before checking moves | Loses preserved theory | Classify move/rename first |

---

## 14. Conflict resolution

Priority order:

```text
P0 Accuracy and safety
P1 Canonical ownership
P2 Completeness of intended contract surface
P3 Retrieval structure
P4 Human readability
P5 Style economy
```

Examples:

| Conflict | Resolution |
|---|---|
| Exact signature is long and ugly | Keep the accurate signature; structure around it |
| Atom exceeds token target but cannot be split without losing correctness | Keep accurate atom and note split exception |
| Guide wants narrative but repeats full API details | Keep narrative, link to reference atoms |
| Root README would benefit from one example but AFAD prefers reference structure | README storefront rule wins; include one concise runnable example |
| Undocumented contract surface has unclear semantics | Create minimal atom with explicit TODO/unknown constraint, then preserve follow-up |
| Style violation but content is accurate and needed | Keep content; fix style in a later pass if needed |

---

## 15. Agent output contract

For non-trivial documentation work, the work summary should state:

```text
Documentation scope:
- Files changed:
- File class: reference, guide, runbook, ADR, nested README, root README exception, or special file:

Truth/evidence:
- Canonical sources checked:
- Verification performed:

Changes:
- Atoms created/updated/deleted/moved:
- Examples updated or validated:
- Links/routes changed:

Remaining risk:
- Missing source owners, uncertain semantics, skipped validation, or follow-up needed:
```

Do not dump this template into trivial summaries. Use it to ensure the agent did not produce prettier but less trustworthy documentation.

---

## 16. Worked examples

### 16.1 Callable atom, Java

~~~markdown
## `Registry.resolve`

Method that resolves a registered item by key.

### Signature
```java
public Item resolve(String key, boolean strict) throws KeyNotFoundException
```

### Parameters
| Name | Req | Semantics |
|:--|:--:|:--|
| `key` | Y | Registration key; non-blank |
| `strict` | N | Throw on missing key |

### Constraints
- Return/Output: Registered `Item`, or `null` when `strict=false` and key is absent.
- Failure: Throws `KeyNotFoundException` when `strict=true` and key is absent.
- State: Read-only.
- Concurrency: Safe for concurrent reads.
- Compatibility: Public API.

---
~~~

### 16.2 Callable atom, Rust

~~~markdown
## `Registry::resolve`

Method that resolves a registered item by key.

### Signature
```rust
pub fn resolve(&self, key: &str) -> Result<Option<Item>, ResolveError>
```

### Parameters
| Name | Req | Semantics |
|:--|:--:|:--|
| `key` | Y | Registration key; non-empty |

### Constraints
- Return/Output: `Ok(Some(Item))` when registered; `Ok(None)` when absent and absence is allowed.
- Failure: Returns `Err(ResolveError)` for invalid keys or unavailable backing store.
- State: Read-only.
- Concurrency: Safe for shared access when the registry is shared immutably.
- Compatibility: Public crate API.

---
~~~

### 16.3 Protocol surface atom

~~~markdown
## `order.created`

Event emitted after an order is durably created.

### Shape
```json
{
  "type": "order.created",
  "order_id": "string",
  "created_at": "RFC3339 timestamp"
}
```

### Constraints
- Input: Emitted only after the order row is committed.
- Output: Downstream fulfillment and analytics consumers may process the event independently.
- Failure: Publishing failure must be retried or dead-lettered according to the event pipeline policy.
- Compatibility: `type` and `order_id` are stable wire-contract fields.

---
~~~

---

END OF PROTOCOL
