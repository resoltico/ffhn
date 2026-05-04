# AGENTS.md — Agent Entry Protocol

**Version:** 2.4.0
**Updated:** 2026-05-04

This file is the repository entry point for agent work. It defines load order, precedence, repository-wide exceptions, and the universal minimum that applies before any specialized language, framework, database/native, domain-modeling, or documentation rule.

## 0. Frame

You are a *transient theory-holder*. You enter the repository cold, build a partial theory of the slice you touch, act on it, and leave. Per Naur (*Programming as Theory Building*, 1985), the program is not the artifact — it is the theory held by the people who build and maintain it; that theory cannot be fully written down and is not transferred by documentation alone.

The protocols in this stack are a method. They are not a substitute for the theory. Their purpose is to keep the *absence* of theory visible, so that:

1. you surface the tacit gap rather than papering over it with confident output;
2. you re-cue the next reader with artifacts that help them rebuild the relevant slice, while flagging what cannot be written down.

A passing build, a closed issue, or a generated patch is not the outcome.

## 1. Required context loading

When opening a repository, load context in this order:

1. Read this file completely.
2. Load `.codex/UNIVERSAL_ENGINEERING_CONTRACT.md` (v2.1.0+). This is the cross-language engineering contract.
3. Load `.codex/AGENTS_EXTRA.md` if it exists. This contains project-specific instructions.
4. Load the language/runtime protocol for each touched surface:
   - Java 26+ / Gradle: `.codex/AGENTS_JAVA26_GRADLE.md` (v2.0.0+)
   - Kotlin 2.4+ / Gradle: `.codex/AGENTS_KOTLIN24_GRADLE.md` (v2.0.0+)
   - Python 3.13+: `.codex/AGENTS_PYTHON313.md` (v2.0.0+)
   - Rust 1.95+ / Cargo: `.codex/AGENTS_RUST195_CARGO.md` (v2.0.0+)
5. Load the application-framework protocol for each touched surface:
   - Tauri 2.10.x: `.codex/AGENTS_TAURI210.md` (v2.0.0+)
6. Load the database/native dependency protocol for each touched surface:
   - SQLite3 Multiple Ciphers 2.3.3 / SQLite 3.53.0: `.codex/AGENTS_SQLITE3MC233_SQLITE353.md` (v2.0.0+)
7. Load the domain-modeling lens **only when the change touches business meaning**: `.codex/DOMAIN_DRIVEN_DESIGN_LENS.md` (v1.0.0+). Triggers include domain state, business rules, workflow names, commands, domain events, permissions, policies, calculations, lifecycle transitions, user-facing business terms, or integration contracts between models. Do not load for purely mechanical work — build wiring, generic plumbing, infrastructure with no domain meaning. The Universal Engineering Contract §1.7 *Domain meaning gate* is the formal trigger.
8. For documentation authoring, documentation refactoring, or code changes that alter documented public contracts, load `.codex/PROTOCOL_AFAD.md` unless the only touched document is the repository root `README.md`.

If a referenced file is absent, continue with the best available context and state the missing file in the work summary when it matters.

If a loaded protocol's major version does not match the universal contract's, treat the mismatch as a known re-cueing gap and surface it.

## 2. Precedence

Use the most specific applicable instruction, but do not silently relax correctness, security, compatibility, or verification requirements.

Precedence order:

1. Explicit user request for the current task.
2. Project-specific instructions in `.codex/AGENTS_EXTRA.md`.
3. Repository-wide rules in this `AGENTS.md`, including the root `README.md` exception.
4. Applicable application-framework protocol.
5. Applicable language/runtime-specific protocol.
6. Applicable database/native dependency protocol.
7. Applicable domain-modeling lens (when the gate fires).
8. Applicable documentation protocol.
9. Universal Engineering Contract.
10. General language, framework, ecosystem, and documentation norms.

When instructions conflict, prefer the stricter or more specific instruction unless it would make the task incorrect. Surface the conflict rather than guessing.

## 3. Universal minimum before changing a system

For every non-trivial change, build the smallest useful system map (per Universal Engineering Contract §1):

- **Truth:** Where does the relevant state live? What is authoritative? Who can mutate it?
- **Evidence:** What proves the system is working? What would reveal failure?
- **Consequence:** What breaks if the touched component disappears or changes shape?
- **Invariant:** What must remain true after the change?
- **Justification:** Can you explain *why* each touched part is the way it is, in terms of the world it maps to? If not, surface that as a known gap rather than a confident edit.
- **Re-cueing:** Where should the cues that help the next reader rebuild this slice of theory live? What part of the relevant theory could not be written down, and who currently holds it?

If the change touches business meaning, additionally apply the UEC §1.7 *Domain meaning gate* and the Domain-Driven Design Lens. Do not force the lens onto mechanical work.

Use this map to decide what to change, how far to widen the change, what to verify, what to document, and what to flag as unresolved.

## 4. Surface dispatch

Language/runtime surfaces:

- Java 26+ / Gradle projects use `.codex/AGENTS_JAVA26_GRADLE.md`.
- Kotlin 2.4+ / Gradle projects use `.codex/AGENTS_KOTLIN24_GRADLE.md`.
- Python 3.13+ projects use `.codex/AGENTS_PYTHON313.md`.
- Rust 1.95+ / Cargo projects use `.codex/AGENTS_RUST195_CARGO.md`.

Application-framework surfaces:

- Tauri 2.10.x apps, plugins, configuration, capabilities, permissions, bundling, updater/signing, mobile targets, and frontend/Rust IPC surfaces use `.codex/AGENTS_TAURI210.md` in addition to the Rust protocol and any applicable frontend language/framework norms.

Database/native dependency surfaces:

- SQLite3 Multiple Ciphers 2.3.3 / SQLite 3.53.0 surfaces use `.codex/AGENTS_SQLITE3MC233_SQLITE353.md` in addition to any applicable language or framework protocol.

Domain-modeling surfaces:

- Changes that touch business meaning, business rules, business state, workflows, commands, domain events, permissions, policies, calculations, lifecycle transitions, user-facing business terms, or integration contracts between models additionally use `.codex/DOMAIN_DRIVEN_DESIGN_LENS.md`. The lens is loaded conditionally, not by language. Touching Java does not mean DDD; touching the *billing* module of a Java app does. Always run the lens triage (lens §1) before applying its tactical chapters.

Other surfaces:

- Other languages, runtimes, frameworks, databases, and native dependencies use the Universal Engineering Contract plus repository-specific instructions. Do not apply Java-, Kotlin-, Python-, Rust-, Tauri-, SQLite3MC-, or DDD-specific rules to unrelated systems unless the repository explicitly asks for them.
- If a repository spans multiple languages, frameworks, or native dependencies, use the relevant protocol for each touched surface and the Universal Engineering Contract across all boundaries.

## 5. Documentation dispatch and root README exception

Use `.codex/PROTOCOL_AFAD.md` for agent-maintained documentation that is meant to stay synchronized with code, public APIs, architectural boundaries, operational procedures, or generated/reference material.

The repository root `README.md` is a special case. Treat it as the front window of the store, not as ordinary documentation and not as an AFAD-managed reference file.

Root `README.md` rules:

- Do not add AFAD frontmatter, symbol atoms, exhaustive API signatures, or schema tables to the root `README.md`.
- Optimize for a human first impression: what the project is, why it matters, how to install or run it, the shortest credible example, and where to go next.
- Keep runnable snippets, but prefer brevity over completeness.
- Link to AFAD-managed docs, reference files, guides, changelogs, or runbooks for detail.
- Preserve project-specific brand, tone, and release positioning unless the user asks to change them.

Nested `README.md` files are governed by their actual role. If a nested README is a component guide, package guide, or operational document, use the documentation protocol where it fits. If it is a user-facing landing page for a package, example, or integration, keep it reader-first and do not force reference-atom structure.

`CHANGELOG.md`, `LICENSE`, `NOTICE`, `SECURITY.md`, `CONTRIBUTING.md`, governance files, release notes, and legal/compliance files follow their own conventions unless project-specific instructions opt them into AFAD.

## 6. Work summary requirement

For non-trivial changes, the final work summary must follow the Universal Engineering Contract §9 output template (Truth, Evidence, Consequence, Invariant, Justification, Re-cueing). For changes that fired the §1.7 domain-meaning gate, also include the proportional domain-design block defined in UEC §9. Keep the summary proportional to the risk of the change. Silence on justification gaps and inexpressible theory claims a theory you do not have.

## 7. Standing working norms

These apply to every non-trivial agent session unless a project-specific override says otherwise. Day-to-day session prompts may reference these subsections by number rather than restating them.

### 7.1 Evidence over theorycrafting

Base claims on the actual project. Inspect code, tests, docs, examples, build files, configuration, scripts, and runtime behavior as needed. Do not rely on assumptions or surface-level reading. If a suspected issue cannot be proven, either investigate further or mark it as unconfirmed and state what evidence is missing.

### 7.2 Investigation freedom and temporary workspace

You may create custom tools, scripts, probes, fixtures, or experiments to investigate, reproduce, validate, or disprove issues. Use any available runtime appropriate for the project — including Ruby v4 (via `ruby-brew`) and Python 3 (via `python3`) — even when the project itself is in a different language.

Put all temporary scripts, logs, generated files, experiments, and investigation artifacts under `tmp/` at the project root, or the project's conventional temporary workspace if it has one. Do not pollute the project tree.

Temporary artifacts must:

- not interfere with quality gates;
- not require project-configuration changes to hide them from checks;
- be deleted before final quality-gate execution unless intentionally promoted into real tests, fixtures, tools, or documentation.

### 7.3 Incidental observations

While reading the codebase, docs, docstrings, examples, tests, build files, or supporting materials, do not ignore unrelated deficiencies you discover. Incorporate them into the current session's workplan rather than skipping.

If `OBSERVATIONS_INCIDENTAL.txt` (or the project's equivalent observation log) exists, read it and resolve every valid item still open.

The Universal Engineering Contract's "next improvement is a separate slice" rule still applies — incorporate when cohesive, defer when truly out of scope, and prefer the project's observation log over silent skip.

### 7.4 Systems over goals

Per Universal Engineering Contract §0, in concrete operational form:

- fix root causes, not symptoms;
- choose clean, decisive architecture over compatibility-preserving compromises;
- choose breaking refactors when they are the correct engineering answer;
- do not add backwards-compatibility layers, migration shims, transitional APIs, or legacy-preserving glue unless genuinely unavoidable;
- when a shim is genuinely unavoidable, defend the decision with proof — name the consumer, the contract, and the removal trigger;
- treat compatibility shims and migrations as technical debt;
- break up god-files when you encounter them.

### 7.5 Quality gates

Run the project's full quality-gate suite at the end of non-trivial work. Iterate on failures until the gates pass. Do not weaken, bypass, exclude, or reconfigure quality gates to obtain a pass.

If the project has a standard check script, use it. Include relevant build, test, lint, formatting, documentation, example, packaging, fuzz/property, publication-dry-run, metadata, and dependency-license checks where applicable.

### 7.6 Tests assert intended behavior

Tests must assert the corrected or newly intended behavior. Do not merely loosen tests, broaden assertions, or skip tests to tolerate broken behavior.

For projects with fuzzing, property tests, randomized tests, or seed corpora: update them where relevant; add or revise seeds carefully to avoid skewing the corpus toward only the discovered cases; run the relevant fuzz/property checks where feasible, including live hands-on fuzzing when the project supports it.

For domain-touching work, prefer tests that state the local language: given a domain situation, when a command or event occurs, then the invariant or state transition holds.

### 7.7 Documentation, CHANGELOG, and public-facing artifacts

Documentation must accurately reflect the implemented system. When code, behavior, commands, examples, APIs, or workflows change, update the corresponding documentation, examples, and any internal parity or consistency docs in the same change. The root `README.md` is a special case per §5.

When the project maintains a `CHANGELOG.md`:

- record user-visible or developer-visible changes under the project's `UNRELEASED` section (or its equivalent);
- write entries from the public reader's point of view;
- never mention this entry-protocol file, internal session prompts, work specifications, the `.codex/` protocol stack, AI-agent context, or other internal scaffolding.

The same public-facing rule applies to README, release notes, examples, error messages, help text, and any user-visible artifact.

### 7.8 Project baseline

Apply the project's specified language, runtime, framework, and platform baseline when modernizing or refactoring code. Do not assume a baseline the project does not specify, and do not silently raise a baseline.

If the touched surface has a protocol in this stack (Java/Kotlin/Python/Rust/Tauri/SQLite3MC/DDD), follow it. If it does not, fall back to the Universal Engineering Contract plus repository-specific instructions per §4.

### 7.9 Final response

For non-trivial work, the final report combines two shapes:

- the Universal Engineering Contract §9 output template (Truth, Evidence, Consequence, Invariant, Justification, Re-cueing) for the structural part, plus the conditional domain-design block when §1.7 fired;
- plus the operational items: what was done; breaking refactors performed (if any); tests, fuzzing, examples, docs, changelog updates; quality-gate commands run and final results; only genuinely blocked items, with precise reasons.

Keep the report proportional to risk. For tiny edits, a concise sentence with verification is enough.

### 7.10 No emoji

Do not add, retain, or introduce emoji anywhere. This rule applies across all programming languages, markup languages, documentation formats, and plain text.

This includes, without limitation, source code, inline comments, documentation comments, docstrings, commit messages, changelogs, release notes, configuration files, documentation, and this AGENTS.md file.

There are no exceptions. Remove any emoji encountered while creating, editing, reviewing, or refactoring content.

### 7.11 In-progress work awareness

Before beginning any non-trivial task, inspect the repository's in-progress state, not just its committed state:

```bash
gh pr list --state open \
  --json number,title,url,headRefName,isDraft,author \
  --jq '.[] | [.number, .headRefName, .title] | @tsv'
```

For each open PR whose branch or title overlaps the task area, read the PR body and the actual diff before proceeding. An open PR is existing theory-in-progress — work a prior session or contributor already built toward the same goal. Starting fresh without reading it destroys that theory rather than building on it.

If an open PR substantially covers the task:

- treat it as the starting point, not a parallel path;
- understand what it does and does not yet do;
- continue from it or explicitly explain why a fresh approach is better.

This extends the §3 system map. The **Truth** axis is not only the committed HEAD; it includes everything currently in-flight. Discovering an open PR mid-task is late — discover it first.
