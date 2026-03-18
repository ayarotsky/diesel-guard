# Adding Foreign Key without NOT VALID

**Check Name**: `AddForeignKeyCheck`

**Lock Type**: ShareRowExclusiveLock

## Bad

Adding a foreign key with validation requires a `ShareRowExclusiveLock`, which blocks writes on the table.
On large tables, this can cause outages.

```sql
ALTER TABLE orders ADD CONSTRAINT fk_user_id
    FOREIGN KEY (user_id) REFERENCES users(id);
```

### Good

Add the foreign key first without validation using the `NOT VALID` clause. Validate the foreign key later in a separate
migration.

```sql
-- Step 1 (no validation scan; short metadata lock)
ALTER TABLE orders ADD CONSTRAINT fk_user_id
    FOREIGN KEY (user_id) REFERENCES users(id) NOT VALID;

-- Step 2 (separate migration, acquires ShareUpdateExclusiveLock only)
ALTER TABLE orders VALIDATE CONSTRAINT fk_user_id;
```
