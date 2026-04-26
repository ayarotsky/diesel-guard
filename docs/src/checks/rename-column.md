# Renaming a Column

**Check name:** `RenameColumnCheck`

**Lock type:** ACCESS EXCLUSIVE (brief, but breaks running app instances)

## Bad

Renaming a column breaks running application instances immediately. Any code that references the old column name will fail after the rename is applied, causing downtime.

```sql
ALTER TABLE users RENAME COLUMN email TO email_address;
```

## Good

Use a multi-step migration to maintain compatibility during the transition:

```sql
-- Migration 1: Add new column
ALTER TABLE users ADD COLUMN email_address VARCHAR(255);

-- Outside migration: Backfill in batches
UPDATE users SET email_address = email;

-- Migration 2: Add NOT NULL if needed
ALTER TABLE users ALTER COLUMN email_address SET NOT NULL;

-- Update application code to use email_address

-- Migration 3: Drop old column after deploying code changes
ALTER TABLE users DROP COLUMN email;
```

**Important:** The RENAME COLUMN operation itself is fast (brief ACCESS EXCLUSIVE lock), but the primary risk is application compatibility, not lock duration. All running instances must be updated to reference the new column name before the rename is applied.
