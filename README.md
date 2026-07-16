# FFHN — deterministic typed measurements

FFHN acquires one JSON or HTML scalar from an HTTP endpoint or local UTF-8 file, validates it against a declared semantic type, and persists accepted evidence as an auditable v2 state document. It supports exact integers, decimals, money, Semantic Versions, explicit-offset date-times, and named typed policy conditions. It deliberately does not infer values or use machine-local time zones. Condition and source-health state are persisted, and optional delivery routes use a durable per-target outbox with immutable process-stdin payloads.

## Install

Download the archive for your platform from [GitHub Releases](https://github.com/resoltico/ffhn/releases). Each public release ships these maintained binary assets:

- `ffhn-<version>-aarch64-apple-darwin.tar.gz`
- `ffhn-<version>-x86_64-apple-darwin.tar.gz`
- `ffhn-<version>-x86_64-unknown-linux-musl.tar.gz`
- `ffhn-<version>-x86_64-pc-windows-msvc.zip`

Download `ffhn-<version>-checksums.txt` with the matching archive, validate its SHA-256 entry, extract the archive, and add the contained `ffhn` executable to your `PATH`. Source archives are also available as `ffhn-source-<version>.zip` and `ffhn-source-<version>.tar.gz`.

## Quick start

Create `checks/price/target.toml`:

```toml
schema_name = "ffhn.target"
schema_version = 9
target_id = "price"
display_name = "Current Price"
enabled = true
escalate_after = 3
declared_type = "money"
conditions = []

[target]
kind = "http"
source_url = "https://example.test/price.json"

[fetch]
engine = "http"
user_agent = "ffhn/example"
accept = "application/json"

[projection]
kind = "json_pointer"
pointer = "/price"

[type_params]
currency = "USD"
```

Run and inspect it:

```bash
ffhn run --watch-root ./checks --target price --format summary
ffhn status --watch-root ./checks --target price --format json-pretty
```

A target-definition change is intentionally refused once state exists. Reset it explicitly:

```bash
ffhn reset --watch-root ./checks --target price
```

The reset keeps `target.toml`, acquires the same target lock as runs, and blindly clears FFHN-owned storage. It never translates old artifacts.

See [docs/targets.md](docs/targets.md) for the target contract,
[docs/cli.md](docs/cli.md) for commands, and
[examples/file-target-json/README.md](examples/file-target-json/README.md) for a local runnable
JSON example.

## Legal

FFHN is released under the [MIT License](LICENSE). See [NOTICE](NOTICE) and [PATENTS](PATENTS.md).
