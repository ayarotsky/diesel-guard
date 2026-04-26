# Renaming a Table

**Check name:** `RenameTableCheck`

**Lock type:** ACCESS EXCLUSIVE (blocks on busy tables; breaks running app instances)

## Bad

Renaming a table breaks running application instances immediately. Any code that references the old table name will fail after the rename is applied. Additionally, this operation requires an ACCESS EXCLUSIVE lock which can block on busy tables.

```sql
ALTER TABLE users RENAME TO customers;
```

## Good

Use a multi-step dual-write migration to safely rename the table:

```sql
-- Migration 1: Create new table
CREATE TABLE customers (LIKE users INCLUDING ALL);

-- Update application code to write to BOTH tables

-- Migration 2: Backfill data in batches
INSERT INTO customers
SELECT * FROM users
WHERE id > last_processed_id
LIMIT 10000;

-- Update application code to read from new table

-- Deploy updated application

-- Update application code to stop writing to old table

-- Migration 3: Drop old table
DROP TABLE users;
```

**Important:** This multi-step approach avoids the ACCESS EXCLUSIVE lock issues on large tables and ensures zero downtime. The migration requires multiple deployments coordinated with application code changes.
