# Dropping a Column

**Check name:** `DropColumnCheck`

**Lock type:** ACCESS EXCLUSIVE + table rewrite

## Bad

Dropping a column acquires an ACCESS EXCLUSIVE lock and typically triggers a table rewrite. This blocks all operations and can cause errors if application code is still referencing the column.

```sql
ALTER TABLE users DROP COLUMN email;
```

## Good

Remove references from application code first, then drop the column in a later migration:

```sql
-- Step 1: Mark column as unused in application code
-- Deploy application code changes first

-- Step 2: (Optional) Set to NULL to reclaim space
ALTER TABLE users ALTER COLUMN email DROP NOT NULL;
UPDATE users SET email = NULL;

-- Step 3: Drop in later migration after confirming it's unused
ALTER TABLE users DROP COLUMN email;
```

Postgres doesn't support `DROP COLUMN CONCURRENTLY`, so the table rewrite is unavoidable. Staging the removal minimizes risk by ensuring no running application code depends on the column at the time of the drop.
