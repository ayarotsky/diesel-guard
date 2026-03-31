# Diesel Guard 🐘💨

![Build Status](https://github.com/ayarotsky/diesel-guard/actions/workflows/ci.yml/badge.svg?branch=main) [![crates.io](https://img.shields.io/crates/v/diesel-guard)](https://crates.io/crates/diesel-guard) [![docs](https://img.shields.io/badge/docs-documentation-blue)](https://ayarotsky.github.io/diesel-guard/) [![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE) [![codecov](https://codecov.io/github/ayarotsky/diesel-guard/graph/badge.svg?token=YCBD10IGNU)](https://codecov.io/github/ayarotsky/diesel-guard)

**Linter for dangerous Postgres migration patterns in Diesel and SQLx. Prevents downtime caused by unsafe schema changes.**

![demo](demo.gif)

✓ Detects operations that lock tables or cause downtime<br>
✓ Provides safe alternatives for each blocking operation<br>
✓ Works with both Diesel and SQLx migration frameworks<br>
✓ Supports safety-assured blocks for verified operations<br>
✓ Extensible with custom checks<br>

## Why diesel-guard?

**Uses PostgreSQL's own parser.** diesel-guard embeds libpg_query — the C library
compiled into Postgres itself. What diesel-guard flags is exactly what Postgres sees.
If your SQL has a syntax error, diesel-guard reports that too.

**Scriptable custom checks.** Write project-specific rules in Rhai with full access
to the SQL AST. No forking required.

**Version-aware.** Configure `postgres_version` to suppress checks that don't apply
to your version (e.g., constant defaults are safe on PG 11+).

**No database connection required.** Works on SQL files directly — no running Postgres
instance needed in CI.

## Installation

Via Cargo:
```sh
cargo install diesel-guard
```

Via Homebrew:
```sh
brew install ayarotsky/tap/diesel-guard
```

Via shell script (macOS/Linux):
```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/ayarotsky/diesel-guard/releases/latest/download/diesel-guard-installer.sh | sh
```

Via PowerShell (Windows):
```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/ayarotsky/diesel-guard/releases/latest/download/diesel-guard-installer.ps1 | iex"
```

Via pre-commit:
```yaml
repos:
  - repo: https://github.com/ayarotsky/diesel-guard
    rev: v0.8.0
    hooks:
      - id: diesel-guard
```

## Quick Start

```sh
diesel-guard init   # creates diesel-guard.toml
diesel-guard check  # checks ./migrations/ by default
```

When it finds an unsafe migration:

```
❌ Unsafe migration detected in migrations/20240101_add_admin/up.sql

❌ ADD COLUMN with DEFAULT

Problem:
  Adding column 'admin' with DEFAULT on table 'users' requires a full table
  rewrite on Postgres < 11, acquiring an ACCESS EXCLUSIVE lock.

Safe alternative:
  1. Add the column without a default:
     ALTER TABLE users ADD COLUMN admin BOOLEAN;

  2. Backfill data in batches (outside migration):
     UPDATE users SET admin = false WHERE admin IS NULL;

  3. Add default for new rows only:
     ALTER TABLE users ALTER COLUMN admin SET DEFAULT false;
```

## CI/CD

Add to your GitHub Actions workflow:

```yaml
- uses: ayarotsky/diesel-guard@main
```

## What It Detects

28 built-in checks across locking, rewrites, and schema safety. See the [full list of checks](https://ayarotsky.github.io/diesel-guard/checks/overview.html).

## Escape Hatch

When you've reviewed an operation and confirmed it's safe, wrap it in a safety-assured block to suppress the check:

```sql
-- safety-assured:start
ALTER TABLE users DROP COLUMN legacy_field;
-- safety-assured:end
```

## Further Reading

- [Your Diesel Migrations Might Be Ticking Time Bombs](https://dev.to/ayarotsky/your-diesel-migrations-might-be-ticking-time-bombs-30g7)
- [Postgres Locks Explained](https://postgreslocksexplained.com/)
- [Zero-downtime Postgres migrations: the hard parts](https://gocardless.com/blog/zero-downtime-postgres-migrations-the-hard-parts/)
- [Zero-downtime Postgres migrations: a little help](https://gocardless.com/blog/zero-downtime-postgres-migrations-a-little-help/)
- [Seven tips for dealing with Postgres locks](https://www.citusdata.com/blog/2018/02/22/seven-tips-for-dealing-with-postgres-locks/)
- [Move fast and migrate things: how we automated migrations in Postgres](https://benchling.engineering/move-fast-and-migrate-things-how-we-automated-migrations-in-postgres-d60aba0fc3d4)
- [PostgreSQL at scale: database schema changes without downtime](https://medium.com/paypal-tech/postgresql-at-scale-database-schema-changes-without-downtime-20d3749ed680)
- [PostgreSQL Explicit Locking](https://www.postgresql.org/docs/current/explicit-locking.html)

## Credits

Inspired by [strong_migrations](https://github.com/ankane/strong_migrations) by Andrew Kane.

## License

[MIT](LICENSE)

---

If this looks useful, a star helps more developers find it ⭐
