# FFHN — deterministic typed measurements

FFHN monitors one selected JSON or HTML value from an HTTP endpoint or local UTF-8 file, validates it against an explicit semantic type, and preserves accepted evidence in auditable per-target state. It supports exact Unicode text, integers, decimals, money, Semantic Versions, explicit-offset date-times, and named typed policy conditions. HTML monitoring distinguishes plain DOM descendant text, document-structured rendered text, and attributes. FFHN never infers values or uses a machine-local time zone. It persists accepted observations, per-condition temporal state, source health, permanent configuration errors, and integration faults; optional process-stdin delivery routes use a durable per-target outbox with immutable payloads.

## Install

Download the archive for your platform from [GitHub Releases](https://github.com/resoltico/ffhn/releases). Each public release ships these maintained binary assets:

- `ffhn-<version>-aarch64-apple-darwin.tar.gz`
- `ffhn-<version>-x86_64-apple-darwin.tar.gz`
- `ffhn-<version>-x86_64-unknown-linux-musl.tar.gz`
- `ffhn-<version>-x86_64-pc-windows-msvc.zip`

Download `ffhn-<version>-checksums.txt` with the matching archive, validate its SHA-256 entry, extract the archive, and add the contained `ffhn` executable to your `PATH`. Source archives are also available as `ffhn-source-<version>.zip` and `ffhn-source-<version>.tar.gz`.

## Quick start

From a source checkout or extracted source archive, materialize the checked-in local JSON example and run it with the installed binary:

```bash
WATCH_ROOT="$(mktemp -d)"
sh ./examples/file-target-json/materialize-target.sh "$WATCH_ROOT/price/target.toml"
ffhn run --watch-root "$WATCH_ROOT" --target price --format summary
ffhn status --watch-root "$WATCH_ROOT" --target price --format json-pretty
```

A target-definition change is intentionally refused once state exists. Reset it explicitly before accepting observations under the new contract:

```bash
ffhn reset --watch-root "$WATCH_ROOT" --target price
```

The reset keeps `target.toml`, acquires the same target lock as runs, and blindly clears only FFHN-owned storage. It never translates old artifacts.

See [docs/targets.md](docs/targets.md) for the target contract,
[docs/cli.md](docs/cli.md) for commands, and
[docs/getting-started.md](docs/getting-started.md) for a from-scratch local target.

## Legal

FFHN is released under the [MIT License](LICENSE). See [NOTICE](NOTICE) and [PATENTS](PATENTS.md).
