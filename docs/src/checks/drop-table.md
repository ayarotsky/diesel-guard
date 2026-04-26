# Dropping a Table

**Check name:** `DropTableCheck`

**Lock type:** ACCESS EXCLUSIVE (blocks all operations)

## Bad

Dropping a table permanently deletes all data, indexes, triggers, and constraints. This operation acquires an ACCESS EXCLUSIVE lock and cannot be undone after the transaction commits. Foreign key relationships in other tables may block the drop or cause cascading deletes.

```sql
DROP TABLE users;
DROP TABLE IF EXISTS orders CASCADE;
```

## Good

Before dropping a table in production, take these precautions:

```sql
-- Step 1: Verify the table is no longer in use
-- Check application code for references to this table
-- Monitor for queries against the table

-- Step 2: Check for foreign key dependencies
SELECT
  tc.table_name, kcu.column_name, rc.constraint_name
FROM information_schema.table_constraints tc
JOIN information_schema.key_column_usage kcu ON tc.constraint_name = kcu.constraint_name
JOIN information_schema.referential_constraints rc ON tc.constraint_name = rc.constraint_name
WHERE rc.unique_constraint_schema = 'public'
  AND rc.unique_constraint_name IN (
    SELECT constraint_name FROM information_schema.table_constraints
    WHERE table_name = 'users' AND constraint_type IN ('PRIMARY KEY', 'UNIQUE')
  );

-- Step 3: Ensure backups exist or data has been migrated

-- Step 4: Drop the table (use safety-assured if intentional)
-- safety-assured:start
DROP TABLE users;
-- safety-assured:end
```

## Important Considerations

- Verify all application code references have been removed and deployed
- Check for foreign keys in other tables that reference this table
- Ensure data backups exist before dropping
- Consider renaming the table first (e.g., `users_deprecated`) and waiting before dropping
