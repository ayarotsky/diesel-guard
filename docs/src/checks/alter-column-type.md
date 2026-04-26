# Changing Column Type

**Check name:** `AlterColumnTypeCheck`

**Lock type:** ACCESS EXCLUSIVE + full table rewrite

## Bad

Changing a column's type typically requires an ACCESS EXCLUSIVE lock and triggers a full table rewrite, blocking all operations.

```sql
ALTER TABLE users ALTER COLUMN age TYPE BIGINT;
ALTER TABLE users ALTER COLUMN data TYPE JSONB USING data::JSONB;
```

## Good

Use a multi-step approach with a new column:

```sql
-- Migration 1: Add new column
ALTER TABLE users ADD COLUMN age_new BIGINT;

-- Outside migration: Backfill in batches
UPDATE users SET age_new = age::BIGINT;

-- Migration 2: Swap columns
ALTER TABLE users DROP COLUMN age;
ALTER TABLE users RENAME COLUMN age_new TO age;
```

## Safe Type Changes

These type changes do not trigger a table rewrite on Postgres 9.2+ and are safe:

- Increasing VARCHAR length: `VARCHAR(50)` → `VARCHAR(100)`
- Converting to TEXT: `VARCHAR(255)` → `TEXT`
- Increasing numeric precision
