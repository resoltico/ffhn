# Kotlin 2.4+ / Gradle Agent Protocol

**Scope:** Kotlin repositories that intentionally use Kotlin **2.4+** or are being migrated to it. This includes Kotlin/JVM, Kotlin Multiplatform, Kotlin/Native, Kotlin/Wasm, Kotlin/JS, Android Kotlin modules, libraries, CLIs, services, plugins, and multi-module Gradle builds.

**Current posture:** Kotlin 2.4 may be an EAP/Beta in the target repository. Treat EAP adoption as an explicit repository decision, not as a default upgrade path. If the project is on Kotlin 2.3.x or lower, do not silently migrate it to Kotlin 2.4+ unless the task is a migration or the repository already opts in.

**Build default:** Gradle Kotlin DSL. Do not introduce Groovy build logic. Use Maven guidance only when the repository is already Maven-based.

**Compiler default:** K2 is the normal compiler path. Do not add compatibility shims for K1-era behavior unless the repository has a documented reason.

Optimize in this order:

```text
correctness → explicit contracts → concurrency correctness → narrow API → evolution safety → readability → terseness
```

Terseness loses to clarity. Convenience loses to correctness. Cleverness loses to maintainability.

This protocol inherits `.codex/UNIVERSAL_ENGINEERING_CONTRACT.md`. Do not duplicate the universal contract here; apply it before all Kotlin-specific rules.

---

## 1. Repository intake before touching code

Before editing Kotlin, derive the repository's actual baseline.

Check:

- Kotlin version and plugin versions in `gradle/libs.versions.toml`, `settings.gradle.kts`, root `build.gradle.kts`, and convention plugins.
- Whether Kotlin 2.4 is GA, Beta, RC, or EAP in this repository, and whether EAP repositories are configured deliberately.
- Target platforms: JVM, Android, Multiplatform, Native, Wasm, JS.
- Java toolchain and `jvmTarget` / `compilerOptions` alignment.
- Gradle wrapper version and whether it supports the selected JDK/toolchain.
- Compiler flags: context parameters, collection literals, explicit context arguments, return-value checker, warning policy, explicit API mode, progressive mode, opt-ins.
- Kotlin compiler plugins: serialization, KSP, all-open, no-arg, Compose compiler, Spring, JPA, Dokka, binary compatibility validation, Android Gradle Plugin.
- Public API posture: application, internal library, published SDK, Gradle plugin, framework integration, or multiplatform package.
- Test infrastructure: JUnit, Kotest, kotlinx-coroutines-test, MockK, Testcontainers, Android instrumentation, Native/JS/Wasm test tasks.
- CI tasks and whether local verification exactly mirrors CI.

Do not infer the baseline from file names alone. The version catalog and convention plugins are usually the canonical build truth.

---

## 2. Kotlin 2.4+ feature posture

Use Kotlin 2.4+ features only according to their stability and repository opt-in status.

### 2.1 Stable in Kotlin 2.4.0-Beta2

When the repository is intentionally on Kotlin 2.4+, these can be treated as normal language or library tools unless project policy says otherwise:

- Context parameters, except callable references and explicit context arguments.
- Explicit backing fields.
- `@all` meta-target for properties.
- New defaulting rules for annotation use-site targets.
- `kotlin.uuid.Uuid` common API, except UUID V4/V7 generation functions that still require opt-in.
- Sorted-order checks such as `isSorted`, `isSortedDescending`, `isSortedWith`, `isSortedBy`, and `isSortedByDescending`.
- JVM `UInt.toBigInteger()` and `ULong.toBigInteger()`.
- Kotlin/JVM support for Java 26 bytecode.
- Kotlin metadata annotations enabled by default on JVM.
- Kotlin/Wasm incremental compilation enabled by default.
- Kotlin/JS value class export to JavaScript/TypeScript and ES2015 support inside `js()` inline code.

Use these features only when they improve the system's theory, not merely because they are new.

### 2.2 Experimental in Kotlin 2.4.0-Beta2

Do not introduce these unless the repository already enables the flag or the task explicitly asks for adoption:

- Explicit context arguments: `-Xexplicit-context-arguments`.
- Collection literals: `-Xcollection-literals`.
- Improved compile-time constant evaluation: `-XXLanguage:+IntrinsicConstEvaluation`.
- WebAssembly Component Model support.
- UUID V4/V7 generation APIs.
- Any Kotlin/Native, Swift export, JS, Wasm, or metadata feature still marked Experimental by the compiler or documentation.

When adding an experimental feature deliberately, update the canonical build policy, add a short rationale, isolate the feature behind explicit compiler flags, and add verification that fails if the flag is removed accidentally.

### 2.3 Rich Errors posture

Do **not** write production Kotlin as if Rich Errors are an available Kotlin 2.4 compiler feature unless the repository already contains an officially supported compiler build and syntax for them.

Until Rich Errors are implemented and stabilized in the compiler, model recoverable domain failures with named sealed hierarchies, explicit result types, or a carefully justified `Result`/Either-style abstraction.

Do not hallucinate syntax such as:

```kotlin
fun loadUser(id: UserId): User | UserError
error class UserError(...)
```

Treat Rich Errors as a future design direction: valuable for thinking about explicit recoverable failures, not as code the agent may invent.

---

## 3. Hard boundaries

Violating these requires explicit repository policy or user authorization.

### 3.1 Type and domain safety

- Never use `!!` unless an invariant is already proven locally or by an external contract and the proof is visible.
- Never use unsafe casts where `as?`, smart casts, generics, sealed types, or better modeling remove the need.
- Never encode ordinary domain alternatives as crashes.
- Never use nullable return types to represent multiple distinct business outcomes.
- Never expose mutable collections or mutable state directly from public APIs.
- Never widen mutability or visibility for convenience.
- Never represent protocol facts as magic strings, booleans, or integers when a named type, enum, value class, or sealed family expresses the contract.

### 3.2 Coroutine and concurrency safety

- Never use `GlobalScope` in production code.
- Never launch coroutines without an owning lifecycle or scope.
- Never call `runBlocking` from suspending code or from code that may run on an event loop, UI thread, servlet thread, or test scheduler.
- Never convert `CancellationException` into ordinary failure.
- Never swallow failures with broad `catch` blocks that hide cancellation or erase evidence.
- Never expose fire-and-forget APIs unless the owning lifecycle, cancellation, and failure reporting are explicit.

### 3.3 API and structure

- Never make declarations `public` by accident.
- Never use boolean mode flags in public APIs when separate functions or a semantic enum is clearer.
- Never create generic buckets named `util`, `helpers`, `misc`, `common`, `manager`, `processor`, or `base` when a domain name exists.
- Never use inheritance where composition expresses the problem more directly.
- Never add abstraction layers that do not pay for themselves immediately.
- Never write Java-shaped Kotlin when idiomatic Kotlin is clearer and equally explicit.

### 3.4 Build and dependency safety

- Never guess Gradle plugin IDs, version catalog coordinates, Maven coordinates, compiler flags, or library APIs.
- Never introduce an EAP, Beta, Alpha, snapshot, or unreleased dependency unless the repository already has an EAP policy or the user explicitly asks.
- Never run concurrent `./gradlew` invocations against the same project directory.
- Never respond to Kotlin daemon failures by editing source or build logic before verifying daemon/process state.
- Never suppress warnings to make verification pass unless the suppression is narrow, documented, and tied to a real false positive or unavoidable interop boundary.

---

## 4. Type system and domain modeling

### 4.1 Prefer types that make invalid states difficult

Choose types that express the domain:

| Need | Preferred Kotlin construct |
|---|---|
| Stateless transformation | top-level or member function |
| Data carrier with value semantics | `data class` |
| Closed alternatives | `sealed interface` / `sealed class` |
| Single-instance alternative | `data object` |
| Small symbolic set | `enum class` |
| Primitive-shaped semantic identity | `@JvmInline value class` |
| Capability contract | `interface` |
| Internal naming shortcut | nested `typealias` when it improves locality |
| Shared dependency available in a lexical context | context parameter, when deliberate |

Do not introduce a type merely to look abstract. Every type must either prevent misuse, name a domain concept, isolate a boundary, or make evolution safer.

### 4.2 Nullability

Use nullable types only when absence is a normal state: optional field, cache miss, absent parent, missing value in a parsed external record.

Do not use nullable types for validation failure, protocol rejection, authorization failure, parse ambiguity, or multiple distinguishable outcomes. Use a named result model.

Avoid nullable parameters in public APIs. Prefer overloads, default parameters, or a semantic option type. A nullable parameter pushes ambiguity to every caller.

A data class with several nullable fields that are only valid in certain combinations is a hidden state machine. Refactor it into a sealed family or another explicit state representation.

### 4.3 Recoverable outcomes

Use sealed hierarchies when callers must distinguish outcomes behaviorally:

```kotlin
sealed interface RegistrationResult {
    data class Success(val id: UserId) : RegistrationResult
    data object EmailAlreadyTaken : RegistrationResult
    data class ValidationError(val violations: List<Violation>) : RegistrationResult
}
```

Prefer named sealed outcomes for domain workflows. Use `kotlin.Result` only when the failure domain is naturally exception-shaped and the caller does not need a domain-specific error taxonomy.

Do not use magic strings, booleans, or `Pair<T, Error?>`-style structures to encode failures.

### 4.4 Sealed families

Use sealed families for protocol messages, parser outcomes, command results, state machines, domain failures, and capability results.

Rules:

- Variants must be coherent and semantically named.
- Exhaustive `when` is preferred for closed families.
- Do not add `else` to a `when` over a sealed family unless there is a real open-world boundary.
- Adding a public sealed subtype is a compatibility decision for libraries, not a refactor.
- Do not use `Unknown`, `Other`, or `GenericError` buckets unless the external boundary genuinely permits unknown cases.

### 4.5 Value classes

Use `@JvmInline value class` to prevent mix-ups of primitive-shaped values:

```kotlin
@JvmInline value class UserId(val value: String)
@JvmInline value class Cents(val value: Long)
@JvmInline value class TenantSlug(val value: String)
```

Do not wrap primitives decoratively. A value class must encode domain meaning or boundary safety.

For Java callers, verify constructor/function exposure. Use interop annotations only when they materially improve the Java API.

### 4.6 Immutability

Default to `val`. Prefer immutable state transitions over in-place mutation.

`List<T>`, `Set<T>`, and `Map<K, V>` are read-only interfaces, not proof of deep immutability.

Rules:

- Store mutable collections privately.
- Expose read-only views only when aliasing is safe.
- Copy defensively at boundaries where mutation aliasing is dangerous.
- Use persistent immutable collections when real structural immutability is required.
- Keep state ownership explicit: source of truth, derived state, cache, and view state must not be confused.

### 4.7 Nested type aliases

Use nested type aliases when they keep implementation vocabulary local and reduce package-level clutter.

Do not use type aliases to hide an important dependency, erase domain meaning, or create shadow contracts. A type alias is naming help, not a new type.

---

## 5. Kotlin 2.4 language features in practice

### 5.1 Context parameters

Context parameters can express ambient capabilities that are genuinely shared across a lexical operation: clock, logger, transaction context, locale, authorization view, tracing span, or domain policy.

Use them when they make dependency flow clearer than manual parameter threading.

Do not use context parameters as a service locator, global registry, or hidden dependency bag. If a dependency is part of an object's durable state, constructor injection is usually clearer. If a dependency is part of one operation, an explicit parameter may be clearer.

Rules:

- Name context parameters unless `_` is materially clearer.
- Keep context sets small and capability-oriented.
- Do not mix `-Xcontext-receivers` and `-Xcontext-parameters`.
- Do not convert existing dependency injection to context parameters merely because the feature is stable.
- Treat explicit context arguments as experimental until the compiler stabilizes them.

### 5.2 Explicit backing fields

Use explicit backing fields when they clarify state ownership and remove a noisy private backing property:

```kotlin
val city: StateFlow<String>
    field = MutableStateFlow("")

fun updateCity(newCity: String) {
    city.value = newCity
}
```

This is useful when the exposed type is narrower than the stored implementation type.

Do not use explicit backing fields to hide mutation, bypass invariants, or make ownership unclear. If state is externally observable, the invariant and mutation paths must remain obvious.

### 5.3 Annotation target rules and `@all`

At framework and wire boundaries, annotations are part of the contract.

Use explicit use-site targets when the target matters:

```kotlin
data class UserDto(
    @field:Email
    @get:Email
    val email: String,
)
```

Use `@all:` only when the annotation genuinely belongs on all relevant property targets and doing so does not change framework behavior unexpectedly.

When migrating to Kotlin 2.4 annotation defaulting rules, verify serialization, validation, DI, persistence, reflection, and annotation-processing behavior. Annotation placement drift can be a runtime contract bug.

### 5.4 Guard conditions and context-sensitive resolution

Use guard conditions in `when` when they make closed-domain branching clearer. Keep guards simple and side-effect-free.

Context-sensitive resolution can reduce noise around enum and sealed members, but do not sacrifice readability at module or API boundaries. In ambiguous files or public examples, explicit qualification may still be better.

### 5.5 Multi-dollar interpolation and multiline strings

Use triple-quoted strings for multiline SQL, JSON, XML, GraphQL, expected output, shell scripts, and generated snippets.

Use multi-dollar interpolation only when it makes literal `$`-heavy content clearer. Do not introduce it for ordinary strings.

### 5.6 Collection literals

Collection literals are experimental in Kotlin 2.4. Do not use them in production code unless the repository explicitly enables `-Xcollection-literals`.

Even when enabled, prefer conventional constructors when the expected collection type is not obvious. Bracket syntax must not hide mutability or custom `operator fun of` behavior.

### 5.7 Compile-time constants

Improved compile-time constant evaluation is experimental. Do not rely on it unless the repository enables the feature deliberately.

If enabled, keep compile-time constants boring and auditable. Do not encode business logic into constants merely because the compiler can evaluate more expressions.

### 5.8 UUID and sorted checks

Use `kotlin.uuid.Uuid` for common multiplatform UUID values when it avoids platform-specific UUID shims.

Do not use experimental UUID generation APIs without explicit opt-in.

Use sorted-order functions such as `isSorted()` and `isSortedBy()` instead of hand-written loops when they directly express the invariant being checked.

---

## 6. Functions, expressions, and control flow

### 6.1 Function design

A function should do one coherent thing at one abstraction level. Inputs, outputs, side effects, and failure shape must be visible.

Prefer:

- small parameter lists with semantic types,
- explicit return types for public/protected declarations,
- deterministic behavior where possible,
- no hidden I/O,
- no hidden mutation,
- named local values for multi-step reasoning.

### 6.2 Expression style

Expression-bodied functions are good when the expression is obvious. Block bodies are better for branching, local names, side effects, validation, or non-trivial reasoning.

Do not compress code into a single expression to appear idiomatic.

### 6.3 Scope functions

Use scope functions only when the receiver or temporary name improves clarity.

| Function | Good use |
|---|---|
| `apply` | object configuration |
| `also` | observation side effect, such as logging or metrics |
| `let` | short transform, especially on nullable value |
| `run` | receiver-based block that genuinely helps |
| `with` | local block around one clear receiver |

Rules:

- Do not chain more than two scope functions.
- Do not nest scope functions when `this`/`it` becomes ambiguous.
- Prefer named locals over clever receiver gymnastics.
- Break call chains before stack traces and debugging become hard.

### 6.4 Destructuring

Use destructuring when all components are consumed immediately and their local names are clearer than property access.

Do not destructure solely for brevity. Prefer named access when only one component is needed, when the original object is reused later, or when component order is not self-evident.

### 6.5 Return-value discipline

Ignored non-`Unit` return values are often bugs. When the repository enables the unused return-value checker, treat findings as correctness signals.

Use `@MustUseReturnValues` for APIs where ignoring the result is dangerous. Use `@IgnorableReturnValue` only for functions where ignoring the result is conventional and safe.

Assign to `val _ = ...` only when the discard is deliberate and locally obvious.

---

## 7. Error handling and failure semantics

### 7.1 Separate recoverable errors from exceptional failures

Use exceptions for unrecoverable failures, broken preconditions, framework boundaries, or non-local failure handling where exception flow is the right contract.

Use explicit result models for recoverable business outcomes, validation failures, parse results, authorization decisions, and protocol alternatives.

### 7.2 Catch narrowly

Catch the narrowest exception type you can handle meaningfully.

Do not catch `Throwable` except at process-level supervision, crash reporting, or framework boundaries that must prevent process termination. Always preserve the cause and context.

### 7.3 `runCatching`

Do not use `runCatching` as a blanket replacement for error modeling.

Use it only when:

- the failure domain is truly exception-shaped,
- cancellation is preserved,
- the resulting code is clearer than explicit `try`/`catch`, and
- callers still receive a meaningful failure shape.

Do not accidentally normalize `CancellationException`.

### 7.4 Validation

Ordinary invalid user input should normally produce an explicit validation result, not an exception.

Throw for violated programmer preconditions. Return a domain result for user/business validation.

### 7.5 Error messages

Error messages are user and operator feedback surfaces. They must preserve enough context to diagnose the failure without leaking secrets.

Do not replace specific errors with vague strings like `failed`, `invalid`, or `unknown` unless the boundary requires redaction.

---

## 8. Coroutines, Flow, and concurrency

### 8.1 Structured concurrency

Prefer `suspend` functions for one-shot async operations. Use `Flow` for asynchronous streams, not for a single immediate value.

Child work must belong to a parent scope. Use `coroutineScope` when child failure should fail the whole operation. Use `supervisorScope` only when sibling isolation is intentional and failures are still observed.

### 8.2 Cancellation

Cancellation is normal control flow.

Rules:

- Rethrow `CancellationException` after cleanup.
- Do not log normal cancellation as an application error.
- Ensure long loops cooperate with cancellation through `ensureActive()`, `yield()`, suspending calls, or explicit checkpoints.
- Do not hide cancellation inside `Result`, `Either`, `runCatching`, retry wrappers, or broad catch blocks.

### 8.3 Dispatchers and blocking work

Do not hide blocking I/O in CPU-oriented paths or default dispatchers.

Use dispatcher boundaries deliberately. `withContext(Dispatchers.IO)` is not a magic fix; it is a statement that blocking or I/O work is happening.

For libraries, avoid hardcoding dispatchers unless the dispatcher is part of the contract. Prefer accepting a dispatcher, scope, or execution policy when needed.

### 8.4 Flow

Use `Flow` for streams with multiple values over time, reactive pipelines, event feeds, or observable state.

Rules:

- Keep cold vs hot flow semantics explicit.
- Do not expose mutable flow types directly.
- Prefer `StateFlow` / `SharedFlow` only when their replay, lifecycle, and ownership semantics fit.
- Document threading, replay, completion, and error behavior for public flows.
- In Swift export / multiplatform APIs, verify how `Flow` appears to consumers.

### 8.5 Shared mutable state

Shared mutable state must have one owner and a synchronization strategy.

Choose the simplest correct tool:

- immutable snapshots,
- actor/message passing,
- `Mutex`,
- atomic primitives,
- database transaction,
- single-threaded confinement,
- framework-managed state container.

Do not mix synchronization strategies casually.

---

## 9. Architecture and module boundaries

### 9.1 Visibility default

Use the narrowest visibility:

- `private` for file/class internals,
- `internal` for module collaboration,
- `public` only for deliberate external contracts.

For libraries, enable explicit API mode. Public and protected declarations must have explicit visibility and return types.

### 9.2 Layering

Keep domain logic separate from transport, persistence, serialization, UI, CLI, HTTP, framework annotations, and infrastructure where that separation buys clarity and testability.

Do not over-layer small applications. The correct boundary is the one that preserves truth, feedback, consequence, and invariants with minimum ceremony.

### 9.3 Dependency injection

Prefer constructor injection and explicit parameters.

Reject service locators, ambient registries, hidden singletons, and global mutable objects unless a framework boundary makes them unavoidable. When unavoidable, isolate them at the adapter edge.

Context parameters can support scoped capabilities, but they are not a substitute for architecture.

### 9.4 Files, packages, and modules

Organize by domain capability and boundary, not by vague reuse category.

Good names: `billing`, `settlement`, `ledger`, `verification`, `identity`, `jurisdiction`, `invoice`, `outbox`.

Bad names without sharper meaning: `utils`, `common`, `shared`, `misc`, `core`, `base`, `helpers`.

A file should contain one coherent responsibility cluster. Do not create god files or dozens of trivial fragments.

### 9.5 Canonical ownership

Every contract-defining fact has one canonical owner:

- enum wire vocabularies,
- validation limits,
- feature capabilities,
- command names,
- error codes,
- schema names,
- route names,
- event types,
- permission names,
- generated docs and examples.

Other surfaces derive from the canonical owner or from generated artifacts rooted in it. Drift must fail verification.

---

## 10. Serialization and external contracts

### 10.1 Serialized shape is public API

Persisted or published serialized shape is an external contract. Do not casually change field names, nullability, optionality, enum symbols, discriminator values, polymorphic structure, date/time formats, numeric precision, or default values.

### 10.2 DTOs and domain models

Use separate DTOs when external shape and domain shape differ materially.

Do not contort domain objects to mirror poor transport formats unless the repository intentionally accepts that tradeoff.

### 10.3 Enum wire vocabulary

Do not rely on enum `.name` for wire representation when the vocabulary is externally meaningful.

For `kotlinx.serialization`, use `@SerialName` explicitly when the wire name differs from the Kotlin name or is part of a published contract. Treat `@SerialName` as stable once published.

For Jackson, map wire values explicitly with the appropriate annotations or adapters. Verify Kotlin module behavior.

### 10.4 Polymorphic serialization

For sealed or polymorphic hierarchies, define discriminator policy at the sealed root or central serializers module. Do not scatter discriminator logic across subtypes.

Adding a new subtype must force a visible update to serialization registration, docs, and tests.

### 10.5 Parse defensively, emit canonically

Accept external inputs defensively where the boundary requires tolerance. Emit one canonical form. Keep adaptation at the boundary, not in the domain core.

---

## 11. Java, JVM, Android, and metadata interop

### 11.1 Java 26 and JVM target alignment

Kotlin 2.4.0-Beta2 can generate Java 26 bytecode. Use Java 26 only when the repository's deployment, Gradle wrapper, toolchain, test runtime, static analysis, and downstream consumers support it.

Align:

- Java toolchain,
- Kotlin `jvmTarget`,
- Java `--release` / source / target settings,
- Gradle wrapper version,
- CI JDK,
- runtime container/base image,
- bytecode consumers and published artifact expectations.

Do not raise bytecode target as a local convenience.

### 11.2 Java-facing APIs

Design Java interop deliberately when Java callers exist.

Use `@JvmStatic`, `@JvmOverloads`, `@Throws`, `@JvmName`, `@JvmSynthetic`, and boxed value-class exposure only when they improve the Java contract. Do not decorate APIs automatically.

For public libraries, test Java call sites for important APIs.

### 11.3 Interface defaults

Kotlin's JVM default-method behavior is a binary compatibility surface. For new code, `NO_COMPATIBILITY` may be appropriate. For existing published libraries, changing default-method mode can break consumers.

Treat `jvmDefault` as build policy, not per-module improvisation.

### 11.4 Kotlin metadata annotations

Kotlin 2.4 enables annotations in metadata by default on JVM. This can affect annotation processors, metadata readers, serialization tooling, binary compatibility tools, and code generators.

When modifying annotations on public declarations, verify both bytecode-level behavior and Kotlin metadata consumers where applicable.

### 11.5 Android

For Android projects, verify Android Gradle Plugin, Kotlin plugin, Compose compiler, KSP, desugaring, minSdk, targetSdk, JDK, and test tooling compatibility before upgrading Kotlin or Java target.

Do not assume server-side JVM advice applies unchanged to Android. Android runtime, desugaring, bytecode target, resource processing, and instrumentation tests are separate constraints.

---

## 12. Multiplatform, Native, Wasm, and JS

### 12.1 Multiplatform source sets

Keep common code genuinely platform-neutral. Do not leak JVM classes, Android classes, Foundation types, Node globals, or browser APIs into `commonMain`.

Use `expect`/`actual` when the domain contract is common but implementation is platform-specific.

### 12.2 Kotlin/Native and Swift export

When exposing APIs to Swift:

- verify exported names, nullability, generics, exceptions, suspend functions, `Flow`, enum mapping, and value semantics,
- keep Swift-facing APIs small and deliberate,
- test from Swift where the contract matters,
- do not assume a Kotlin-idiomatic API is Swift-idiomatic.

Swift package import and Flow export are important Kotlin 2.4 Native capabilities, but adoption must be verified against Xcode, Gradle, dependency, and CI constraints.

### 12.3 Native memory and concurrency

Kotlin/Native GC behavior changed over recent releases. Do not cargo-cult old freezing or memory-manager rules. Verify the current runtime behavior and repository target versions.

Performance-sensitive Native changes need benchmarks or measurable runtime evidence.

### 12.4 Kotlin/Wasm

Kotlin/Wasm incremental compilation is stable and enabled by default in Kotlin 2.4. Do not disable it unless diagnosing a confirmed compiler/build issue.

The WebAssembly Component Model support is experimental. Do not introduce it without explicit opt-in, runnable examples, and CI coverage.

### 12.5 Kotlin/JS

When exporting to JavaScript/TypeScript:

- exported names and generated declarations are public API,
- verify TypeScript consumption,
- avoid exposing Kotlin-only domain shapes that are awkward in JS,
- treat value-class export as a contract decision,
- keep `js()` inline code constant, auditable, and minimal.

Do not use JavaScript dynamic interop where typed declarations or generated bindings can express the contract.

---

## 13. Build logic and Gradle

### 13.1 Default build posture

Use:

- `settings.gradle.kts`, `build.gradle.kts`, and convention plugins,
- `gradle/libs.versions.toml` for plugin and dependency versions,
- `build-logic` included build for substantial shared build policy,
- explicit Java toolchains,
- centralized Kotlin `compilerOptions`,
- explicit test tasks and quality gate,
- type-safe project accessors for multi-module projects when appropriate.

Do not hardcode versions in module build files.

### 13.2 Kotlin and plugin version alignment

Kotlin compiler plugins must be version-compatible with the Kotlin compiler:

- `org.jetbrains.kotlin.plugin.serialization`,
- KSP,
- Compose compiler plugin,
- all-open/no-arg/spring/jpa plugins,
- Dokka,
- binary compatibility tooling,
- static analysis that embeds or expects a Kotlin compiler version.

Before upgrading Kotlin, verify every compiler plugin and static analysis tool. If a stable tool does not yet support Kotlin 2.4, prefer postponing that tool or the migration over adding unstable tooling to production.

### 13.3 Compiler options

Put shared compiler options in convention plugins, not copy-pasted module blocks.

Use `compilerOptions {}` rather than deprecated configuration surfaces.

Keep opt-ins explicit and scoped. A broad project-wide opt-in is a design decision and must be justified by the repository policy.

### 13.4 Explicit API mode

Enable explicit API mode for libraries and SDKs. Application modules usually do not need it.

Use warning mode only as a migration step. Published libraries should converge to strict explicit API mode.

### 13.5 Warning policy

Warnings are feedback. Do not globally disable them.

Use `-Werror` only when the repository can keep it green consistently. If specific diagnostics must be tuned, use a centralized warning policy and document why.

The return-value checker is especially useful for command-style APIs, persistence writes, validation results, and domain operations where ignored results are dangerous.

### 13.6 Build isolation and daemon management

The Gradle daemon and Kotlin compile daemon are shared machine resources.

Rules:

- Do not run multiple Gradle invocations concurrently in the same project directory.
- In multi-agent/multi-project environments, isolate daemon pools with project-local `GRADLE_USER_HOME` when needed.
- Add project-local Gradle homes to `.gitignore`.
- Use `./gradlew --stop` only to recover from confirmed daemon corruption, not as routine build hygiene.
- Treat “Could not connect to Kotlin compile daemon” and similar failures as infrastructure first. Retry cleanly before editing build logic.

### 13.7 Dependency anti-hallucination

Before adding or updating any dependency:

- verify exact group, artifact, and version in the declared repository,
- verify Kotlin version compatibility,
- verify platform compatibility,
- verify license and security posture if relevant,
- verify scope: `implementation`, `api`, `compileOnly`, `runtimeOnly`, `testImplementation`, `ksp`, `kapt`, or platform-specific source set,
- verify whether the standard library, JDK, kotlinx library, or existing stack already covers the need.

Do not invent coordinates or assume APIs from memory.

---

## 14. Testing and verification

### 14.1 Red → Green → Refactor

For new behavior, start with the smallest failing proof: unit test, property test, integration test, compiler check, serialization round trip, or reproducible script.

Then implement the minimum code that passes. Then refactor until the touched system is clearer and easier to change.

### 14.2 Test behavior and invariants

Prioritize:

- domain invariants,
- error paths,
- cancellation behavior,
- boundary conditions,
- serialization round trips,
- Java/Swift/JS interop where public,
- concurrency correctness,
- numeric precision,
- public API compatibility,
- migration behavior.

Do not overfit tests to private implementation details.

### 14.3 Coroutine tests

Use `kotlinx-coroutines-test` for coroutine behavior. Prefer `runTest` and controlled schedulers.

Avoid sleeps, wall-clock timing, thread races, and remote networks. Advance virtual time deliberately.

Verify cancellation and child failure semantics, not only happy-path results.

### 14.4 Multiplatform tests

For multiplatform code, test the common contract and platform-specific actual implementations.

Do not assume JVM tests prove Native, JS, Wasm, or Android behavior.

### 14.5 Verification scope

Run the narrowest relevant verification first, then widen based on consequence:

1. touched module compile/test,
2. dependent modules if public contracts changed,
3. affected platform tasks,
4. full quality gate for scaffolding, build, serialization, public API, or cross-module changes.

Report what was run and what could not be run.

---

## 15. Public API and evolution

### 15.1 Published contracts

For libraries, SDKs, Gradle plugins, public APIs, and multiplatform packages, treat the following as contract:

- public/protected declarations,
- constructor signatures,
- default parameter values,
- type parameters and variance,
- sealed family shape,
- annotations visible to processors or reflection,
- serialized shape,
- generated TypeScript/Swift/Java API,
- binary compatibility and metadata,
- compiler flags that affect emitted ABI.

### 15.2 Compatibility checks

Use binary compatibility validation or equivalent where the project publishes Kotlin APIs.

When changing public API, state whether the change is source-compatible, binary-compatible, and serialization-compatible.

### 15.3 App vs library posture

Application code can prioritize operational clarity and internal maintainability.

Library code must prioritize consumer clarity, compatibility, conservative API surface, and predictable behavior across Kotlin/Java/platform versions.

---

## 16. Documentation, KDoc, and generated surfaces

Load `.codex/PROTOCOL_AFAD.md` when doing documentation work or code changes that alter documented public contracts, except for the repository root `README.md` special case defined in `AGENTS.md`.

KDoc rules:

- Add KDoc to public library APIs where the signature does not fully explain behavior, failure, threading, units, or compatibility.
- Do not add KDoc that restates obvious code.
- Document cancellation, dispatcher, and Flow semantics for coroutine APIs.
- Document serialization, wire names, and compatibility-sensitive defaults.
- Keep examples small, compileable, and synchronized with the canonical API.

Generated documentation must derive from canonical code or schema. Do not maintain duplicate contract facts manually.

---

## 17. Scaffolding and structural overhaul

When creating or restructuring a Kotlin/Gradle project, audit the whole project shape, not just the requested file.

Required surfaces:

- `settings.gradle.kts` with included builds, project names, repositories, modules, and type-safe accessors when appropriate.
- `gradle/libs.versions.toml` with all plugin and dependency versions.
- `build-logic/` convention plugins for shared Kotlin/JVM, Kotlin library, Kotlin application, Kotlin Multiplatform, Android, serialization, test, and publishing policy as needed.
- Root `build.gradle.kts` with only root-level coordination.
- `gradle.properties` with deliberate Gradle/Kotlin daemon, cache, parallelism, configuration cache, and Kotlin code style settings.
- `.gitignore` matching Kotlin/Gradle/IDE/build artifacts and any project-local Gradle home.
- CI workflows that use the wrapper, correct JDK, Gradle caching, explicit timeouts, stale-run cancellation, and the same quality gate as local verification.

Break old layout when it is structurally wrong. Backwards compatibility with a poor repository layout is not a goal unless consumers depend on it.

Recommended multi-module physical grouping when no better domain layout exists:

| Directory | Purpose |
|---|---|
| `libs/` | core domain libraries |
| `adapters/` | technology adapters: HTTP, DB, XML, PDF, messaging, persistence |
| `apps/` | application entry points: CLI, server, worker |
| `features/` or `packs/` | pluggable feature packs or jurisdiction/domain packs |
| `testkit/` | shared fixtures, generators, test utilities |
| `build-logic/` | convention plugins |

Keep logical Gradle paths clear even if physical directories are grouped.

---

## 18. Agent output contract for Kotlin work

For non-trivial Kotlin changes, the summary must include:

- changed behavior,
- source of truth touched,
- validation or feedback added/used,
- blast radius considered,
- invariant preserved,
- verification run,
- Kotlin/Gradle/compiler flags affected,
- public or serialized contract impact,
- documentation or system theory preserved.

Do not provide only “updated the code.” Explain the engineering consequence proportionally to risk.

---

## 19. Pre-output checklist

Before yielding Kotlin code, verify:

**System theory**

- [ ] Source of truth identified.
- [ ] Feedback path identified or added.
- [ ] Blast radius considered.
- [ ] Invariant preserved.
- [ ] Theory preserved in tests/docs/code where appropriate.

**Semantics**

- [ ] Nullability, mutability, and failure states are visible in types.
- [ ] Domain alternatives are explicit where behavior differs.
- [ ] Public APIs avoid nullable parameters unless `null` is semantically precise.
- [ ] Sealed families are exhaustive where closed.
- [ ] Rich Errors syntax was not invented.

**Concurrency**

- [ ] Coroutine work is lifecycle-owned.
- [ ] Cancellation is preserved.
- [ ] Blocking work is isolated.
- [ ] Flow semantics are clear.

**API and compatibility**

- [ ] Visibility is intentional.
- [ ] Public return types are explicit where required.
- [ ] Java/Swift/JS/serialization contracts were considered where applicable.
- [ ] Annotation targets are deliberate.

**Build**

- [ ] Kotlin version, Gradle version, JDK/toolchain, and compiler plugins are compatible.
- [ ] No guessed dependencies or compiler flags.
- [ ] Shared build policy lives in convention plugins.
- [ ] No concurrent Gradle invocations were used.

**Testing**

- [ ] Smallest relevant verification was run first.
- [ ] Wider verification was run when contracts changed.
- [ ] Coroutine/time/concurrency tests are deterministic.
- [ ] Any skipped verification is stated honestly.

If any answer is “no” or “unclear,” refactor or surface the uncertainty before final output.
