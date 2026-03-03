# Adding a UNIQUE Constraint

**Check name:** `AddUniqueConstraintCheck`

**Lock type:** ACCESS EXCLUSIVE (blocks all reads and writes)

## Bad

Adding a UNIQUE constraint via ALTER TABLE acquires an ACCESS EXCLUSIVE lock, blocking all reads and writes during index creation. This is worse than CREATE INDEX without CONCURRENTLY.

```sql
ALTER TABLE users ADD CONSTRAINT users_email_key UNIQUE (email);
ALTER TABLE users ADD UNIQUE (email);  -- Unnamed is also bad
```

## Good

Use CREATE UNIQUE INDEX CONCURRENTLY, then optionally add the constraint:

```sql
-- Step 1: Create the unique index concurrently
CREATE UNIQUE INDEX CONCURRENTLY users_email_idx ON users(email);

-- Step 2 (Optional): Add constraint using the existing index
-- This is instant since the index already exists
ALTER TABLE users ADD CONSTRAINT users_email_key UNIQUE USING INDEX users_email_idx;
```

**Important:** Requires a migration without a transaction block:

For Diesel:
```toml
# migrations/2024_01_01_add_unique/metadata.toml
run_in_transaction = false
```

For SQLx:
```sql
-- no-transaction
CREATE UNIQUE INDEX CONCURRENTLY users_email_idx ON users(email);
```

## Adding a Primary Key

Adding a primary key constraint to an existing table acquires an ACCESS EXCLUSIVE lock, blocking all operations (reads and writes). The operation must also create an index to enforce uniqueness.

**Check name:** `AddPrimaryKeyCheck`

### Bad

```sql
-- Blocks all operations while creating index and adding constraint
ALTER TABLE users ADD PRIMARY KEY (id);
ALTER TABLE users ADD CONSTRAINT users_pkey PRIMARY KEY (id);
```

### Good

Use CREATE UNIQUE INDEX CONCURRENTLY first, then add the primary key constraint using the existing index:

```sql
-- Step 1: Create unique index concurrently (allows concurrent operations)
CREATE UNIQUE INDEX CONCURRENTLY users_pkey ON users(id);

-- Step 2: Add PRIMARY KEY using the existing index (fast, minimal lock)
ALTER TABLE users ADD CONSTRAINT users_pkey PRIMARY KEY USING INDEX users_pkey;
```

**Important:** The CONCURRENTLY approach requires a migration without a transaction block (same `metadata.toml` or `-- no-transaction` directive as above).

**Note:** This approach requires Postgres 11+. For earlier versions, you must use the unsafe `ALTER TABLE ADD PRIMARY KEY` during a maintenance window.
