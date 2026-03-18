# Adding Check Constraint without NOT VALID

**Check Name**: `AddCheckConstraintCheck`

**Lock Type**: AccessExclusive Lock

## Bad

Adding a check constraint '{constraint_name}' on table '{table_name}' without NOT VALID scans the entire table to validate existing rows, which can block autovacuum.
On larger tables this can cause performance issues.

```sql
ALTER TABLE orders ADD CONSTRAINT check_amount CHECK (amount > 0);
```

### Good

Add the check first without validation using the `NOT VALID` clause. Validate the check later in a separate migration.

```sql
-- Step 1 (no table scans; lock acquired momentarily)
ALTER TABLE orders ADD CONSTRAINT check_amount CHECK (amount > 0) NOT VALID;

-- Step 2 (separate migration, acquires ShareUpdateExclusiveLock only)
ALTER TABLE orders VALIDATE CONSTRAINT check_amount;
```
