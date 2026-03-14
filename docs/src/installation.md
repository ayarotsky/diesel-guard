# Installation

## From crates.io

```sh
cargo install diesel-guard
```

## Prebuilt Binaries

macOS and Linux:
```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/ayarotsky/diesel-guard/releases/latest/download/diesel-guard-installer.sh | sh
```

Windows (PowerShell):
```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/ayarotsky/diesel-guard/releases/latest/download/diesel-guard-installer.ps1 | iex"
```

Homebrew:
```sh
brew install ayarotsky/tap/diesel-guard
```

## Verify Installation

```sh
diesel-guard --version
```

## Initialize Configuration

Run the interactive setup wizard to create `diesel-guard.toml`:

```sh
diesel-guard init
```

The wizard auto-detects your framework, migrations path, and Postgres version, then confirms each value before writing a minimal config. See [Configuration](configuration.md) for full details.
