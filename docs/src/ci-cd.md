# CI/CD Integration

## GitHub Actions

Add `diesel-guard` to your CI pipeline to automatically check migrations on pull requests.

### Option 1: GitHub Action (Recommended)

Use the official GitHub Action:

```yaml
name: Check Migrations
on: [pull_request]

jobs:
  check-migrations:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      # Pin to specific version (recommended for stability)
      - uses: ayarotsky/diesel-guard@v0.4.0
        with:
          path: migrations/
```

**Versioning:**
- The action automatically installs the diesel-guard CLI version matching the tag
- `@v0.4.0` installs diesel-guard v0.4.0
- `@main` installs the latest version

**Alternatives:**

```yaml
# Always use latest (gets new checks and fixes automatically)
- uses: ayarotsky/diesel-guard@main
  with:
    path: migrations/
```

This will:
- Install diesel-guard
- Check your migrations for unsafe patterns
- Display detailed violation reports in workflow logs
- Fail the workflow if violations are detected

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
