# TRUNCATE TABLE

**Check name:** `TruncateTableCheck`

**Lock type:** ACCESS EXCLUSIVE (blocks all operations)

## Bad

TRUNCATE TABLE acquires an ACCESS EXCLUSIVE lock, blocking all operations (reads and writes) on the table. Unlike DELETE, TRUNCATE cannot be batched or throttled, making it unsuitable for large tables in production environments.

```sql
TRUNCATE TABLE users;
TRUNCATE TABLE orders, order_items;
```

## Good

Use DELETE with batching to incrementally remove rows while allowing concurrent access:

```sql
-- Delete rows in small batches to allow concurrent access
DELETE FROM users WHERE id IN (
  SELECT id FROM users LIMIT 1000
);

-- Repeat the batched DELETE until all rows are removed
-- (Can be done outside migration with monitoring)

-- Optional: Reset sequences if needed
ALTER SEQUENCE users_id_seq RESTART WITH 1;

-- Optional: Reclaim space
VACUUM users;
```

**Important:** If you absolutely must use TRUNCATE (e.g., in a test environment or during a maintenance window), use a `safety-assured` block:

```sql
-- safety-assured:start
-- Safe because: running in test environment / maintenance window
TRUNCATE TABLE users;
-- safety-assured:end
```
