# SET NOT NULL

**Check name:** `AddNotNullCheck`

**Lock type:** ACCESS EXCLUSIVE (blocks all operations while scanning table)

## Bad

Adding a NOT NULL constraint requires scanning the entire table to verify all values are non-null. This acquires an ACCESS EXCLUSIVE lock and blocks all operations.

```sql
ALTER TABLE users ALTER COLUMN email SET NOT NULL;
```

## Good

For large tables, use a CHECK constraint approach that allows concurrent operations:

```sql
-- Step 1: Add CHECK constraint without validating existing rows
ALTER TABLE users ADD CONSTRAINT users_email_not_null_check CHECK (email IS NOT NULL) NOT VALID;

-- Step 2: Validate separately (uses SHARE UPDATE EXCLUSIVE lock)
ALTER TABLE users VALIDATE CONSTRAINT users_email_not_null_check;

-- Step 3: Add NOT NULL constraint (instant if CHECK exists)
ALTER TABLE users ALTER COLUMN email SET NOT NULL;

-- Step 4: Optionally drop redundant CHECK constraint
ALTER TABLE users DROP CONSTRAINT users_email_not_null_check;
```

The VALIDATE step allows concurrent reads and writes, only blocking other schema changes. On Postgres 12+, NOT NULL constraints are more efficient, but this approach still provides better control.
