# Python 3.13+ Agent Protocol

This protocol governs agent work on Python projects that target Python 3.13 or newer, or that use Python 3.13 as the lowest supported runtime.

Scope: libraries, services, CLIs, daemons, data pipelines, notebooks, web APIs, test suites, build scripts, code generators, Python-backed plugins, C/Rust extension packages, and mixed-language repositories with Python surfaces.

Primary objective: produce Python that is explicit, typed where it matters, verifiable, maintainable, secure at boundaries, concurrency-safe, packaging-correct, and aligned with the repository's actual compatibility contract.

Optimize in this order:

```text
correctness → invariants → explicit contracts → observability → packaging compatibility → maintainability → performance where measured → terseness
```

Terseness loses to clarity. Dynamic convenience loses to explicit system boundaries. A passing import is not the finish line. A green test suite is not enough if the change weakens state ownership, API contracts, or failure evidence.

This protocol inherits `.codex/UNIVERSAL_ENGINEERING_CONTRACT.md`. Do not duplicate the universal contract here; apply it before all Python-specific rules.

---

## 1. Repository intake before touching Python

Before editing Python, derive the repository's actual baseline.

Inspect the relevant subset of:

- `pyproject.toml`, `setup.cfg`, `setup.py`, `requirements*.txt`, `constraints*.txt`, lock files, and dependency-group definitions;
- `.python-version`, `.tool-versions`, `runtime.txt`, Dockerfiles, CI matrices, tox/nox sessions, and declared supported Python versions;
- package manager and build backend: uv, pip, Poetry, PDM, Hatch, setuptools, Flit, Bazel, Pants, or project-specific tooling;
- test framework: pytest, unittest, doctest, hypothesis, tox, nox, coverage, integration harnesses, fixtures, snapshots, golden files, and service emulators;
- type checker: pyright, basedpyright, mypy, pyre, pytype, or no checker;
- linter/formatter: Ruff, Black, isort, pylint, flake8, bandit, custom checks, pre-commit hooks;
- package layout: `src/` layout, namespace packages, editable installs, generated code, vendored code, data files, entry points, extras, plugins, and import boundaries;
- runtime surface: web framework, ORM, async runtime, scheduler, worker queue, CLI framework, notebook/runtime environment, external APIs, databases, caches, message brokers, and config sources;
- public API surface: imports, type stubs, protocols, entry points, CLI flags, HTTP routes, event schemas, model schemas, generated clients, and documented examples;
- C/Rust/foreign extension surfaces, ABI policy, free-threaded compatibility, wheels, platform tags, and build isolation;
- repository verification commands and the exact CI gates that define success.

Classify the touched Python surface before designing the change:

- **Published library:** supported Python versions, public imports, SemVer, type hints, stubs, docs, examples, and extras are contracts.
- **Internal library:** API evolution is easier, but invariants, import boundaries, and type contracts still matter.
- **Application/service:** configuration, persistence, migrations, logs, metrics, alerts, operational safety, and dependency pins are contracts.
- **CLI:** flags, arguments, environment variables, exit codes, stdout/stderr shape, config files, and shell completions are contracts.
- **Data/ML pipeline:** schema, reproducibility, randomness, artifacts, data freshness, lineage, and idempotency are contracts.
- **Build/codegen/test tooling:** determinism, generated output, local/CI parity, and developer ergonomics are contracts.
- **Extension package:** ABI, wheel tags, platform support, free-threaded behavior, and memory/thread safety are contracts.

Do not infer the baseline from a single file. In Python projects, compatibility truth is often split across packaging metadata, lock files, CI, docs, and release tooling.

---

## 2. Change loop in Python terms

For every non-trivial change, apply the Universal Engineering Contract concretely.

### 2.1 Minimum system map

Before editing, identify:

```text
Truth:
- Source of truth for the relevant state, config, schema, model, dependency, generated artifact, cache, migration, or runtime value:
- Mutation paths:
- Derived/cached/generated copies:

Evidence:
- Existing checks: unit/integration/property tests, type checks, lint, format, coverage, fixtures, logs, metrics, traces, CLI repros, notebooks, CI:
- Missing feedback worth adding:

Consequence:
- Direct Python dependencies: imports, callers, subclasses, protocols, entry points, tests, generated clients, stubs:
- Indirect dependencies: serialization, CLI output, HTTP contracts, database schema, queues, cron jobs, docs, dashboards, support workflows:

Invariant:
- Type, domain, data, idempotency, authorization, concurrency, compatibility, or operational rule that must remain true:

Preservation:
- Where the learned theory should live: type, test, docstring, module name, comment, migration note, docs, runbook, schema, config validation:
```

Keep the map lightweight for low-risk changes. Do not skip it for changes that touch state, public APIs, persistence, concurrency, packaging, security, or external contracts.

### 2.2 Red → Green → Refactor

For new behavior, start with the smallest failing proof:

- unit test;
- integration test;
- contract test;
- property test;
- regression fixture;
- CLI invocation;
- doctest;
- type-checking expectation;
- migration check;
- notebook smoke check;
- runtime reproduction from logs or issue evidence.

Then make the smallest coherent implementation and immediately refactor until the touched Python is easier to understand, easier to test, and harder to misuse.

### 2.3 Narrow-to-wide verification

Work in small increments:

1. make one coherent change;
2. run the narrowest useful check, such as the targeted pytest node, module import, type-check target, or CLI repro;
3. read the first real failure;
4. fix the root cause;
5. rerun the narrow check;
6. widen to repository-required checks before completion.

Do not accumulate speculative edits while verification is failing.

### 2.4 Root-cause fixes only

When verification fails:

- read the exact traceback, assertion diff, type-check message, linter diagnostic, or runtime log;
- identify whether the root is domain logic, type shape, import path, dependency version, fixture design, async scheduling, serialization, environment, permissions, or stale generated state;
- fix that cause;
- rerun the relevant check;
- preserve the failing proof if it guards a real regression.

Do not:

- catch broad exceptions to silence failures;
- loosen types to `Any` to appease a type checker;
- mutate fixtures or expected files without proving the new behavior is correct;
- skip tests because the environment is inconvenient;
- edit generated code without updating its generator or source of truth;
- claim completion while required checks still fail.

---

## 3. Python 3.13+ baseline posture

### 3.1 Runtime compatibility

Use the repository's declared interpreter policy. If the repository is governed by this protocol and no stronger local policy exists, assume Python 3.13+ as the baseline.

For new Python packages created under this protocol, prefer:

```toml
[project]
requires-python = ">=3.13"
```

For existing projects:

- do not raise `requires-python` without a concrete benefit and compatibility judgment;
- treat `requires-python`, CI Python matrices, lock files, Docker images, deployment runtimes, and docs as a single compatibility contract;
- do not use Python 3.14+ syntax or APIs in a Python 3.13-baseline project unless guarded, backported, or explicitly allowed;
- do not assume CPython-only behavior unless the repository declares CPython as part of the contract;
- if PyPy, GraalPy, embedded Python, iOS, Android, or WASI support matters, verify behavior on that target or preserve target-specific guards.

### 3.2 Python 3.13 capabilities

Use Python 3.13 capabilities when they make the system clearer and the repository baseline permits them.

Normal Python 3.13 tools include:

- PEP 695 generic type-parameter syntax from Python 3.12, when supported by the repository's type checker;
- PEP 696 defaults for type parameters when they reduce overload noise or make generic APIs clearer;
- `typing.ReadOnly` for read-only `TypedDict` items;
- `typing.TypeIs` for precise user-defined narrowing;
- `warnings.deprecated()` for deprecations that should be visible both at runtime and to type checkers;
- defined `locals()` / `frame.f_locals` behavior, especially for debuggers, tracers, REPL tools, and dynamic execution code;
- `copy.replace()` and `__replace__` where immutable update semantics are clearer than hand-written reconstruction;
- `queue.Queue.shutdown()` / `queue.ShutDown` when coordinating queue lifecycle explicitly;
- standard-library improvements such as the `dbm.sqlite3` backend when the project actually benefits from them.

Do not use new features merely for novelty. Prefer them when they reduce ambiguity, make invariants visible, or remove compatibility shims that no longer serve the baseline.

### 3.3 Experimental CPython features

Python 3.13 includes experimental implementation paths. Treat them as opt-in runtime targets, not assumptions.

#### Free-threaded CPython

Free-threaded CPython disables the GIL in a separate experimental build.

Rules:

- do not rely on the GIL as a correctness mechanism for mutable shared state;
- protect shared mutable state with explicit ownership, locks, queues, actors, immutable snapshots, atomics in extension code, or process boundaries;
- assume C extensions may re-enable or require the GIL unless they explicitly advertise free-threaded support;
- do not claim free-threaded compatibility unless tests run under the free-threaded executable or the repository has equivalent CI evidence;
- treat hidden global caches, singletons, lazy imports, monkeypatching, module-level mutation, and process-wide environment changes as concurrency risks.

#### Experimental JIT

The Python 3.13 JIT is experimental and disabled by default unless CPython is built/configured for it.

Rules:

- do not depend on JIT availability for correctness;
- do not promise performance improvements without measurement on the target interpreter and workload;
- do not micro-optimize around undocumented JIT internals;
- prefer algorithmic improvements, I/O reduction, data-shape fixes, and measured hot-path changes.

### 3.4 Removed Python 3.13 surfaces

Do not introduce dependencies on modules and APIs removed in Python 3.13.

Removed legacy standard-library modules include:

```text
aifc, audioop, cgi, cgitb, chunk, crypt, imghdr, mailcap, msilib, nis,
nntplib, ossaudiodev, pipes, sndhdr, spwd, sunau, telnetlib, uu, xdrlib
```

Also avoid `lib2to3`, the `2to3` tool, `tkinter.tix`, `locale.resetlocale()`, `typing.io`, `typing.re`, and chained `classmethod` descriptor patterns.

When migrating old code, remove the dependency, choose a maintained replacement, and add compatibility tests around the behavior that mattered. Do not vendor dead stdlib behavior by copying unreviewed code.

---

## 4. Hard boundaries

Violating these requires explicit repository policy or user authorization.

### 4.1 Correctness and contracts

- Never change public API shape without compatibility analysis.
- Never change persisted data format, migration ordering, serialization keys, CLI output, error codes, route semantics, or environment-variable names without tracing downstream consumers.
- Never duplicate canonical contract facts across code, docs, tests, generated clients, schemas, or examples.
- Never edit generated files without editing the generator or canonical source unless the repository explicitly stores generated outputs as the source of truth.
- Never weaken validation to make tests pass.
- Never replace a failing proof with a weaker assertion unless the old assertion was wrong and the new one proves the real invariant.

### 4.2 Type and dynamic-safety boundaries

- Never introduce `Any` as an escape hatch where a protocol, type variable, overload, `TypedDict`, dataclass, Pydantic model, or narrower type can express the contract.
- Never suppress type errors globally; suppress locally only with a reason tied to a real limitation.
- Never use `cast()` to lie. A cast must document a boundary where runtime evidence already proves the type.
- Never use mutable default arguments.
- Never return heterogeneous dictionaries as domain objects when a named type would make the contract clear.
- Never use stringly typed state where an enum, literal, dataclass, typed model, or value object is the real contract.
- Never rely on import-time side effects unless the repository intentionally uses plugin registration or framework discovery and tests cover it.

### 4.3 Error and security boundaries

- Never use bare `except:` or broad `except Exception:` unless re-raising, narrowing, or preserving cancellation/interrupt semantics is explicit.
- Never swallow `KeyboardInterrupt`, `SystemExit`, `asyncio.CancelledError`, or process termination signals accidentally.
- Never log secrets, credentials, tokens, private keys, passwords, session cookies, PII, or unredacted authorization headers.
- Never use unsafe deserialization, `eval`, `exec`, shell interpolation, path traversal, or SSRF-prone URL handling without a narrowly justified, validated boundary.
- Never pass user-controlled strings to `subprocess` with `shell=True` unless the shell itself is the explicit product surface and inputs are safely quoted/validated.
- Never make network, filesystem, database, or process side effects at import time unless the project has a deliberate plugin/bootstrap pattern.

### 4.4 Async and concurrency boundaries

- Never create orphan tasks without ownership, cancellation, and failure observation.
- Never call blocking I/O inside an event loop without moving it to an executor or using an async-native API.
- Never ignore backpressure in queues, streams, workers, or message consumers.
- Never mutate process environment after concurrent work starts unless the operation is serialized and isolated.
- Never use global mutable caches without invalidation, capacity, thread-safety, and test evidence.
- Never rely on CPython's GIL for logical thread safety in code that may run on Python 3.13 free-threaded builds or alternative interpreters.

### 4.5 Build and dependency boundaries

- Never install packages globally for repository work.
- Never change dependency constraints or lock files without understanding direct, transitive, security, and deployment impact.
- Never add a dependency when a small local function or existing dependency is enough.
- Never vendor code without license, update, and security implications.
- Never mix package managers casually. Preserve the repository's canonical tool.
- Never claim a package is compatible with Python 3.13 unless tests/imports/builds verify the relevant dependency set.

---

## 5. Types, domain modeling, and API design

### 5.1 Prefer named domain shapes

Choose constructs that express the domain:

| Need | Preferred Python construct |
|---|---|
| Immutable data value | `@dataclass(frozen=True, slots=True)` or `NamedTuple` where appropriate |
| Mutable internal record | `@dataclass(slots=True)` with controlled mutation |
| External validated model | Repository-standard schema/model tool, such as Pydantic, attrs, dataclass, Marshmallow, or framework model |
| Closed symbolic states | `Enum` / `StrEnum` / `Literal` depending on runtime needs |
| Structural capability | `typing.Protocol` |
| Mapping with fixed keys | `TypedDict`, using `Required`, `NotRequired`, and `ReadOnly` when useful |
| API narrowing helper | `TypeIs` when both true and false branches narrow correctly; `TypeGuard` only when its semantics are intended |
| Simple result pair | named dataclass or tuple only when positional meaning is obvious |
| Distinct domain identity | small value object, validated newtype-like wrapper, or repository-standard model |

Do not create a type merely to look enterprise. Every type must prevent misuse, name a domain concept, isolate a boundary, or make evolution safer.

### 5.2 Type hints are contracts, not decoration

Use type hints to communicate real API contracts.

Rules:

- prefer precise collection and callable types from `collections.abc` for parameters;
- prefer concrete return types where callers depend on behavior;
- use `Self`, `Protocol`, `TypeVar`, `ParamSpec`, `TypeVarTuple`, overloads, and type aliases when they remove ambiguity;
- use `Literal` for small protocol strings only when the set is stable and public;
- use `Final` and `ClassVar` where mutation semantics matter;
- use `ReadOnly` for `TypedDict` items that callers must not mutate;
- keep annotations import-safe under the repository's chosen annotation policy;
- avoid runtime type introspection on annotations without understanding postponed annotation behavior and `typing.get_type_hints()` consequences.

Avoid:

- annotating everything as `dict`, `list`, `Callable`, or `Any` when the shape matters;
- type aliases that hide complexity without naming a domain concept;
- overloads where a small object model or enum would be clearer;
- casts that mask incorrect validation or parsing.

### 5.3 Public API evolution

For public packages:

- preserve import paths unless the change is a deliberate deprecation or major-version break;
- add deprecation warnings through the repository's established mechanism, using `warnings.deprecated()` where appropriate;
- keep type stubs, `py.typed`, docs, examples, and runtime behavior synchronized;
- test public imports and representative type-checking examples;
- update changelog or migration notes when user behavior changes.

For internal packages:

- prefer cohesive refactors over compatibility shims that no real caller needs;
- delete dead wrappers once callers are migrated;
- keep import boundaries clean so internal convenience does not leak into public contracts.

### 5.4 Dynamic behavior needs stronger evidence

Python allows dynamic patterns. Use them only when they earn their keep.

Dynamic dispatch, monkeypatching, runtime imports, metaclasses, descriptors, decorators, `__getattr__`, `__getattribute__`, module-level plugin discovery, and reflection require:

- a clear owner of the registry or dynamic state;
- tests for registration, lookup, error messages, and duplicate/missing cases;
- type stubs or protocols where static tools cannot infer the contract;
- documentation when public users must participate in the pattern.

Do not use dynamic machinery to avoid naming the domain model.

---

## 6. State, configuration, and truth ownership

### 6.1 State must have one owner

Before changing stateful code, identify the authority:

- database row or transaction;
- migration or schema file;
- environment variable or config file;
- CLI argument or parsed settings object;
- cache, memoized value, singleton, or lazy-loaded object;
- queue message, event, task state, or job record;
- external API, webhook, or generated client;
- notebook cell state or pipeline artifact;
- package metadata or lock file.

Rules:

- centralize parsing and validation at the boundary;
- represent validated configuration as a named object rather than repeatedly reading `os.environ`;
- pass dependencies explicitly where practical;
- isolate global state behind a small owner with reset hooks for tests;
- keep derived state either recomputable or explicitly invalidated;
- make idempotency and transaction boundaries visible.

### 6.2 Imports are a dependency graph

Python imports execute code. Treat imports as design, not plumbing.

Rules:

- avoid import cycles by improving module boundaries, not by adding random local imports;
- use local imports only for measured startup cost, optional dependencies, cycle-breaking with rationale, or plugin loading;
- keep `__init__.py` exports deliberate and tested;
- preserve package data and resources using `importlib.resources` rather than filesystem assumptions;
- do not shadow standard-library or dependency module names;
- avoid side effects at import time except deliberate registration patterns.

### 6.3 Configuration is a contract

Configuration facts must have one canonical owner.

Rules:

- prefer typed settings objects over scattered environment reads;
- validate config at startup with actionable errors;
- keep defaults in one place;
- ensure docs, examples, deployment manifests, and tests derive from or match the canonical config;
- keep secret values out of code, logs, tests, and docs;
- test missing, malformed, defaulted, and override cases.

---

## 7. Errors, failures, and observability

### 7.1 Model recoverable outcomes deliberately

Use exceptions for exceptional control transfer and integration boundaries. Use named result objects or sealed-like domain models when callers must distinguish ordinary business outcomes.

Examples of ordinary outcomes that should not be hidden in a generic exception:

- validation failure;
- authorization denial;
- duplicate record;
- cache miss;
- parse ambiguity;
- idempotent no-op;
- unavailable optional feature.

When using exceptions:

- choose the narrowest meaningful exception type;
- include enough context to debug without leaking secrets;
- preserve cause chains with `raise ... from ...`;
- do not erase cancellation, timeout, or interrupt signals;
- use `ExceptionGroup` / `except*` where concurrent failures must be preserved.

### 7.2 Logs and metrics are feedback surfaces

Use structured, actionable observability.

Rules:

- log at the boundary where context exists;
- include stable correlation identifiers where available;
- avoid duplicate noisy logs at every stack layer;
- make failure messages useful to operators and users;
- do not log secrets or sensitive payloads;
- add metrics/traces where tests cannot prove runtime health;
- keep CLI stdout for machine/user output and stderr for diagnostics.

### 7.3 User-facing errors are contract surfaces

For CLIs, APIs, SDKs, and libraries:

- test error messages when users or tools depend on them;
- keep exit codes stable;
- preserve HTTP status semantics and response schemas;
- avoid exposing internal traceback details across service boundaries;
- document new failure modes when they affect users.

---

## 8. Concurrency, async, and scheduling

### 8.1 `asyncio` and structured concurrency

Prefer structured ownership of async work.

Rules:

- use `asyncio.TaskGroup` or repository-standard structured-concurrency tools for related tasks;
- keep task ownership, cancellation, timeout, and error aggregation explicit;
- propagate `CancelledError` unless deliberately translating it at a boundary;
- use `asyncio.timeout()` or repository-standard timeout policy for bounded work;
- do not block the event loop with synchronous file, network, database, subprocess, CPU, or sleep calls;
- isolate sync/async boundaries with clear adapters;
- test cancellation, timeout, and partial-failure cases for non-trivial async code.

### 8.2 Threads and processes

Use threads for blocking I/O and integration with thread-safe libraries. Use processes for CPU-bound work unless free-threaded compatibility and measurement justify threads.

Rules:

- guard shared mutable state explicitly;
- keep executor lifetime owned and shut down;
- pass immutable or serialized data across process boundaries;
- design worker shutdown and queue draining deliberately;
- test race-prone logic with deterministic synchronization where possible;
- do not mutate module globals from multiple threads without a lock or owner.

### 8.3 Background jobs and queues

For workers, schedulers, and queues:

- define idempotency keys and retry semantics;
- record durable job state where work must survive process death;
- distinguish queued, running, succeeded, failed, cancelled, and retried states;
- preserve backpressure;
- make poison-message handling explicit;
- test shutdown and restart behavior.

---

## 9. Packaging, environments, and dependencies

### 9.1 `pyproject.toml` is a design surface

`pyproject.toml` communicates build backend, project metadata, dependency groups, Python compatibility, entry points, tool configuration, and packaging behavior.

Rules:

- do not guess build-backend syntax;
- preserve the repository's package manager and lock-file semantics;
- keep package metadata, import package name, docs, and distribution name aligned;
- define console scripts through project entry points rather than ad hoc shell wrappers;
- include package data deliberately;
- ensure `py.typed` is present for typed published packages where appropriate;
- do not move tool configuration without checking whether the tool reads the new location.

### 9.2 Dependencies and lock files

Dependency changes are system changes.

Before adding or changing dependencies, check:

- direct need and alternatives;
- transitive dependency and license impact;
- Python 3.13 wheel availability;
- C extension and platform compatibility;
- free-threaded compatibility when relevant;
- security advisories;
- lock-file updates and deployment reproducibility;
- CI and Docker image implications.

Prefer narrow, explicit dependencies. Do not add a dependency merely for a small function unless the dependency already exists or the capability is non-trivial and maintained.

### 9.3 Environments

Rules:

- use the repository's environment tool;
- never install into global Python for project work;
- prefer `python -m <tool>` when it avoids PATH ambiguity;
- ensure local commands use the same interpreter and dependency group as CI;
- do not mix venvs, pyenv, uv, conda, Poetry, PDM, tox, and system Python without establishing which one is canonical;
- record new required environment variables, services, and system packages in the appropriate setup docs or runbook.

### 9.4 Wheels, extensions, and ABI

For packages with native extensions:

- prefer the stable ABI / `abi3` only when the extension's API usage truly fits it;
- test source builds and wheels for supported platforms;
- keep build isolation correct;
- pin or declare build dependencies in `pyproject.toml`;
- account for Python 3.13 free-threaded builds only with explicit evidence;
- avoid private CPython C APIs unless the repository accepts version-specific breakage.

---

## 10. Testing and verification

### 10.1 Test the contract, not implementation trivia

Good Python tests prove behavior, boundaries, and regressions.

Prefer tests that cover:

- public API behavior;
- domain invariants;
- edge cases and invalid inputs;
- serialization/deserialization round trips;
- CLI outputs and exit codes;
- config parsing and defaults;
- database migrations and rollback-sensitive paths;
- async cancellation and timeout behavior;
- dependency-injection boundaries;
- import/export compatibility;
- type-checking examples for library APIs.

Avoid tests that only assert private call order unless the private order is itself the contract.

### 10.2 Pytest posture

When pytest is used:

- keep fixtures explicit, narrow, and named by domain role;
- avoid autouse fixtures unless they protect a global invariant;
- use `tmp_path`, monkeypatching, and dependency injection to isolate state;
- mark slow, integration, network, or flaky tests according to repository policy;
- do not hide real flakes by broad retries; identify race, time, I/O, or ordering causes;
- use parametrization to clarify behavior matrices without obscuring failures.

### 10.3 Property, fuzz, and snapshot tests

Use stronger test forms when examples are insufficient.

- Use property tests for parsers, serializers, normalizers, validators, and state machines.
- Use fuzzing for untrusted input boundaries where the repository supports it.
- Use snapshots/golden files only when the output is a real contract. Review diffs manually.
- Keep fixtures minimal and meaningful.

### 10.4 Type checking as verification

For typed Python code:

- run the repository's type checker on the narrowest useful target first;
- do not weaken annotations to make checks pass;
- add type tests or examples for generic public APIs;
- keep `py.typed` and stubs synchronized;
- treat type-checker differences as tool contracts, not as runtime truth.

### 10.5 Required verification summary

For non-trivial work, report:

```text
Verification:
- Narrow checks run:
- Full or CI-equivalent checks run:
- Checks not run and why:
- Runtime/manual evidence:
```

Do not claim a check passed unless it actually ran and passed.

---

## 11. Refactoring Python safely

### 11.1 Boy Scout + Mikado

When touching Python, leave the touched surface better:

- improve names;
- extract coherent functions/classes;
- remove dead branches;
- collapse needless indirection;
- reduce import cycles;
- tighten types;
- delete stale compatibility shims;
- replace stringly contracts with named types;
- make validation central and explicit;
- improve test coverage around real behavior.

Use Mikado sequencing for broader refactors:

1. identify target design;
2. make the smallest safe prerequisite change;
3. verify;
4. repeat;
5. stop when the next improvement is a separate slice.

Do not perform broad rewrites without executable evidence and a rollback path.

### 11.2 Deleting code

Before deleting Python code, trace the blast radius:

- static imports and references;
- dynamic imports and plugin registrations;
- entry points and console scripts;
- framework discovery patterns;
- test fixtures and monkeypatch targets;
- docs, examples, generated clients, and stubs;
- serialized names, pickled paths, migration references, and config keys;
- external user imports and SemVer commitments.

Deletion is safe only when the contract is gone, deprecated, or migrated and evidence proves no live dependency remains.

### 11.3 Generated code and migrations

Generated outputs and migrations require source-of-truth discipline.

Rules:

- update the generator, schema, or template first;
- regenerate outputs using the canonical command;
- inspect generated diffs for unexpected drift;
- preserve migration ordering;
- do not edit database migrations casually after release;
- test upgrade paths and data invariants where possible.

---

## 12. Framework and boundary posture

### 12.1 Web services and APIs

For FastAPI, Django, Flask, Starlette, aiohttp, or other frameworks:

- identify the canonical route/schema/dependency owner;
- keep request validation, authorization, business logic, and persistence boundaries distinct;
- avoid framework globals leaking into pure domain code;
- test auth, validation, error response, and serialization contracts;
- keep OpenAPI/schema/docs synchronized with runtime behavior;
- preserve middleware ordering and dependency-injection semantics.

### 12.2 Databases and ORMs

For SQLAlchemy, Django ORM, async ORMs, migrations, or raw SQL:

- identify schema truth: migration, model, generated schema, or database introspection;
- use transactions deliberately;
- avoid N+1 regressions;
- handle isolation, locking, and retries explicitly;
- test migrations and representative queries;
- do not change cascade, nullability, uniqueness, or index semantics without blast-radius analysis.

### 12.3 CLIs

For CLIs:

- treat flags, env vars, config files, output, and exit codes as public contracts;
- keep human-readable and machine-readable output distinct;
- avoid logging to stdout when stdout is data;
- test help text only where it is a maintained contract;
- preserve shell completion and packaging entry points.

### 12.4 Data science, notebooks, and ML

For notebooks and pipelines:

- separate exploratory notebooks from production logic;
- move reusable logic into importable modules with tests;
- make randomness, data versions, feature definitions, and artifact paths explicit;
- preserve data lineage and reproducibility;
- avoid hidden state between notebook cells;
- test productionized transformations outside the notebook.

---

## 13. Documentation and Python examples

Use `.codex/PROTOCOL_AFAD.md` for agent-maintained documentation when public contracts, guides, runbooks, or code/document synchronization are involved, except for the repository root `README.md` exception defined in `AGENTS.md`.

Python-specific documentation rules:

- keep examples runnable against Python 3.13 unless explicitly marked otherwise;
- keep imports, entry points, package names, and type signatures synchronized with code;
- include version guards when behavior differs by Python version;
- prefer small complete examples over fragments that hide setup;
- document deprecations, migration paths, and public failure modes;
- use docstrings for local API semantics and AFAD-managed docs for broader contract theory.

Root `README.md` remains a storefront. Keep it human-first and link to detailed docs rather than turning it into a reference database.

---

## 14. Agent output checklist

For non-trivial Python work, final output should include the relevant subset:

```text
Python baseline:
- Interpreter/package baseline confirmed:
- Packaging/build tool used:

System map:
- Truth owner:
- Evidence added/used:
- Blast radius checked:
- Invariant preserved:
- Theory preserved in:

Change summary:
- Files changed:
- Public API/config/schema/CLI behavior changed:
- Dependencies or lock files changed:

Verification:
- Narrow checks:
- Full checks:
- Checks not run:

Risk:
- Remaining compatibility, concurrency, packaging, or operational risk:
```

Keep summaries proportional. Do not produce ceremony for a typo. Do not omit risk for changes that affect public contracts, persistence, packaging, concurrency, or security.
