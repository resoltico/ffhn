# FFHN — deterministic observation graphs

FFHN monitors typed scalar measurements from shared HTTP or local-file sources. A source is acquired once per cycle; its measurements project, parse, and evaluate independent typed policies over that same complete representation. Durable lineage, state, event identities, outboxes, and retry behavior make every accepted decision auditable and deterministic.

FFHN supports exact Unicode text, integers, decimals, money, Semantic Versions, and explicit-offset date-times. It never infers values, rounds policy arithmetic, or uses the machine time zone for typed decisions. HTML extraction uses HTMLCut v13.2.0 with plain DOM text, rendered text, or attributes as explicitly configured.

## Install

On Linux and Windows, download a platform archive and its checksum manifest from [GitHub Releases](https://github.com/resoltico/ffhn/releases), verify the archive’s SHA-256 entry, extract it, and place `ffhn` on your `PATH`.

FFHN does not use Apple notarization. Browser-downloaded macOS binaries are unsupported because Gatekeeper quarantines them, and FFHN does not prescribe an `xattr` clearance flow. Supported macOS installation is a host-native build from the verified source archive or source checkout obtained without browser quarantine.

## Quick start

Create a graph root, then create editable source and measurement configuration templates:

```bash
GRAPH_ROOT="$(mktemp -d)"
ffhn new source --source shop --graph-root "$GRAPH_ROOT"
ffhn new measurement --source shop --measurement price --graph-root "$GRAPH_ROOT"
```

Enable and edit `sources/shop/source.toml` and `sources/shop/measurements/price/measurement.toml` to select a source, projection, declared type, and conditions. Validate configuration without network access or state changes:

```bash
ffhn validate --all --graph-root "$GRAPH_ROOT" --format summary
```

Preview an acquisition without creating lineage, state, outbox records, or delivery attempts:

```bash
ffhn measure --source shop --graph-root "$GRAPH_ROOT" --dry-run --format summary
```

Run one live source cycle, inspect its route-independent state, or start the graph agent:

```bash
ffhn measure --source shop --graph-root "$GRAPH_ROOT" --format summary
ffhn status --source shop --graph-root "$GRAPH_ROOT" --format json-pretty
ffhn agent run --graph-root "$GRAPH_ROOT" --format summary
```

Resets are deliberate clean breaks. `ffhn reset --source <ID>` mints fresh source lineage and removes all owned measurement state; `ffhn reset --source <ID> --measurement <ID>` replaces only the named measurement lineage. FFHN does not read old state as a migration input and provides no migration path.

See [the CLI reference](docs/cli.md), [source and measurement configuration](docs/targets.md), [operation reports](docs/reports.md), and [the architecture guide](docs/architecture.md).

## Legal

FFHN is released under the [MIT License](LICENSE). See [NOTICE](NOTICE) and [PATENTS](PATENTS.md).
