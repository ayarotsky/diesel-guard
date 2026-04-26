# Add Primary Key

**Check name:** `AddPrimaryKeyCheck`

**Lock type:** ACCESS EXCLUSIVE (blocks all reads and writes)

Adding a primary key via `ALTER TABLE` acquires an ACCESS EXCLUSIVE lock for the entire operation — including building the unique index and scanning all rows to validate uniqueness and non-nullability. On large tables this can take a very long time.

## Bad

```sql
ALTER TABLE users ADD PRIMARY KEY (id);
ALTER TABLE users ADD CONSTRAINT users_pkey PRIMARY KEY (id);
```

## Good

Build the unique index concurrently first, then attach it as the primary key. The index build allows concurrent reads and writes; only the final attachment briefly acquires ACCESS EXCLUSIVE.

```sql
-- Step 1: build the index without blocking (run outside a transaction)
CREATE UNIQUE INDEX CONCURRENTLY users_pkey ON users(id);

-- Step 2: attach as primary key (fast, minimal lock)
ALTER TABLE users ADD CONSTRAINT users_pkey PRIMARY KEY USING INDEX users_pkey;
```

**Important:** Step 2 will do a full table scan under ACCESS EXCLUSIVE if the column is not already `NOT NULL`. Mark the column `NOT NULL` before this migration to keep step 2 fast.

**Important:** `CONCURRENTLY` cannot run inside a transaction block.

For Diesel:
```toml
# migrations/2024_01_01_add_primary_key/metadata.toml
run_in_transaction = false
```

For SQLx:
```sql
-- no-transaction
CREATE UNIQUE INDEX CONCURRENTLY users_pkey ON users(id);
```
