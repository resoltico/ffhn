# Java 26+ / Gradle Agent Protocol

**Version:** 2.0.0
**Updated:** 2026-04-27
**Inherits:** [.codex/UNIVERSAL_ENGINEERING_CONTRACT.md](./UNIVERSAL_ENGINEERING_CONTRACT.md) v2.0.0+
**Scope:** Java **26+** projects built with **Gradle** — applications, libraries, CLIs, services, frameworks, plugins, tools, and multi-module builds.

## 0. Scope and inheritance

This protocol inherits the Universal Engineering Contract. The universal contract defines the meta-questions every change must answer — Truth, Evidence, Consequence, Invariant, Justification, Re-cueing — and frames the agent as a *transient theory-holder*. Apply the universal contract before any rule below; do not restate it here.

This protocol adds Java- and Gradle-specific content for which the universal contract is intentionally silent: language-feature posture, build wiring, JVM concurrency primitives, serialization shapes, and verification patterns appropriate to Java 26+.

**Primary objective:** produce Java that is correct, explicit, maintainable, compatible with the repository's real baseline, and validated through the narrowest sufficient feedback path.

**Optimization order:** correctness → explicit contracts → concurrency correctness → narrow API → evolution safety → readability → terseness.

Terseness loses to clarity. Convenience loses to correctness. Cleverness loses to maintainability.

### 0.1 Java 26 + Gradle tacit gaps

Per the Naurian frame, some theory the agent typically does not bring in cold and must surface rather than paper over. Watch especially for:

- Whether Gradle wrapper, plugins, toolchain, IDE, and CI are all on a Java-26-capable line, and where that wiring actually lives.
- Whether the repository is an internal tool, a library, or a published artifact — this changes what "public" and "compatible" mean.
- Whether preview or incubator features are intentionally enabled for the slice being touched. Compile, test, runtime, IDE, packaging, and CI must all agree, or the feature silently fails in one phase.
- Whether the codebase has already migrated past historical `synchronized` / virtual-thread pinning workarounds. Java 26's locking advice differs from pre-24 advice; old folklore decays badly.
- Whether JPMS `opens` directives reflect intentional reflective seams or accumulated escape hatches.
- Whether serialized shapes are external contract or scratch internals.
- Whether deep reflection on `final` fields (now warned by default in Java 26) is in use anywhere along the change's path.

Where the answer is not derivable from code, history, or conversation, surface the gap explicitly; do not assume the convenient answer.

## 1. Repository intake

Before touching source, establish the repository baseline:

1. **Build:** Gradle wrapper version, wrapper checksum posture, Gradle DSL (`.gradle.kts` vs `.gradle`), version catalog, toolchain configuration, `--release`/source/target settings, preview-feature wiring.
2. **Shape:** application, library, plugin, framework, CLI, or multi-module build; JPMS usage; generated code; publication targets; runtime packaging.
3. **Tests and CI:** test framework, canonical verification tasks, coverage tools, static analysis, CI matrix, release gates.
4. **Compatibility posture:** internal tool, published library, plugin, framework, service API, wire protocol, serialized format, or migration-sensitive data model.
5. **System map:** apply the universal contract's six concerns (truth, evidence, consequence, invariant, justification, re-cueing) to the touched surface.

Do not assume the project wants the newest syntax, the broadest refactor, or a published-library compatibility posture. Derive the posture from the repository and task.

## 2. Change loop

Per the universal contract §2 (Red → Green → Refactor), start with the smallest failing proof of behavior: test, assertion, reproducible check, type-level constraint, contract test, or manual verification path.

Then:

1. make the minimal coherent implementation;
2. run the narrowest relevant verification task;
3. refactor immediately until the touched code is clearer and easier to change;
4. widen the change only when local repair would preserve or deepen a bad structure;
5. widen verification when contracts, module boundaries, public APIs, serialization, concurrency, or build logic change.

When a build or test fails, read the actual failure output. Fix the structural, type, logic, or configuration cause. Do not pile up cascading errors, cargo-cult compiler suggestions, or suppress warnings merely to pass.

## 3. Java 26 baseline posture

Java 26 is the target baseline for this protocol. Use Java 26 capabilities when the repository baseline permits them and when they make the result clearer, safer, or more maintainable.

### 3.1 Normal Java 26+ toolset

Use these as ordinary tools when they improve the result:

| Capability | Use when | Avoid when |
|---|---|---|
| **Records** | Immutable value carriers with named components and invariants | Identity-bearing, lifecycle-heavy, mutable, or behavior-rich objects |
| **Sealed types** | Closed result families, state machines, protocol variants, controlled error families | Future external extension is a real requirement |
| **Pattern matching `switch`** | Exhaustive dispatch over sealed families or enums | A single simple `if` is clearer |
| **Record patterns** | Multiple record components are consumed immediately in a pattern arm | Only one component is used, or a named variable is clearer |
| **Unnamed variables and patterns** | A binding is intentionally discarded | The ignored value carries useful meaning |
| **Sequenced Collections** | First/last/reversed operations on ordered collections | As a reason to prefer `LinkedList`; `ArrayList` remains the default list |
| **Stream Gatherers** | A named gatherer expresses reusable domain pipeline behavior | A plain loop or collector is clearer |
| **Text blocks** | Multiline SQL, JSON, XML, HTML, templates, and structured constants | Single-line strings or whitespace-sensitive values where escapes are clearer |
| **Virtual threads** | Highly concurrent blocking I/O workloads | CPU-bound parallelism or work that needs bounded compute pools |
| **Scoped values** | Immutable request/task context flowing down a bounded call tree | Recreating ambient global state under a new name |
| **Module import declarations** | Small tools or files with genuinely module-oriented imports | Blanket style that hides which types are actually used |
| **Compact source files / instance `main`** | Demos, learning material, scripts, one-file tools | Production code that benefits from explicit class structure |
| **Flexible constructor bodies** | Early validation, normalization, and computing delegation arguments | Hiding substantive business logic inside constructors |

### 3.2 Java 26 changes to account for

- **Final field mutation by deep reflection now warns by default.** Do not design new code, tests, serializers, or dependency-injection paths that mutate `final` fields by reflection. Prefer constructors, factories, builders with real invariants, or supported serialization mechanisms.
- **Applet API is removed.** Do not add or preserve applet dependencies. Remove applet-era compatibility paths when no real contract depends on them.
- **HTTP/3 is available through the JDK HTTP Client API.** Before adding a networking dependency solely for HTTP/3, check whether `java.net.http` satisfies the requirement. Keep fallback and compatibility behavior explicit.
- **AOT object caching and GC/runtime improvements are operational tools.** Use them only with deployment, measurement, and rollback discipline. Do not introduce JVM flags speculatively.
- **Synchronized virtual-thread pinning was largely eliminated in Java 24.** On Java 26+, do not replace `synchronized` with `ReentrantLock` solely to avoid historical virtual-thread pinning. Choose the locking primitive by semantics: `synchronized` where practical; `ReentrantLock`, `ReadWriteLock`, `StampedLock`, or `Condition` when their extra capabilities are needed. Still avoid blocking or slow I/O while holding any lock.

### 3.3 Preview and incubator features

Preview and incubator features are useful tools, not defaults. They require deliberate governance.

A preview or incubator feature is acceptable only when all of the following are true:

1. it materially improves design, control flow, concurrency semantics, API shape, security, or performance;
2. the repository already accepts the operational cost, or the task explicitly authorizes it;
3. compile, test, runtime, CI, IDE, and developer workflow implications are updated together;
4. the usage is contained to the smallest reasonable surface;
5. the repository can tolerate source, binary, or behavior changes in a later JDK.

When introducing one:

- enable it explicitly and consistently across all affected phases;
- keep the blast radius small;
- avoid leaking preview-dependent types through broad public APIs unless the project accepts that risk;
- record the justification (per universal contract §1.5) so the next reader can see why the cost was accepted;
- prefer wrappers or adapters if later redesign is likely.

Currently relevant Java 26 preview/incubator features:

| Feature | Posture |
|---|---|
| **Structured Concurrency** | Prefer for related subtasks that need shared cancellation, ownership, and failure semantics, when preview is enabled. Otherwise preserve the same ownership discipline with the repository's approved concurrency model. |
| **Primitive types in patterns, `instanceof`, and `switch`** | Use only when it materially improves clarity or correctness. Do not enable preview syntax to look modern. |
| **Lazy Constants** | Specialized deferred-immutability tool for expensive values that should be initialized at most once and then treated as constant. Do not use as a general cache. |
| **PEM encodings of cryptographic objects** | Use when the task is actually about encoding or decoding cryptographic objects. Do not invent a custom parser or add a library first. |
| **Vector API** | Use only for measured or clearly motivated performance work. Do not introduce speculatively. |

## 4. Domain modeling

Choose the narrowest construct that represents domain truth.

### 4.1 Preferred constructs

- **Record:** immutable value carrier. Default when the type's main job is to carry named values with invariants.
- **Sealed interface/class:** closed alternatives callers must distinguish behaviorally: outcomes, state machines, protocol messages, controlled error descriptors.
- **Enum:** closed symbolic set with stable vocabulary.
- **Small semantic record/enum:** primitive values that are easy to confuse: `UserId`, `Port`, `CurrencyCode`, `Retries`.
- **Ordinary class:** identity-bearing, lifecycle-heavy, mutable, or behavior-rich object.

Do not use `String`, boolean flags, integer codes, or `null` where the caller must distinguish domain alternatives behaviorally.

### 4.2 Records

A compact constructor is the normalization and invariant boundary.

Rules:

- Null-check required fields at the trust boundary.
- Reject blank semantic strings with precise messages.
- Defensively copy collection components with `List.copyOf`, `Set.copyOf`, or `Map.copyOf`.
- Store immutable views only.
- Normalize once inside the compact constructor, not at every call site.
- Throw `IllegalArgumentException` for business invariant violations.

Every record with a collection component must have an explicit compact constructor that performs the defensive copy unless the component type is already an immutable project-owned type. A record with a `List`, `Set`, or `Map` component and no defensive-copy boundary is an invariant leak.

### 4.3 Sealed hierarchies

Keep sealed families coherent. Avoid catch-all variants such as `Unknown`, `Other`, or `GenericFailure` unless the boundary genuinely permits unknown values.

For public sealed families, every subtype is part of the API. Adding a subtype is a compatibility event because exhaustive switches in consumers may need to change.

### 4.4 Exception families

Exception families that share accessor fields should use a sealed interface to declare those accessors. Each concrete subtype should extend the appropriate JDK exception class and implement the interface, carrying its own fields directly.

```java
public sealed interface ParseProblem
    permits InvalidTokenException, UnexpectedEofException {
  String source();
  int position();
}

public final class InvalidTokenException extends IllegalArgumentException
    implements ParseProblem {
  private final String source;
  private final int position;
  // ...
}
```

Do not use an abstract sealed exception class merely to share fields. That couples subtypes to a shared mutable state carrier rather than a pure interface contract.

### 4.5 Construction

Direct construction is preferred when the constructor is the clearest contract.

Builders are justified for many independent optional fields, staged construction with real invariants, generated external APIs that conventionally use builders, or readability at complex call sites. Do not introduce builders by reflex.

### 4.6 Shadow types and wire boundaries

Fields representing a finite set of values must use the canonical enum or semantic type, not a `String` or a locally defined shadow type that duplicates a type already defined elsewhere.

Wire serialization is a permitted translation boundary. Convert to canonical wire names at the boundary, not by storing wire strings in internal records.

Never call `.name()` on a third-party or external-layer enum in application code to produce a wire string when the wire vocabulary is an external contract. Use an explicit exhaustive `switch` to produce the canonical wire string.

## 5. Null discipline, outcomes, and exceptions

### 5.1 Null policy

Default posture is non-null.

`null` is permitted only where absence is unsurprising and the API clearly models it: external APIs that use `null`, narrow legacy boundaries, internal caches where null means not yet loaded, or framework fields populated reflectively.

In domain code, `null` must not represent business alternatives. Model absence with `Optional<T>` for a simple optional return, a sealed result family for domain alternatives, or early rejection at the trust boundary.

### 5.2 JSpecify discipline

When the project uses JSpecify:

- Annotate every production package with `@org.jspecify.annotations.NullMarked` in `package-info.java`.
- Use `@Nullable` only where null is a deliberate, documented value at that exact site.
- Add `org.jspecify:jspecify` as `compileOnly`.
- Wire enforcement through NullAway in JSpecify mode, Checker Framework, or the repository's chosen null checker.
- Adopt nullness at package or module boundaries, not as isolated individual-method decoration.

Annotations are compile-time signals. Runtime checks at external boundaries and compact constructors remain necessary.

### 5.3 Optional discipline

`Optional<T>` models deliberate absence of a single non-null value.

Permitted:

- return type where absence is an expected outcome and callers must handle it;
- a record component representing a single independent optional attribute.

Avoid:

- method parameters;
- serialized DTO/entity fields unless the framework explicitly handles the desired shape;
- multiple optionals that secretly encode mutually exclusive states.

Never call `Optional.get()` without first establishing presence through control flow or by using `orElseThrow`, `ifPresent`, or another explicit handling method.

### 5.4 Exceptions

Throw exceptions for invariant breaches, contract violations, infrastructure failures, and states that should not occur under the type contract.

Do not throw exceptions for ordinary business alternatives that callers are expected to handle.

Prefer unchecked exceptions in domain and application logic. Checked exceptions are warranted at I/O, parsing, and external seams where callers must acknowledge a specific recoverable condition. Translate exceptions across boundaries only when the boundary requires a different contract.

### 5.5 Catch policy

Catch narrowly. Preserve meaning and cause. Do not swallow exceptions, catch broadly and return fake success, destroy interrupt status, or convert cancellation into ordinary domain failure.

### 5.6 No dead defensive checks

Do not add null checks on values whose non-null return is guaranteed by JDK or library contract. Dead checks create branches that cannot be meaningfully covered.

When uncertain, read the API contract. Add a null check only when the contract permits null or the boundary is demonstrably unreliable.

## 6. Control flow and exhaustiveness

### 6.1 Exhaustive switching

For sealed families and enums, prefer exhaustive `switch` expressions. Do not add `default` branches when all real alternatives are known; defaults weaken compiler help and can hide missing handling during evolution.

### 6.2 Pattern matching

Use pattern matching `switch` for closed-domain dispatch. Use `instanceof` pattern matching for a single simple type test followed immediately by use of the bound variable.

Do not build long `instanceof` ladders in place of an exhaustive switch over a sealed family.

### 6.3 Record patterns

Destructure a record in a pattern arm when multiple fields are consumed immediately and the record name adds no clarity in the arm body.

```java
case Committed(PostingId id, _, LocalDate date, _) -> format(id, date)
```

Prefer a named binding when only one or two fields are used, or when the variable is reused later.

Do not nest record patterns beyond two levels. Deep nesting is a signal to extract a named helper.

### 6.4 Guards

Use guarded pattern cases only when the guard materially improves clarity. If the guard-false path has meaningful behavior, prefer a separate case, an inner exhaustive switch, or a simple `if` inside the arm.

When a guard tests a sealed component field, prefer an inner exhaustive switch over the component type.

### 6.5 Multi-label pattern arms

Prefer one pattern subtype per arm. Multi-label pattern arms can obscure coverage and future evolution.

```java
// Prefer
case Foo _ -> handleBoth();
case Bar _ -> handleBoth();

// Avoid
case Foo _, Bar _ -> handleBoth();
```

### 6.6 No pre-filter before exhaustive switch

Do not filter out one subtype before an exhaustive switch over the same domain. The switch should be the sole dispatch site.

```java
// Prefer
switch (cell) {
  case BlankSnapshot _ -> blankCount++;
  case TextSnapshot _ -> populatedCount++;
}
```

### 6.7 Loops, streams, and gatherers

Choose the form with the clearest intent and cost model. Loops are fine. Streams are fine. Gatherers are justified only when they express a real, named stream transformation that is clearer than a loop or collector.

### 6.8 Local variable type inference

Use `var` when the type is obvious from the right-hand side and repeating it adds no information.

```java
var counts = new HashMap<AccountCode, Integer>();
```

Do not use `var` when the type name documents domain intent or the return type is non-obvious.

```java
PostEntryResult result = applicationService.commit(command);
```

`var` is for local variables only, not fields, method signatures, or constructor parameters.

## 7. Concurrency, parallelism, and context propagation

### 7.1 Ownership

Every asynchronous task needs an owner, lifetime, cancellation path, and shutdown path. No orphan tasks. No hidden background work.

### 7.2 Virtual threads

Prefer virtual threads for highly concurrent blocking I/O workloads. Do not use them as a CPU-bound speedup.

Do not create unbounded work just because virtual threads are cheap. Upstream and downstream resources still have limits: databases, sockets, queues, rate limits, files, locks, memory, and external APIs.

### 7.3 Locking

On Java 26+, choose between `synchronized` and `java.util.concurrent.locks` by semantics, not by obsolete virtual-thread pinning folklore.

Use `synchronized` where it is practical and clear. Use `ReentrantLock` or related locks when you need interruptible acquisition, timed acquisition, fairness, multiple conditions, read/write semantics, optimistic reads, or other advanced behavior.

Keep critical sections small. Avoid I/O, blocking calls, callbacks, or unknown user code while holding any lock.

### 7.4 Structured concurrency

When preview is enabled and subtasks are related, prefer Structured Concurrency for lexical ownership, shared cancellation, and coordinated failure handling.

If preview is not enabled, preserve the same discipline with the repository's approved concurrency model.

### 7.5 Context propagation

Prefer explicit parameters for local context. Prefer scoped values for immutable context flowing down a bounded task tree. Avoid `ThreadLocal` proliferation, especially in virtual-thread-heavy code.

### 7.6 Executors and pools

If you introduce an executor, define who creates it, who closes it, what workload it serves, its bounds, and why an existing managed facility is insufficient.

Do not create thread pools casually.

### 7.7 Blocking and cancellation

If a method blocks, make that operational fact discoverable through naming, placement, contract, or documentation.

Treat interruption and cancellation as real control flow. Restore interrupt status where appropriate. Avoid retry loops that ignore cancellation.

## 8. Architecture and boundaries

### 8.1 Visibility

Default to `private` or package-private. Widen visibility only when real consumers require it. For libraries and plugins, public surface is a compatibility commitment.

### 8.2 Layering

Keep domain logic separate from transport, persistence, framework glue, serialization, and generated code. Do not let framework annotations colonize the core domain by default.

Reusing one type across layers is acceptable only when the sameness is genuinely true and stable.

### 8.3 Serialization is a contract

Serialized shape is external contract. Do not casually change field names, optionality, enum symbols, discriminator values, polymorphic structure, date/number formatting, or error envelope shape.

For polymorphic sealed types, keep discriminator registration visible at the sealed family boundary when the serializer supports it. Discriminator values must be stable protocol vocabulary strings, not Java class names.

Never use `Id.CLASS`-style discriminators for external protocols; they leak implementation names and break versioning.

### 8.4 Naming and organization

Names must reveal domain capability. Avoid vague type, package, or module names such as `Manager`, `Helper`, `Utils`, `Misc`, `Common`, `Shared`, or `Base` unless they carry precise domain meaning in context.

If you cannot explain what a package contains without listing its members, the package needs a sharper name or a different structure.

### 8.5 JPMS

If the repository uses JPMS, module boundaries are architectural decisions.

- `exports <pkg>` exposes public types to other modules.
- `opens <pkg> to <module>` grants targeted deep reflection access.

Never add broad `opens` merely to silence an `InaccessibleObjectException`. Diagnose the specific reflective consumer and open the narrowest package to the narrowest module.

When a type moves package, update `exports` and `opens` in the same change.

### 8.6 Project-owned tooling seams

When a project uses a narrow slice of a third-party or native-backed API, define a project-owned seam for that slice and keep application code behind it.

Rules:

- Expose only operations the project consumes today.
- Name the seam by domain purpose, not vendor type.
- Remove old direct third-party call sites once the seam exists.
- Prefer deterministic pure-Java replay or test adapters when exact semantics can be reproduced locally.

### 8.7 Canonical ownership

The universal contract §5 defines the canonical-ownership rule. Java/Gradle-relevant facts that typically need a single owner: domain invariants, operation catalogs, protocol semantics, error classification systems, enum vocabularies, validation rules, configuration schema, version catalog coordinates.

Every surface that exposes the fact must derive from that owner or from generated artifacts rooted in it.

## 9. Gradle and build logic

### 9.1 Wrapper, toolchains, and Java 26 compatibility

Use the Gradle wrapper. Do not invoke a globally installed `gradle`.

Use Java toolchains for compilation and, where appropriate, test and runtime tasks. The build must not depend on whichever JDK happens to be installed on the machine.

For Java 26:

- Gradle must be new enough to support Java 26 toolchains and, if needed, running Gradle on Java 26. Use Gradle **9.4.0+** for Java 26 support.
- When upgrading the wrapper, prefer the current stable Gradle version supported by the repository's plugins rather than a minimal version alone.
- Verify Kotlin, Groovy, Android Gradle Plugin, JaCoCo, Error Prone, NullAway, Checkstyle, PMD, SpotBugs, and other tooling against the configured Java toolchain.

### 9.2 Build authoring language

For new build logic, prefer Gradle Kotlin DSL. If the repository uses Groovy DSL, preserve that choice unless migration is part of the task.

Do not turn a Java task into an accidental DSL migration.

### 9.3 Bytecode targeting

For libraries, reusable modules, plugins, or mixed-JDK ecosystems, use explicit bytecode targeting with `--release`. Do not assume `sourceCompatibility` and `targetCompatibility` alone express compatibility intent precisely enough.

### 9.4 Dependencies

Prefer version catalogs (`libs.versions.toml`) for shared dependency coordinates. Do not scatter repeated version strings across build files.

Pin versions. Avoid floating versions such as `latest.release`, `latest.integration`, or `1.+`.

Before adding a dependency:

- verify exact group ID, artifact ID, and version in the declared repository;
- verify it is not EOL or incompatible with Java 26;
- verify it is not already provided by the JDK, existing stack, or an existing dependency;
- verify the API from current documentation, not memory.

Do not add a library to avoid writing a small amount of straightforward code.

### 9.5 Repositories

Keep repositories minimal and explicit. Do not add broad or duplicate repositories casually.

### 9.6 Shared build logic

For substantial shared build logic, prefer convention plugins in an included build such as `build-logic`.

`buildSrc` is acceptable when the repository already uses it, the logic is small and local, or migration cost exceeds benefit.

Convention plugin IDs must be qualified (`com.example.project.java-library`), not generic (`java-library`, `jvm-conventions`).

### 9.7 Preview-feature wiring

If preview syntax or APIs are used, synchronize configuration across compilation, test execution, runtime tasks, CI, IDE/developer workflow, packaging, and documentation.

Do not wire preview support for only one phase. A preview feature enabled in `compileJava` but not `test` is the canonical way to ship an unverified change.

### 9.8 Build performance features

Configuration cache, build cache, parallelism, and test distribution are good when correct for the repository. Correctness first. Do not cargo-cult performance flags.

### 9.9 Multi-module structure

Keep module responsibilities sharp. Avoid circular dependencies. Put shared policy in convention plugins rather than duplicated snippets. Do not create modules that exist only to look clean without reducing coupling.

### 9.10 Null annotation build wiring

When adopting JSpecify:

- add `org.jspecify:jspecify` as `compileOnly` in annotated modules;
- wire NullAway or the chosen checker in shared build logic;
- enable JSpecify mode where supported;
- enforce consistently across modules;
- avoid partial annotation that produces false confidence.

### 9.11 Build isolation and daemon management

Never run multiple Gradle invocations concurrently against the same project directory.

For concurrent builds across different projects, isolate Gradle user homes:

```bash
GRADLE_USER_HOME="$PROJECT_ROOT/.gradle-home" ./gradlew check
```

Add `.gradle-home/` to `.gitignore` if this convention is adopted.

Do not use `./gradlew --stop` routinely. Stop daemons only to recover from confirmed daemon corruption.

Keep `org.gradle.jvmargs` at the minimum heap the project actually requires.

## 10. Testing and coverage

### 10.1 Determinism

Tests must be deterministic. Control time, randomness, environment variables, filesystem layout, locale, timezone, network behavior, and concurrency timing where practical.

### 10.2 What to test

Prioritize domain invariants, boundary mappings, result-shape decisions, serialization contracts, error translation, concurrency ownership/cancellation, public API behavior, and regressions for fixed bugs.

### 10.3 Test style

Test observable behavior and contract. Do not couple tests to incidental implementation details unless the task is specifically about those details.

Avoid reflection in tests. If a class requires reflection to test, prefer improving the design. If a private branch is genuinely unreachable through public behavior but still important, expose a narrow package-private helper in the same package and test it directly.

### 10.4 Coverage

Coverage is a signal, not the goal. Do not distort design merely to satisfy a metric. Investigate meaningful blind spots.

Avoid dead branches created by default arms over sealed domains, pre-filters before exhaustive switches, and defensive checks against impossible nulls.

### 10.5 Test organization

Default style:

- one top-level test class per production class: `<ProductionClass>Test`;
- nested classes to group scenarios;
- behavior names such as `execute_returnsFailure_whenSourcePathIsBlank()`;
- direct constructors for records;
- no mocking of record types;
- assert expected values directly, not through reflection, ordinals, or runtime type inspection.

Repository convention overrides naming style when already consistent.

## 11. Refactoring and deletion (Java-specific notes)

The universal contract covers Boy Scout + Mikado discipline (§3), architecture as preserved theory (§4), and deletion-requires-proof (§8). The notes below add Java/Gradle-specific concerns; they are not a replacement for those sections.

### 11.1 Compatibility-aware refactoring

Refactor aggressively inside private and internal surfaces. Refactor public or published surfaces deliberately, with migration cost, binary/source compatibility, serialization, and user contracts treated as design inputs. For libraries and plugins, Naur's "amorphous additions" warning bites hardest at the public surface — every patch made without the published-API theory tends to leak shape.

### 11.2 Structural tasks

When the task is about scaffolding, architecture, or repository cleanup, audit the whole affected surface: module layout, package names, build logic, convention plugins, dependency centralization, CI assumptions, generated code, and verification tasks.

Do not stop at the first file named in the prompt if the real problem is structural.

### 11.3 God constructs

A god construct concentrates unrelated responsibilities in one place.

Refactoring signals:

- **God class:** factory logic, descriptors, validation, lookup, and lifecycle logic mixed together.
- **God record:** many optional or nullable fields encoding mutually exclusive states.
- **God method:** long method split by inline phase comments or mixing unrelated responsibilities.

Refactor by extracting cohesive types or helpers named for domain purpose. Never extract merely to save lines.

### 11.4 JPMS and reflection deletion hazards

Java-specific deletion hazards beyond the universal §8 list:

- `opens` directives and the reflective consumers they serve;
- `ServiceLoader` registrations under `META-INF/services/`;
- `Class.forName`, MethodHandles, VarHandles, and other late-bound references;
- annotation processors, KAPT/KSP, and generated code rooted in deleted types;
- preview-feature flags whose removal silently downgrades an in-use API.

## 12. Documentation and self-containment

### 12.1 Javadoc

Public APIs require Javadoc that states purpose and contract. Published-library APIs require especially careful compatibility and behavior prose.

Package-private APIs require Javadoc when they are part of an internal contract, widened for testing, non-obvious, or reused across classes.

Record component accessors usually do not need Javadoc beyond clear component names. `@Nullable` parameters and returns must explain when null is expected and what it means.

### 12.2 Style

- One clear sentence first.
- No filler such as "This method..." or "This class...".
- Use `@param` and `@return` only when names alone are insufficient.
- Do not add comments or Javadoc that merely restate code.
- Use inline comments only for non-obvious reasoning, invariants, or boundary decisions — i.e., where the *why* (per universal contract §1.5 Justification) cannot be read off the code.

### 12.3 Self-containment

Source code, Javadoc, comments, and product documentation must not reference agent directive files by name, section number, or as justification for a design decision.

```java
// Forbidden
// Per AGENTS.md, no default on sealed switch.

// Correct
// No default: compiler enforces exhaustiveness over sealed subtypes.
```

Agent directive files are operational instructions, not developer-facing design records.

## 13. CI and project automation

### 13.1 CI mirrors local verification

The canonical verification command must pass locally and in CI with identical strictness. Do not create CI-only checks that cannot be reproduced locally. Do not soften local checks based on `CI=true`.

### 13.2 Pin third-party actions

Third-party CI actions should be pinned to full-length commit SHAs, not mutable tags.

```yaml
# Prefer
uses: actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683  # v4.2.2

# Avoid
uses: actions/checkout@v4
```

### 13.3 Timeouts and stale runs

Every CI job should declare `timeout-minutes` appropriate to observed runtime.

Use concurrency groups with `cancel-in-progress: true` to abort obsolete runs on the same branch.

```yaml
concurrency:
  group: ci-${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true
```

### 13.4 Dependency freshness

Use either asynchronous dependency automation or a sync gate paired with automation. A blocking dependency-freshness gate without automated PR creation turns unrelated work into manual dependency maintenance.

## 14. Incidental observation protocol

When reading a file surfaces a defect, rule violation, or clear improvement outside the active task, record it in the project's designated observation log if one exists. Do not derail the active task unless the issue blocks correctness or safety. This is the Java-side practice for honoring the universal contract's rule that the next improvement is a separate slice (§10).

Each entry should record:

- stable ID;
- date;
- status;
- file and line range;
- category;
- what is wrong and why it matters;
- current pattern or excerpt;
- resolving change;
- effort level.

When resolved, update the entry in place rather than deleting it. If no observation log is defined, mention material observations in the work summary only when relevant.

## 15. Pre-output checklist

The universal contract §10 (stop conditions) and §9 (output contract) define the cross-language stops. The checks below are Java/Gradle-specific additions; do not duplicate the universal output template here.

### Java semantics

- Are domain alternatives explicit rather than hidden in `null`, flags, magic strings, or exceptions?
- Are invariants enforced at type boundaries or constructors?
- Are expected outcomes separated from exceptional failures?
- Are sealed or enum domains handled exhaustively?
- Are `Optional` and `@Nullable` used deliberately?

### API and boundaries

- Is visibility as narrow as possible?
- Are public surfaces compatible with their consumers?
- Are serialization and external contracts preserved or intentionally evolved?
- Are enum-to-wire mappings explicit where the wire vocabulary matters?

### Concurrency

- Does every asynchronous task have an owner, cancellation path, and shutdown path?
- Are locks chosen by semantics rather than folklore?
- Is context propagation explicit and bounded?
- Is interruption/cancellation preserved?

### Build

- Did you use the wrapper and toolchains?
- Is Gradle new enough for Java 26 when Java 26 is required?
- Are versions pinned and centralized?
- Are preview features wired consistently across compile, test, runtime, IDE, and CI?
- Did you avoid concurrent Gradle invocations in the same project?

### Verification

- Did you run the smallest sufficient verification path?
- Did verification widen when the change widened?
- Are warnings resolved rather than suppressed?
- Does the repository end more coherent than it started?
