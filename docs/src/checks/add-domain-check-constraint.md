# Domain CHECK Constraint without NOT VALID

**Check Name**: `AddDomainCheckConstraintCheck`

**Lock Type**: AccessExclusive Lock

## Bad

Adding a CHECK constraint to an existing domain without `NOT VALID` causes Postgres to validate all columns that use that domain across all tables. This is a potentially slow, lock-holding full-scan operation.

```sql
ALTER DOMAIN positive_int ADD CONSTRAINT pos_check CHECK (VALUE > 0);
```

## Good

Use `NOT VALID` to add the constraint without scanning existing data, then validate in a separate migration — the same two-step pattern used for table-level CHECK constraints.

```sql
-- Step 1 (fast, no full scan; lock acquired momentarily)
ALTER DOMAIN positive_int ADD CONSTRAINT pos_check CHECK (VALUE > 0) NOT VALID;

-- Step 2 (separate migration, acquires ShareUpdateExclusiveLock only)
ALTER DOMAIN positive_int VALIDATE CONSTRAINT pos_check;
```

> **Note:** `CREATE DOMAIN ... CHECK (...)` is always safe. The domain is new, so no table columns reference it yet — there is no existing data to scan.
