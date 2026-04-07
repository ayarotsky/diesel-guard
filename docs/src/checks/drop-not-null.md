# DROP NOT NULL

**Check name:** `DropNotNullCheck`

## Bad

Removing a NOT NULL constraint changes a contract that application code may depend on.
Once NULL values are written to this column, any code that reads it without handling NULL
will fail at runtime.

```sql
ALTER TABLE users ALTER COLUMN email DROP NOT NULL;
```

## Good

Ensure this change is intentional and coordinated with application changes. Update all
code paths that read this column to handle NULL before or alongside the migration.

If the change has been reviewed and confirmed safe, suppress it explicitly:

```sql
-- safety-assured:start
ALTER TABLE users ALTER COLUMN email DROP NOT NULL;
-- safety-assured:end
```
