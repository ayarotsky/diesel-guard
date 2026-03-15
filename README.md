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
- uses: ayarotsky/diesel-guard-action@v1
```

## What It Detects

24 built-in checks across locking, rewrites, and schema safety:

| Check | Risk |
|-------|------|
| ADD COLUMN with DEFAULT | Table rewrite on Postgres < 11 (ACCESS EXCLUSIVE) |
| ADD INDEX without CONCURRENTLY | Blocks writes (SHARE lock) |
| ADD NOT NULL constraint | Full table scan (ACCESS EXCLUSIVE) |
| ADD PRIMARY KEY | Blocks all operations during index creation |
| ADD UNIQUE constraint | ACCESS EXCLUSIVE during index build |
| ALTER COLUMN TYPE | Table rewrite (ACCESS EXCLUSIVE) |
| ADD COLUMN with SERIAL | Table rewrite to populate sequence |
| ADD COLUMN with GENERATED STORED | Table rewrite to compute expressions |
| DROP COLUMN | ACCESS EXCLUSIVE lock |
| DROP INDEX without CONCURRENTLY | ACCESS EXCLUSIVE lock |
| DROP PRIMARY KEY | Breaks FK relationships |
| DROP TABLE | Irreversible, ACCESS EXCLUSIVE |
| DROP DATABASE | Irreversible |
| REINDEX without CONCURRENTLY | ACCESS EXCLUSIVE lock |
| RENAME COLUMN | Breaks running app references immediately |
| RENAME TABLE | Breaks running app references, ACCESS EXCLUSIVE |
| TRUNCATE TABLE | ACCESS EXCLUSIVE, cannot be batched |
| ADD COLUMN with JSON | Breaks DISTINCT/GROUP BY |
| ADD COLUMN with CHAR | Storage waste, comparison bugs |
| ADD COLUMN with TIMESTAMP | DST/timezone hazards |
| PRIMARY KEY with INT/SMALLINT | ID exhaustion risk |
| CREATE EXTENSION | Requires superuser |
| CONSTRAINT without name | Auto-names break future migrations |
| CREATE INDEX with 4+ columns | Ineffective, high storage overhead |

Plus [custom checks via Rhai scripting](https://ayarotsky.github.io/diesel-guard/custom-checks.html).

## Escape Hatch

When you've reviewed an operation and confirmed it's safe, wrap it in a safety-assured block to suppress the check:

```sql
-- safety-assured:start
ALTER TABLE users DROP COLUMN legacy_field;
-- safety-assured:end
```

## Further Reading

- [Your Diesel Migrations Might Be Ticking Time Bombs](https://dev.to/ayarotsky/your-diesel-migrations-might-be-ticking-time-bombs-30g7)
- [Zero-downtime Postgres migrations: the hard parts](https://gocardless.com/blog/zero-downtime-postgres-migrations-the-hard-parts/)
- [Zero-downtime Postgres migrations: a little help](https://gocardless.com/blog/zero-downtime-postgres-migrations-a-little-help/)
- [Seven tips for dealing with Postgres locks](https://www.citusdata.com/blog/2018/02/22/seven-tips-for-dealing-with-postgres-locks/)
- [Move fast and migrate things: how we automated migrations in Postgres](https://benchling.engineering/move-fast-and-migrate-things-how-we-automated-migrations-in-postgres-d60aba0fc3d4)
- [PostgreSQL at scale: database schema changes without downtime](https://medium.com/paypal-tech/postgresql-at-scale-database-schema-changes-without-downtime-20d3749ed680)
- [SQL Style Guide](https://www.sqlstyle.guide/)

## Credits

Inspired by [strong_migrations](https://github.com/ankane/strong_migrations) by Andrew Kane.

## License

[MIT](LICENSE)

---

If this looks useful, a star helps more developers find it ⭐
