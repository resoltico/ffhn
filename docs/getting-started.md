---
afad: "4.0"
version: "5.0.0"
domain: GETTING_STARTED
updated: "2026-05-03"
route:
  keywords: [getting started, install, quick start, release package, sample page, windows]
  questions: ["how do I install ffhn?", "how do I try ffhn quickly?", "what is the fastest verified ffhn sample flow?", "where are the packaged install commands?"]
---

# Getting Started

Use this page for the fastest path from zero to one repeatable FFHN check.

If you want the short storefront overview first, start at [../README.md](../README.md).

## Start Paths

- Build from source if you are already working in Rust or cloning the repository.
- Install a release package if you want a ready-made platform binary.
- Use the checked-in file-target sample if you want the quickest verified live run.

If you are working directly from the repository without installing the binary, replace `ffhn` in the commands below with `cargo run -p ffhn-cli --`.

## Build From Source

```bash
cargo build --release --locked -p ffhn-cli --bin ffhn
./target/release/ffhn --help
```

Maintained builds are pinned to Rust `1.95.0` through [../rust-toolchain.toml](../rust-toolchain.toml). Regular builds and normal CLI usage do not need nightly Rust; nightly is only part of the coverage and fuzzing workflows.

## Install Prebuilt Release Package

### macOS Or Linux

```bash
VERSION="<published-version>" # for example: 4.0.0
case "$(uname -s)/$(uname -m)" in
  Darwin/arm64) TARGET="aarch64-apple-darwin" ;;
  Darwin/x86_64) TARGET="x86_64-apple-darwin" ;;
  Linux/x86_64) TARGET="x86_64-unknown-linux-musl" ;;
  *)
    printf 'unsupported host for packaged FFHN install: %s/%s\n' "$(uname -s)" "$(uname -m)" >&2
    exit 1
    ;;
esac

curl -fsSLO "https://github.com/resoltico/ffhn/releases/download/v${VERSION}/ffhn-${VERSION}-${TARGET}.tar.gz"
curl -fsSLO "https://github.com/resoltico/ffhn/releases/download/v${VERSION}/ffhn-${VERSION}-checksums.txt"
if command -v shasum >/dev/null 2>&1; then
  grep "  ffhn-${VERSION}-${TARGET}.tar.gz$" "ffhn-${VERSION}-checksums.txt" | shasum -a 256 -c
else
  grep "  ffhn-${VERSION}-${TARGET}.tar.gz$" "ffhn-${VERSION}-checksums.txt" | sha256sum -c
fi
tar -xzf "ffhn-${VERSION}-${TARGET}.tar.gz"
mkdir -p "$HOME/.local/bin"
install "ffhn-${VERSION}-${TARGET}/ffhn" "$HOME/.local/bin/ffhn"
export PATH="$HOME/.local/bin:$PATH"
ffhn --help
```

### Windows PowerShell

```powershell
$Version = "<published-version>" # for example: 4.0.0
$Target = "x86_64-pc-windows-msvc"
Invoke-WebRequest "https://github.com/resoltico/ffhn/releases/download/v$Version/ffhn-$Version-$Target.zip" -OutFile "ffhn-$Version-$Target.zip"
Invoke-WebRequest "https://github.com/resoltico/ffhn/releases/download/v$Version/ffhn-$Version-checksums.txt" -OutFile "ffhn-$Version-checksums.txt"
$Expected = ((Select-String -Path "ffhn-$Version-checksums.txt" -Pattern "  ffhn-$Version-$Target\.zip$").Line -replace ' .*', '').ToLowerInvariant()
$Actual = (Get-FileHash "ffhn-$Version-$Target.zip" -Algorithm SHA256).Hash.ToLowerInvariant()
if ($Actual -ne $Expected) { throw "checksum mismatch" }
Expand-Archive "ffhn-$Version-$Target.zip" -DestinationPath .
New-Item -ItemType Directory -Force "$HOME\bin" | Out-Null
Copy-Item "ffhn-$Version-$Target\ffhn*" "$HOME\bin"
$env:Path = "$HOME\bin;$env:Path"
ffhn --help
```

Each prebuilt release package contains the platform binary plus `README.md` and `LICENSE`. The release asset inventory, platform matrix, and packaging policy live in [platform-support.md](platform-support.md).

## Quick Start

FFHN expects one directory per target under a watch root. The default watch root is `./watchlist`, but the quickest deterministic starter flow in this repository uses the checked-in file-target example inside a disposable watch root.

Materialize and run the local example once:

```bash
WATCH_ROOT="$(mktemp -d)"
./examples/file-target-with-notifications/materialize-target.sh \
  "$WATCH_ROOT/release_notes/target.toml"
ffhn run --watch-root "$WATCH_ROOT" --target release_notes
```

Keep that same `$WATCH_ROOT` in your shell for the next commands.

Inspect the current status for that same target:

```bash
ffhn status --watch-root "$WATCH_ROOT" --target release_notes
```

Run the full watch root in parallel:

```bash
ffhn run --watch-root "$WATCH_ROOT" --all --jobs 4
```

Inspect everything without mutating snapshots or run reports:

```bash
ffhn run --watch-root "$WATCH_ROOT" --target release_notes --dry-run
```

When you are done with that disposable quick-start target, remove it with `rm -rf "$WATCH_ROOT"`.

The checked-in `watchlist/demo` directory remains a maintained minimal HTTP starter target. Live runs create local runtime artifacts such as `state.json`, `last_run.json`, and `snapshots/` under that directory, and both `run` and `status` may create `lock/` on first use for locking. Those generated artifacts are ignored by Git.

## Next Docs

- [README.md](../README.md): short product-facing overview
- [cli.md](cli.md): command behavior, exit codes, and `--all` discovery
- [targets.md](targets.md): how to define what FFHN watches
- [reports.md](reports.md) and [run-reports.md](run-reports.md): emitted and persisted JSON
- [examples/README.md](../examples/README.md): checked-in runnable examples
