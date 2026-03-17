# Domain CHECK Constraints

**Check name:** `DomainCheckConstraintCheck`

**Risk:** Global validation / full-scan risk

## Bad

Adding a `CHECK` constraint to a domain causes Postgres to validate every column using that domain across all tables. There is no `NOT VALID` escape hatch for domain constraints, so this can become an expensive global validation step during deployment.

`CREATE DOMAIN ... CHECK (...)` is also flagged conservatively. It is not the same validation path as altering an existing domain, but it still bakes a global invariant into the schema without an incremental rollout strategy.

```sql
CREATE DOMAIN email AS text CHECK (VALUE ~* '^[^@]+@[^@]+$');

ALTER DOMAIN email ADD CONSTRAINT email_check CHECK (VALUE ~* '^[^@]+@[^@]+$');
```

## Good

Prefer table-level or column-level `CHECK` constraints so you can roll them out with `NOT VALID` and validate each table separately:

```sql
-- Step 1: Add the check without scanning existing rows immediately
ALTER TABLE users
ADD CONSTRAINT users_email_check
CHECK (email ~* '^[^@]+@[^@]+$') NOT VALID;

-- Step 2: Validate later, when you're ready
ALTER TABLE users VALIDATE CONSTRAINT users_email_check;
```

Repeat this pattern for each affected table instead of enforcing the rule at the domain level during a normal migration.

If the invariant truly must live on the domain, schedule the change for a maintenance window rather than attempting an online rollout.
