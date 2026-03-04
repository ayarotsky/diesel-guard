# Diesel Guard

![Build Status](https://github.com/ayarotsky/diesel-guard/actions/workflows/ci.yml/badge.svg?branch=main) [![crates.io](https://img.shields.io/crates/v/diesel-guard)](https://crates.io/crates/diesel-guard) [![docs](https://img.shields.io/badge/docs-documentation-blue)](https://ayarotsky.github.io/diesel-guard/) [![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE) [![codecov](https://codecov.io/github/ayarotsky/diesel-guard/graph/badge.svg?token=YCBD10IGNU)](https://codecov.io/github/ayarotsky/diesel-guard)

**Catch dangerous Postgres migrations before they take down production.**

![demo](demo.gif)

✓ Detects operations that lock tables or cause downtime<br>
✓ Provides safe alternatives for each blocking operation<br>
✓ Works with both Diesel and SQLx migration frameworks<br>
✓ Supports safety-assured blocks for verified operations<br>
✓ Extensible with custom checks<br>

Works with [Diesel](https://diesel.rs) and [SQLx](https://github.com/launchbadge/sqlx).

## Quick Start

```sh
cargo install diesel-guard
diesel-guard init          # creates diesel-guard.toml
diesel-guard check migrations/
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

## Escape Hatch

When you've reviewed an operation and confirmed it's safe, wrap it in a safety-assured block to suppress the check:

```sql
-- safety-assured:start
ALTER TABLE users DROP COLUMN legacy_field;
-- safety-assured:end
```

## Credits

Inspired by [strong_migrations](https://github.com/ankane/strong_migrations) by Andrew Kane.

## License

[MIT](LICENSE)

---

If this looks useful, a star helps more developers find it ⭐
