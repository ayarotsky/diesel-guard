# Dropping an Index

**Check name:** `DropIndexCheck`

**Lock type:** ACCESS EXCLUSIVE (blocks all queries)

## Bad

Dropping an index without CONCURRENTLY acquires an ACCESS EXCLUSIVE lock on the table, blocking all queries (SELECT, INSERT, UPDATE, DELETE) until the drop operation completes.

```sql
DROP INDEX idx_users_email;
DROP INDEX IF EXISTS idx_users_username;
```

## Good

Use CONCURRENTLY to drop the index without blocking queries:

```sql
DROP INDEX CONCURRENTLY idx_users_email;
DROP INDEX CONCURRENTLY IF EXISTS idx_users_username;
```

**Important:** CONCURRENTLY requires Postgres 9.2+ and cannot run inside a transaction block.

**For Diesel migrations:** Add a `metadata.toml` file to your migration directory:

```toml
# migrations/2024_01_01_drop_user_index/metadata.toml
run_in_transaction = false
```

**For SQLx migrations:** Add the no-transaction directive at the top of your migration file:

```sql
-- no-transaction
DROP INDEX CONCURRENTLY idx_users_email;
```

**Note:** Dropping an index concurrently takes longer than a regular drop and uses more resources, but allows concurrent queries to continue. If it fails, the index may be left in an "invalid" state and should be dropped again.
