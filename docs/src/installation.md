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

## pre-commit

Add diesel-guard as a [pre-commit](https://pre-commit.com/) hook to catch unsafe migrations before they're committed.

In your `.pre-commit-config.yaml`:

```yaml
repos:
  - repo: https://github.com/ayarotsky/diesel-guard
    rev: v0.8.0
    hooks:
      - id: diesel-guard
```

The hook triggers whenever a `.sql` file is staged and runs `diesel-guard check` against your migrations directory (as configured in `diesel-guard.toml`).

> If diesel-guard is already installed (via Homebrew, cargo, or the shell installer) and you don't want to use the Rust toolchain, change `language` to `system` in your `.pre-commit-config.yaml`:
>
> ```yaml
> hooks:
>   - id: diesel-guard
>     language: system
> ```

If your migrations live outside the default `migrations/` path, pass the path via `args`:

```yaml
repos:
  - repo: https://github.com/ayarotsky/diesel-guard
    rev: v0.8.0
    hooks:
      - id: diesel-guard
        args: [db/migrate/]
```

## Verify Installation

```sh
diesel-guard --version
```

## Initialize Configuration

Generate a documented configuration file in your project root:

```sh
diesel-guard init
```

This creates a `diesel-guard.toml` with all available options and their descriptions. See [Configuration](configuration.md) for full details.
