# SQLite3 Multiple Ciphers 2.3.3 / SQLite 3.53.0 Agent Protocol

**Version:** 2.0.0
**Updated:** 2026-04-27
**Inherits:** [.codex/UNIVERSAL_ENGINEERING_CONTRACT.md](./UNIVERSAL_ENGINEERING_CONTRACT.md) v2.0.0+
**Scope:** projects that build, vendor, link, wrap, configure, distribute, test, or operate **SQLite3 Multiple Ciphers 2.3.3**, based on **SQLite 3.53.0**. Includes C and C++ integrations, amalgamation builds, static or shared library packaging, embedded applications, CLIs, services, language bindings, JNI/JNA, Python/Rust/Node/.NET/Java/Kotlin wrappers, SQL migrations, encrypted database files, PRAGMA/URI configuration, key and rekey flows, backups, WAL/journal behavior, build flags, and cross-platform distribution.

## 0. Scope and inheritance

This protocol inherits the Universal Engineering Contract. The universal contract defines the meta-questions every change must answer — Truth, Evidence, Consequence, Invariant, Justification, Re-cueing — and frames the agent as a *transient theory-holder*. Apply the universal contract before any rule below; do not restate it here. When SQLite3MC is used from Java, Kotlin, Python, Rust, C, C++, or another runtime, apply this protocol in addition to the relevant language protocol.

This protocol adds SQLite3MC- and SQLite-specific content for which the universal contract is intentionally silent: cipher and key lifecycle, file-format state, native-library identity across compile and runtime, SQL/SQLite version compatibility, FFI safety, and the at-rest encryption boundary.

**Primary objective:** preserve data integrity, encryption correctness, key safety, SQLite compatibility, build reproducibility, and clear ownership of database/file-format contracts.

**Optimization order:**

```text
data integrity → key safety → cipher/file-format compatibility → source-of-truth clarity → portability → observability without leakage → performance where measured → terseness
```

Convenience loses to data safety. Local build success loses to runtime link correctness. Encryption that is not tested as encryption is not verified. A wrapper API that hides key ownership, cipher selection, or migration behavior is not finished.

### 0.1 SQLite3MC + SQLite 3.53 tacit gaps

Per the Naurian frame, some theory the agent typically does not bring in cold and must surface rather than paper over. Watch especially for:

- Whether the headers at compile time, the static or shared library linked at build time, and the dynamic library actually loaded at runtime are the same SQLite3MC version. A single file will not answer this; the agent must verify across phases.
- Whether the application actually loads SQLite3MC at runtime, or quietly resolves to a system SQLite. "Drop-in replacement" is a code property, not a runtime guarantee.
- Whether encrypted-database test fixtures reflect production cipher, KDF, page size, and reserve-byte settings — or were created with default settings and so prove nothing about the deployed format.
- Whether keys ever appear in URIs, `ATTACH ... KEY` statements, `PRAGMA key`/`rekey`, debug captures, query logs, crash reports, shell history, or process listings. Every one of these is a real production leak class.
- Whether `TEMP` tables, in-memory databases, or bytes 16–23 of the database file are inside or outside the threat model. The encryption boundary is non-obvious and easy to assume away.
- Whether old SQLCipher, sqleet, or SQLite Encryption Extension conventions still inform the codebase. SQLite3MC is API-compatible in many places but is not identical, and copy-pasted SQLCipher recipes can silently drift.
- Whether SQLite 3.52.0 (withdrawn upstream) is still pinned anywhere as a fallback baseline.
- Whether the secure cipher-state nullification path that distinguishes SQLite3MC 2.3.3 from older releases is still intact. It looks redundant; removing it is a security regression.

Where the answer is not derivable from code, history, or conversation, surface the gap explicitly; do not assume the convenient answer.

---

## 1. Repository intake before touching SQLite3MC surfaces

Before editing anything related to SQLite3 Multiple Ciphers, determine the repository's actual integration model.

Inspect the relevant subset of:

- vendored source files, especially `sqlite3mc_amalgamation.c`, `sqlite3mc_amalgamation.h`, `sqlite3.c`, `sqlite3.h`, `sqlite3ext.h`, `sqlite3mc.h`, patches, generated amalgamation scripts, and third-party manifests;
- version pins, release tags, commit hashes, checksums, package metadata, lock files, SBOM entries, release notes, and any source-ID assertions;
- build systems: Autotools, CMake, Premake, GNU Make, MSBuild/Visual Studio, Meson, Bazel, Gradle, Cargo build scripts, Python extension builds, npm native builds, or project-specific wrappers;
- compiler and linker flags, `SQLITE_*` options, `SQLITE3MC_*` options, default cipher configuration, legacy cipher flags, ICU/ZLIB/MINIZ configuration, and platform-specific defines;
- whether the repository links the SQLite3MC library directly, replaces a vanilla SQLite library, uses the amalgamation, or consumes a language binding that bundles SQLite3MC;
- runtime library resolution: static vs dynamic linking, DLL/shared-object search paths, rpath/install-name, package manager behavior, container images, Android/iOS/WASM targets, and CI artifacts;
- key paths: where passphrases, raw keys, KMS handles, secrets, environment variables, config values, user prompts, hardware-backed secrets, or test keys enter the system;
- SQL and API usage: `sqlite3_key`, `sqlite3_key_v2`, `sqlite3_rekey`, `sqlite3_rekey_v2`, `sqlite3mc_*` APIs, `PRAGMA key`, `PRAGMA rekey`, URI parameters, `ATTACH ... KEY`, backup APIs, and language-binding equivalents;
- database lifecycle: initial creation, open, authentication/keying, migration, attach/detach, backup, restore, VACUUM, WAL checkpointing, rekeying, decryption, compaction, corruption handling, and deletion;
- file-format assumptions: cipher scheme, page size, reserve bytes, plaintext header policy, KDF settings, legacy compatibility, `user_version`, schema migrations, and database compatibility fixtures;
- journaling and temp behavior: rollback journal, WAL, shared memory files, temporary tables, in-memory databases, temp-store configuration, and file-permission policy;
- tests and evidence: encrypted fixture files, wrong-key tests, rekey tests, migration tests, cross-platform CI, sanitizer runs, Valgrind, fuzzers, SQL logic tests, and production observability;
- the universal contract's six concerns (truth, evidence, consequence, invariant, justification, re-cueing) for the touched surface.

Classify the touched surface before designing the change:

- **Vendored native dependency:** version, patches, compile options, and source provenance are contracts.
- **Application database:** file format, key lifecycle, migrations, backups, durability, and restore behavior are contracts.
- **Published binding/package:** ABI/API, binary compatibility, platform wheels/artifacts, runtime linking, docs, examples, and package metadata are contracts.
- **Internal wrapper:** key ownership, error handling, connection lifecycle, and safe defaults are contracts.
- **CLI/tooling:** command-line flags, stdout/stderr shape, exit codes, script compatibility, secret handling, and non-interactive behavior are contracts.
- **Embedded/mobile/WASM build:** target support, compile flags, filesystem/VFS behavior, entropy source, memory constraints, and package size are contracts.

Do not infer SQLite3MC behavior from ordinary SQLite alone. SQLite3MC is intentionally compatible with SQLite APIs, but encryption, VFS behavior, keying, and file-format state add contracts that ordinary SQLite does not have.

---

## 2. Change loop in SQLite3MC terms

### 2.1 Minimum system map

For every non-trivial SQLite3MC change, apply the universal contract §1 system map (Truth / Evidence / Consequence / Invariant / Justification / Re-cueing) to the touched surface. SQLite3MC-specific anchors for each concern:

- **Truth:** canonical owner of SQLite3MC version, SQLite source version, compile options, default cipher, legacy flags, binding/runtime package version; canonical owner of key material and key lifecycle; canonical owner of database schema, migrations, cipher configuration, page format, and fixtures; derived/generated copies (amalgamation, headers, wrappers, package metadata, docs, CI images, lock files).
- **Evidence:** native build correctness, runtime link correctness, encryption roundtrip, wrong-key failure, rekey, migration, backup/restore, language binding behavior; missing feedback worth adding.
- **Consequence:** direct (callers, wrappers, SQL scripts, migrations, bindings, tests, packaging, CLI tools, deployment images); indirect (stored database files, backups, restore tools, support workflows, monitoring, user data, compliance, release process).
- **Invariant:** data, encryption, file-format, key-safety, ABI/API, migration, or compatibility rule that must remain true.
- **Justification:** why each cipher / page-size / KDF / legacy-mode choice is the way it is, and which are inherited rather than deliberately chosen. If the answer is not available, surface that gap.
- **Re-cueing:** where the learned theory belongs — build manifest, test fixture, migration note, wrapper API, safety comment, runbook, AFAD-managed doc, release checklist, CI assertion. Flag the parts of the theory that cannot be written down, and who currently holds them.

Keep this lightweight for low-risk edits. Do not skip it for changes that affect encryption, persisted files, build flags, runtime linking, migrations, or key handling.

### 2.2 Red → Green → Refactor

Per universal contract §2. SQLite3MC-typical "smallest failing proofs":

- encrypted open/read/write roundtrip;
- wrong-key rejection;
- rekey or decrypt migration fixture;
- cross-version or legacy-cipher fixture;
- SQL migration test;
- native build/link test;
- language-binding integration test;
- WAL/journal/backup/restore test;
- sanitizer or memory-leak reproduction;
- CLI invocation with deterministic output mode;
- file header or plaintext-leak check.

Then make the smallest coherent implementation and immediately refactor until the touched surface has clearer ownership, fewer hidden states, and better verification.

### 2.3 Narrow-to-wide verification

Per universal contract §2 and §7 (Feedback must match risk). For SQLite3MC, widening usually means verifying both compile-time and runtime facts: the code compiled against the intended headers and also loaded the intended library at runtime. The two are independent; a green compile-time check does not prove the runtime answer.

### 2.4 Root-cause fixes only

Per universal contract §0 ("the agent must not paper over what it does not have") and §2 (read the actual failure). When verification fails, distinguish among SQLite3MC-specific root causes:

- key timing,
- wrong cipher configuration,
- stale generated source,
- mixed headers/library,
- runtime library shadowing,
- unsupported SQL,
- file permissions,
- WAL/journal mode,
- platform target,
- actual corruption.

Do not:

- swallow SQLite errors or collapse them into vague application errors;
- log passphrases, raw keys, key-bearing URIs, PRAGMA statements containing secrets, or decrypted data;
- downgrade to vanilla SQLite accidentally;
- mix SQLite headers from one version with a different runtime library;
- regenerate or edit amalgamation artifacts without updating the canonical generation path;
- change cipher defaults, page sizes, reserve bytes, KDF parameters, or legacy flags without a migration and fixture evidence;
- claim encryption correctness without a wrong-key failure test and a plaintext-leak check.

---

## 3. Baseline posture: SQLite3MC 2.3.3 and SQLite 3.53.0

### 3.1 Version baseline

For repositories governed by this protocol, assume:

```text
SQLite3 Multiple Ciphers: 2.3.3
Underlying SQLite:        3.53.0
```

Use the repository's pinned version when it is more specific. Do not upgrade or downgrade SQLite3MC without a compatibility judgment, migration-risk assessment, and verification plan.

SQLite3MC 2.3.3 includes the upstream SQLite 3.53.0 baseline and fixes secure nullification of cipher data structures on freeing. Treat any edit around cipher state cleanup as security-sensitive. Do not remove zeroization, nullification, or cleanup paths because they look redundant — this is exactly the kind of code where Naur's "amorphous additions" warning bites in reverse.

SQLite 3.53.0 includes a fix for the WAL-reset database corruption bug. Do not downgrade to a pre-fix SQLite baseline without explicitly accepting the risk and recording the justification (per universal contract §1.5).

### 3.2 SQLite 3.53.0 feature posture

Use SQLite 3.53.0 capabilities only when the deployed runtime is guaranteed to be SQLite3MC 2.3.3 / SQLite 3.53.0 or newer.

Notable 3.53.0 behavior for agents:

- `ALTER TABLE` can add and remove `NOT NULL` and `CHECK` constraints. Use this only when migration compatibility is acceptable.
- `REINDEX EXPRESSIONS` can rebuild expression indexes. Prefer it when repairing stale expression-index state rather than inventing application-level workarounds.
- `json_array_insert()` and `jsonb_array_insert()` are available in the 3.53.0 baseline.
- The CLI output defaults changed for interactive sessions through QRF. Tests and scripts must set explicit output modes instead of relying on human-oriented defaults.
- Bare semicolons at the end of dot-commands are silently ignored. Treat CLI script compatibility deliberately.
- New C interfaces such as `sqlite3_str_truncate()`, `sqlite3_str_free()`, `sqlite3_carray_bind_v2()`, `SQLITE_PREPARE_FROM_DDL`, `SQLITE_UTF8_ZT`, `SQLITE_LIMIT_PARSER_DEPTH`, and `SQLITE_DBCONFIG_FP_DIGITS` are available only when the runtime really is 3.53.0+.
- Floating-point text conversion behavior changed to round by default to 17 significant digits instead of the previous 15. Review golden outputs, text dumps, hash inputs, and deterministic serialization tests.
- The self-healing index feature may address stale expression index issues, but it does not replace tests for migration and query correctness.

Do not write code or migrations that silently require 3.53.0 if production, tests, system packages, or bundled artifacts may still load an older SQLite.

### 3.3 SQLite 3.52 warning

SQLite 3.52.0 was withdrawn upstream. Do not select SQLite3MC 2.3.0 / SQLite 3.52.0 as a fallback baseline. If a repository already contains that version (see §0.1), surface the issue and prefer moving to SQLite3MC 2.3.3 or a project-approved fixed baseline.

---

## 4. Canonical ownership and provenance

### 4.1 One owner for version and build facts

Per universal contract §5 (canonical ownership of contract facts). For SQLite3MC, the contract facts that need a single owner include: SQLite3MC version, SQLite source version, release tag, commit hash, checksums, compile flags, enabled extensions, default cipher, legacy options, and platform artifact versions.

Acceptable owners include:

- a third-party dependency manifest;
- a vendoring manifest;
- a build-system version catalog;
- a lock file plus package metadata;
- a dedicated `third_party/sqlite3mc/README` or manifest;
- a generated-source script with checksum assertions.

Do not hard-code the SQLite3MC version, SQLite source ID, compile options, or cipher defaults independently across build scripts, docs, wrappers, and tests. Derive, generate, or validate secondary surfaces from the canonical owner.

### 4.2 Provenance checks

When adding or updating SQLite3MC:

- use an authoritative upstream release, source archive, package, or repository tag;
- record the SQLite3MC version and underlying SQLite version;
- verify checksums or signed provenance when the repository supports it;
- preserve local patches as small, named, reviewable patches;
- update package metadata, lock files, SBOM, docs, and CI images together;
- run fixture tests against existing encrypted databases before release.

If the repository uses prebuilt binaries, verify that binary provenance and compile options are inspectable. Opaque binaries are a supply-chain and compatibility risk.

### 4.3 Header/library/runtime coherence

The following must agree unless the repository has an explicit compatibility shim:

- headers used at compile time;
- static or shared library linked at build time;
- dynamic library loaded at runtime;
- package metadata;
- `sqlite3_libversion()` and `sqlite3_sourceid()` observations;
- compile-option observations such as `PRAGMA compile_options` or `sqlite3_compileoption_get()`;
- language-binding reported versions.

A common failure mode — and the headline tacit gap from §0.1 — is compiling against the intended SQLite3MC headers while loading a system SQLite library at runtime. Always verify runtime identity when touching packaging, dynamic linking, containers, or language bindings.

---

## 5. Build, linking, and packaging discipline

### 5.1 Do not accidentally link vanilla SQLite

SQLite3MC can be used as a drop-in replacement for SQLite in some build layouts, but replacement is not proof of correct encryption behavior.

Agents must check:

- the actual library file packaged into the application;
- symbol resolution order;
- DLL/shared-object search path;
- rpath/install-name settings;
- static-link symbol conflicts;
- transitive dependencies that also bundle SQLite;
- package manager postinstall behavior;
- runtime version reports.

If both vanilla SQLite and SQLite3MC appear in the same process, trace which consumers bind to which symbols. Avoid duplicate SQLite global state unless the project deliberately isolates it.

### 5.2 Amalgamation discipline

When using the amalgamation:

- treat the generated amalgamation as derived unless the repository explicitly vendors it as the source of truth;
- do not manually edit generated amalgamation code except for clearly named, documented emergency patches;
- keep headers, source, generated files, build flags, and docs in sync;
- preserve a reproducible regeneration path;
- validate the resulting source ID, version, and compile options.

If the repository replaces `sqlite3.c` with `sqlite3mc_amalgamation.c`, ensure every consumer that expects encryption is compiled and linked against the replacement, not a system SQLite artifact.

### 5.3 Compile-time options

Compile-time options are contract facts. Changing them can alter SQL availability, file behavior, performance, compatibility, and security posture.

For encrypted databases, pay special attention to:

- `SQLITE_TEMP_STORE`;
- `SQLITE_SECURE_DELETE`;
- `SQLITE_USE_URI`;
- enabled extensions such as FTS, JSON, RTREE, GEOPOLY, CARRAY, CSV, SHA3, UUID, FILEIO, REGEXP, SERIES, user authentication, and optional ZLIB-backed extensions;
- default cipher `CODEC_TYPE`;
- legacy compatibility flags such as sqleet or SQLCipher legacy modes;
- platform-specific flags for WASM, Android, Windows, or cross-compilation.

Changing compile options requires tests and documentation because runtime SQL behavior and file handling may change even when application source code does not.

### 5.4 Platform-specific builds

For Windows, verify architecture naming, CRT expectations, DLL placement, `.lib` import libraries, Visual Studio/MSBuild files, and MinGW/GNU Make variants.

For Linux and macOS, verify Autotools/CMake or project-specific build output, install names, rpath, shared-library versioning, pkg-config files, and container images.

For Android/iOS/mobile, verify ABI splits, bundled native libraries, filesystem behavior, entropy, backup behavior, and secure storage for keys.

For WebAssembly, verify VFS behavior, exported C APIs, memory model, JS glue, OPFS or browser storage behavior, and whether encryption keys cross JS/WASM boundaries safely.

For language bindings, verify both the native artifact and the high-level package. The package version alone is insufficient evidence.

---

## 6. Encryption, ciphers, and key lifecycle

### 6.1 Key ownership

Key material must have one explicit owner and lifecycle.

Identify:

- where the key/passphrase originates;
- who can create, rotate, recover, or revoke it;
- how it is transported into SQLite3MC;
- whether it is a passphrase, raw key, KMS-derived secret, user credential, device secret, or test fixture;
- where it is stored, cached, zeroized, redacted, and destroyed;
- what happens on wrong key, missing key, expired key, or partial rekey failure.

Do not hard-code production keys. Do not commit real encrypted database keys. Do not add default passphrases for convenience. Test keys must be visibly test-only and isolated from production configuration.

### 6.2 Prefer safe API boundaries

Prefer a wrapper API that applies the key immediately after opening a connection and before any schema reads, migrations, PRAGMAs, or application queries.

C API posture:

- `sqlite3_key()` and `sqlite3_key_v2()` set a database key and should normally be called immediately after `sqlite3_open()` / `sqlite3_open_v2()`.
- Use `sqlite3_key_v2()` when the schema name matters, including attached databases.
- `sqlite3_rekey()` and `sqlite3_rekey_v2()` change keys. They can also decrypt a database by specifying an empty key; require explicit migration intent for that path.
- SQLite3MC-specific functions use the `sqlite3mc_` prefix. Do not assume every SQLite Encryption Extension or SQLCipher convention is identical.

SQL posture:

- `PRAGMA key` and `PRAGMA rekey` are available, but they are easier to leak in logs, traces, query capture, debugging output, and crash reports.
- `ATTACH ... KEY` can attach encrypted databases, but the key string is still sensitive.
- URI parameters can configure encryption, but key-bearing URIs are high leakage risk because URIs commonly appear in logs, diagnostics, process listings, shell history, metrics, and crash reports.

Use SQL or URI keying only when the repository has explicit redaction and logging discipline.

### 6.3 Cipher choice

For new encrypted databases, prefer the repository's existing secure default. If no repository default exists, prefer the modern authenticated default used by SQLite3MC rather than legacy compatibility modes.

SQLite3MC supports multiple cipher schemes, including:

- wxSQLite3 AES-128 CBC without HMAC;
- wxSQLite3 AES-256 CBC without HMAC;
- sqleet ChaCha20-Poly1305 HMAC;
- SQLCipher AES-256 CBC with SHA HMAC variants;
- System.Data.SQLite RC4;
- Ascon-128 v1.2;
- AEGIS family algorithms.

For new development, do not choose AES-CBC-without-HMAC or RC4 unless the task is explicitly legacy compatibility. Treat legacy modes as migration targets, not modern defaults.

Cipher configuration is file-format state. Changing cipher scheme, KDF parameters, page size, reserve bytes, plaintext header behavior, or legacy mode requires migration tests using real fixtures.

Per universal contract §1.5 (Justification), record *why* the cipher, KDF, and page-format choice is the way it is — threat model, performance budget, legacy compatibility, regulatory constraint, or inherited default. A choice without a recorded reason cannot be safely re-evaluated by the next reader.

### 6.4 Rekey and cipher migration

Rekeying is a data migration, not a simple settings edit.

Before implementing rekey or cipher migration, define:

- the old cipher/key format;
- the new cipher/key format;
- whether migration is in-place or copy-based;
- transaction and crash-safety expectations;
- backup/rollback plan;
- verification after migration;
- behavior for wrong old key or failed new key;
- user-visible recovery path.

Test rekey with fixtures, wrong keys, interrupted operations where feasible, and backup/restore workflows.

### 6.5 Attachments and multiple databases

SQLite3MC can handle encrypted and unencrypted databases together through `ATTACH`, and each database can use a different cipher scheme.

When touching `ATTACH` behavior:

- key each attached schema explicitly;
- test cross-database queries;
- test backup and detach behavior;
- verify that migration scripts do not accidentally copy plaintext into unencrypted files;
- ensure temp tables and intermediate data do not leak sensitive content to disk.

---

## 7. Database-file, journal, WAL, temp, and backup safety

### 7.1 What encryption covers and does not cover

SQLite3MC encrypts database files and journal files, but not every byte or every storage path is equally protected.

Important boundaries:

- `TEMP` tables are not encrypted by SQLite3MC.
- In-memory databases are not encrypted because they are not database files at rest.
- Bytes 16 through 23 of the database file contain header information that is usually not encrypted.
- Plaintext header features, if enabled, intentionally expose header bytes for compatibility.
- Application logs, caches, telemetry, memory dumps, backups, export files, and temp files are outside SQLite3MC's at-rest encryption boundary unless separately protected.

For sensitive workloads, use `SQLITE_TEMP_STORE=2` or `SQLITE_TEMP_STORE=3` where appropriate, and use `PRAGMA temp_store=MEMORY` when compile-time temp-store policy is not sufficient.

### 7.2 WAL and rollback journals

When a database uses WAL or rollback journaling:

- verify encryption of sidecar files where applicable;
- test checkpoints, crash recovery, and reopen behavior;
- preserve file permissions for `-wal`, `-shm`, and journal files;
- avoid deleting sidecar files as a substitute for proper checkpoint/recovery logic;
- test multiple connections if the application uses them.

SQLite 3.53.0 includes an upstream fix for a WAL-reset corruption bug, but this does not remove the need for connection, checkpoint, and backup discipline.

### 7.3 Backup, restore, VACUUM, and export

Backup and export paths are common leakage points.

Rules:

- distinguish encrypted database backup from plaintext export;
- document and test whether backups preserve encryption, cipher settings, page size, and reserve bytes;
- use SQLite backup APIs, `VACUUM INTO`, or application-specific copy flows deliberately;
- check whether `VACUUM INTO` target URI parameters such as `reserve=N` affect the generated database copy;
- protect dumps, CSV exports, JSON exports, logs, and support bundles separately from SQLite3MC encryption;
- test restore from real encrypted fixtures, not only creation of new databases.

### 7.4 File permissions and deletion

SQLite3MC is at-rest encryption, not a replacement for file permissions or access control.

Preserve or improve:

- restrictive permissions on database, WAL, SHM, journal, backup, and temp directories;
- secure deletion policy where the repository relies on it;
- cleanup of temporary exports and test fixtures;
- redaction in support bundles;
- platform-specific backup exclusion where applicable.

---

## 8. SQLite API, SQL, and migration discipline

### 8.1 SQLite error handling

Expose enough SQLite detail to debug real failures without leaking secrets.

Prefer preserving:

- SQLite primary and extended error codes;
- connection/path context with redacted filenames when needed;
- operation phase: open, key, migrate, query, backup, checkpoint, rekey;
- whether failure was wrong key, missing key, unsupported cipher, corrupt file, permission failure, lock contention, or runtime link mismatch.

Do not convert all SQLite failures into generic booleans or generic exceptions.

### 8.2 SQL feature compatibility

SQLite SQL compatibility is a runtime contract.

Before using a 3.53.0 SQL feature in migrations or generated SQL, verify that all deployment targets load SQLite3MC 2.3.3 / SQLite 3.53.0 or newer.

Be especially cautious with:

- `ALTER TABLE` constraint changes;
- `REINDEX EXPRESSIONS`;
- JSONB functions;
- temp triggers touching the main schema;
- query plans that rely on new optimizer behavior;
- deterministic text output involving floating-point values.

If a repository supports multiple SQLite baselines, write migrations and SQL to the lowest supported runtime or guard/version-check the new feature.

### 8.3 CLI scripts and golden outputs

SQLite 3.53.0 changed human-oriented CLI formatting through QRF.

For tests and automation:

- set `.mode`, `.headers`, `.nullvalue`, `.separator`, and other output controls explicitly;
- avoid comparing default interactive output;
- avoid relying on shell history or command lines that contain keys;
- quote dot-commands deliberately;
- test batch and non-interactive behavior separately from interactive usability.

### 8.4 Generated code and migrations

If SQL is generated by an ORM, migration tool, code generator, or binding:

- update the generator or schema source of truth, not only generated SQL;
- regenerate in a deterministic path;
- test generated migrations against encrypted fixtures;
- verify that schema introspection works after keying the database;
- preserve `user_version`, migration history, and compatibility checks.

---

## 9. Language binding and FFI rules

### 9.1 Apply both protocols

When SQLite3MC is used through a language binding, use this protocol plus the relevant language protocol.

Examples:

- Java/JDBC or JNI/JNA: apply Java protocol and verify native library loading, classpath/resource packaging, and thread/connection lifecycle.
- Kotlin/SQLDelight or JVM/native wrappers: apply Kotlin protocol and verify Gradle metadata, generated database code, and native packaging.
- Python/APSW-style bindings or extension modules: apply Python protocol and verify wheels, ABI, free-threaded CPython posture, and runtime native identity.
- Rust FFI or crates bundling SQLite3MC: apply Rust protocol and verify `build.rs`, `links`, bindgen output, `unsafe` boundaries, and feature flags.
- Node/Electron/native modules: verify prebuilds, Electron ABI, install scripts, and runtime platform selection.
- .NET/native bundles: verify RID-specific packaging and native asset resolution.

### 9.2 Wrapper API design

A good wrapper makes unsafe states hard to represent.

Prefer APIs that:

- require keying before queries or migrations can run;
- distinguish encrypted, plaintext, and unknown database state;
- make cipher and migration intent explicit;
- preserve SQLite errors with redaction;
- close connections deterministically;
- prevent URI/PRAGMA secret leakage;
- expose version and compile-option diagnostics for support;
- allow test fixtures for wrong-key and migration cases.

Avoid APIs that:

- accept optional keys with ambiguous defaults;
- silently create plaintext databases when keying fails;
- auto-migrate cipher formats without backup or user intent;
- hide native-library identity;
- expose raw database handles without lifecycle rules;
- run migrations before applying the key.

### 9.3 FFI safety

For FFI surfaces:

- treat SQLite handles, statement handles, allocated strings, key buffers, and callback pointers as ownership-sensitive;
- pair every allocation/free convention correctly;
- do not keep pointers to temporary key buffers beyond their valid lifetime;
- define thread ownership and callback threading;
- prevent exceptions/panics from crossing C ABI boundaries;
- test with sanitizers where practical;
- document safety preconditions in the native language's idiom.

---

## 10. Testing and verification matrix

### 10.1 Minimum verification for encryption-affecting changes

For changes that affect encryption, keying, cipher config, database lifecycle, or persisted files, verify at least:

- create encrypted database;
- reopen with correct key;
- fail to open/read with wrong key;
- ensure file does not contain obvious plaintext table names or inserted sentinel values where the expected encryption boundary applies;
- run schema migration on encrypted fixture;
- backup and restore encrypted database;
- rekey when relevant;
- verify runtime library identity and compile options;
- verify logs/traces do not include secrets.

### 10.2 Compatibility fixtures

Maintain real fixture files when compatibility matters:

- current default cipher fixture;
- each supported legacy cipher fixture;
- plaintext fixture if the application supports plaintext databases;
- old application-version fixture;
- wrong-key fixture or negative test;
- corrupted/truncated fixture where recovery behavior matters;
- WAL/journal fixture when sidecar handling matters.

Do not replace all fixture tests with mock-level tests. The file format is the contract.

### 10.3 Native verification

Use the repository's exact commands. Where no commands exist, useful checks may include:

```text
native build for each supported platform/configuration
runtime sqlite3_libversion() / sqlite3_sourceid() assertion
PRAGMA compile_options assertion
unit/integration tests using the packaged artifact
ASan/UBSan/Valgrind leak checks where feasible
cross-platform CI smoke tests
package install/uninstall tests
```

For release artifacts, test the installed package, not only the build-tree binary.

### 10.4 Concurrency and durability tests

If the application uses multiple connections, WAL, background workers, or concurrent readers/writers, add or preserve tests for:

- multiple connections with correct keying;
- lock contention and busy timeouts;
- WAL checkpoint behavior;
- crash/restart or process-kill recovery where feasible;
- backup during active use;
- thread ownership rules in the language binding.

### 10.5 Performance tests

Measure before optimizing.

Performance-sensitive changes should consider:

- cipher cost;
- page size and reserve bytes;
- cache size;
- WAL vs rollback journal;
- synchronous mode;
- hardware acceleration and target CPU features;
- binding overhead;
- query planner changes in SQLite 3.53.0.

Do not weaken encryption, durability, or compatibility for unmeasured performance claims.

---

## 11. Security and operational posture

### 11.1 Threat model clarity

SQLite3MC protects database contents at rest under defined assumptions. It does not automatically protect:

- data while the process is running;
- data returned through queries;
- temp tables unless temp storage is forced into memory;
- application logs and telemetry;
- exported files and backups;
- process memory dumps;
- keys stored beside the database;
- compromised application users or compromised hosts.

State the real threat model when changing encryption behavior. The threat model is itself theory in Naur's sense — usually held by a security stakeholder, often not in the diff. Where the agent is acting without it, surface the gap (per universal contract §0).

### 11.2 Secret redaction

Never emit secrets through:

- logs;
- metrics;
- traces;
- SQL query capture;
- crash reports;
- exception messages;
- debug dumps;
- command-line arguments;
- test snapshots;
- support bundles;
- root README examples.

Redact keys, passphrases, key IDs where needed, key-bearing URIs, and SQL statements containing `PRAGMA key`, `PRAGMA rekey`, or `ATTACH ... KEY`.

### 11.3 Secure defaults

For new work:

- require explicit key configuration for encrypted databases;
- fail closed if a key is missing where encryption is required;
- avoid silently falling back to plaintext;
- prefer modern authenticated ciphers;
- use memory temp storage for sensitive workloads;
- keep file permissions restrictive;
- expose diagnostics for version/build identity without exposing secrets.

### 11.4 Supply-chain safety

SQLite3MC is security-relevant native code. Treat dependency changes as security-sensitive.

When touching vendored or prebuilt artifacts:

- verify source and artifact provenance;
- review changelog and security-relevant fixes;
- update SBOM or dependency inventory;
- avoid unpinned downloads in build scripts;
- avoid executing downloaded build tools without checksum/provenance controls;
- test downstream packages after update.

---

## 12. Observability without leakage

Operational feedback should prove the database subsystem works without exposing secrets or sensitive data.

Useful signals:

- SQLite3MC/SQLite version and source ID;
- compile options;
- database open/key/migration phase failures;
- busy/locked timeout counts;
- checkpoint and backup outcomes;
- migration duration and success;
- corrupt-file or wrong-key failure classification;
- native-library load path in debug diagnostics, redacted as needed;
- package artifact version.

Do not log full SQL statements if they can include keys or sensitive data. If query logging is necessary, redact keying operations and sensitive values first.

---

## 13. Deletion and blast-radius rules

Per universal contract §8 (deletion and simplification require proof). SQLite3MC-specific blast-radius surfaces beyond the universal list:

- native source files and generated amalgamation paths;
- headers and exported symbols;
- package artifacts, installers, Docker images, mobile bundles, and WASM glue;
- static and dynamic link references;
- language bindings and generated wrappers;
- SQL migrations and CLI scripts;
- encrypted fixtures and support tools;
- docs, examples, runbooks, and release checklists;
- production data files and backups that may require legacy cipher support.

Removing a cipher, compile option, wrapper method, or legacy compatibility flag can strand existing encrypted databases. Treat such deletion as a data-migration decision, not cleanup. Naur's "amorphous additions" warning applies in reverse here: a deletion made without the cipher/file-format theory destroys structure that *looks* redundant but is in fact load-bearing for some existing on-disk file the agent has never seen.

---

## 14. Documentation and re-cueing

Use `.codex/PROTOCOL_AFAD.md` for docs that describe SQLite3MC integration, public APIs, migrations, operational procedures, or code/documentation synchronization.

Per universal contract §1.6 (re-cueing), preserve the cues that let the next reader rebuild the relevant slice of theory. SQLite3MC-specific homes for those cues:

- version/build facts in the canonical dependency manifest;
- cipher choices and migration rationale in migration notes or AFAD-managed docs;
- key lifecycle in wrapper API docs or security runbooks;
- FFI safety rules in safety comments;
- compile options in build manifests and CI assertions;
- compatibility fixtures in tests;
- operational recovery in runbooks.

Theory the agent could not write down — production threat model nuance, why a particular legacy fixture exists, who chose the current KDF settings, what historical incident led to a defensive zeroization path — should be flagged as a known re-cueing gap so the next reader knows where to ask. Do not pretend an artifact transfers a theory it can only re-cue.

The repository root `README.md` remains a storefront. It may mention that the project supports encrypted SQLite, but detailed cipher configuration, key management, and migration mechanics belong in deeper docs.

---

## 15. Completion checklist

The universal contract §10 (stop conditions) covers the cross-language stops, and §9 defines the agent output template. The checks below are SQLite3MC-specific additions; do not duplicate the universal output template here.

```text
Baseline:
- Did I verify the intended SQLite3MC and SQLite versions at build time AND runtime?

Truth:
- Did I preserve one canonical owner for version, compile options, cipher defaults, key lifecycle, and migration state?

Evidence:
- For encryption changes, did I prove correct-key success, wrong-key failure, and absence of obvious plaintext leakage?
- Did I verify against real encrypted fixtures, not only freshly created scratch databases?

Consequence:
- Did I trace packaging, linking, language bindings, stored files, backups, and support tools?

Invariant:
- Did data integrity, key safety, cipher compatibility, ABI/API compatibility, and migration safety remain intact?

Justification:
- Can I explain why each touched cipher, page-size, KDF, or legacy-mode choice is the way it is — or have I surfaced that as a known gap rather than silently changing it?

Re-cueing:
- Did I update tests, fixtures, build assertions, docs, runbooks, or comments where the learned theory belongs?
- Did I flag what could not be written down, and who currently holds it?

Leakage:
- Did I avoid logging, committing, or documenting real secrets or key-bearing commands?
```

Do not claim completion if runtime library identity is unverified, encryption behavior is untested, or existing encrypted database compatibility is unknown.
