# Add Foreign Key

## Description

The `add_foreign_key` check flags foreign keys added without the `NOT VALID` option. Adding a foreign key with validation requires a `ShareRowExclusiveLock`, which blocks writes on the table. On large tables, this can cause outages.

## Examples

### Bad
```sql
-- Unsafe Adding foreign key without NOT VALID
ALTER TABLE orders ADD CONSTRAINT fk_user_id
    FOREIGN KEY (user_id) REFERENCES users(id);
```

### Good
```sql
-- Safe
-- Step 1 (no table scan, no lock)
ALTER TABLE orders ADD CONSTRAINT fk_user_id
    FOREIGN KEY (user_id) REFERENCES users(id) NOT VALID;

-- Step 2 (separate migration, acquires ShareUpdateExclusiveLock only)
ALTER TABLE orders VALIDATE CONSTRAINT fk_user_id;
```
