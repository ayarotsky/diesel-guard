# CI/CD Integration

## GitHub Actions

### Option 1: GitHub Action (Recommended)

Use the official GitHub Action:

```yaml
name: Check Migrations
on: [pull_request]

jobs:
  check-migrations:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v6
      - uses: ayarotsky/diesel-guard-action@v1
        with:
          path: migrations/
```

This will:
- Install diesel-guard
- Check your migrations for unsafe patterns
- Display detailed violation reports in workflow logs
- Fail the workflow if violations are detected

**Inputs:**

| Input     | Description                                            | Default       |
|-----------|--------------------------------------------------------|---------------|
| `path`    | Path to migrations directory or a single `.sql` file   | `migrations/` |
| `version` | diesel-guard binary version to install (e.g. `0.9.0`)  | `latest`      |

**Pin the diesel-guard binary version** (recommended for reproducible builds):

```yaml
- uses: ayarotsky/diesel-guard-action@v1
  with:
    version: '0.9.0'
```

### Option 2: Manual Installation

For more control or custom workflows:

```yaml
name: Check Migrations
on: [pull_request]

jobs:
  check-migrations:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup Rust toolchain
        uses: actions-rust-lang/setup-rust-toolchain@v1
        with:
          toolchain: stable

      - name: Install diesel-guard
        run: cargo install diesel-guard

      - name: Check DB migrations
        run: diesel-guard check
```

