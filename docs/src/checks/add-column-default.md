# ADD COLUMN with DEFAULT

**Check name:** `AddColumnCheck`

**Lock type:** ACCESS EXCLUSIVE + full table rewrite (Postgres < 11)

## Bad

In Postgres versions before 11, adding a column with a default value requires a full table rewrite. This acquires an ACCESS EXCLUSIVE lock and can take hours on large tables, blocking all reads and writes.

```sql
ALTER TABLE users ADD COLUMN admin BOOLEAN DEFAULT FALSE;
```

## Good

Add the column first, backfill the data separately, then add the default:

```sql
-- Migration 1: Add column without default
ALTER TABLE users ADD COLUMN admin BOOLEAN;

-- Outside migration: Backfill in batches
UPDATE users SET admin = FALSE WHERE admin IS NULL;

-- Migration 2: Add default for new rows only
ALTER TABLE users ALTER COLUMN admin SET DEFAULT FALSE;
```

**Note:** For Postgres 11+, adding a column with a constant default value is instant and safe. Set `postgres_version = 11` (or higher) in `diesel-guard.toml` to suppress this check for constant defaults.
