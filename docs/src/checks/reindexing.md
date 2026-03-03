# REINDEX

**Check name:** `ReindexCheck`

**Lock type:** ACCESS EXCLUSIVE (blocks all operations)

## Bad

Reindexing without CONCURRENTLY acquires an ACCESS EXCLUSIVE lock on the table, blocking all operations until complete. Duration depends on index size.

```sql
REINDEX INDEX idx_users_email;
REINDEX TABLE users;
```

## Good

Use CONCURRENTLY to reindex without blocking operations:

```sql
REINDEX INDEX CONCURRENTLY idx_users_email;
REINDEX TABLE CONCURRENTLY users;
```

**Important:** CONCURRENTLY requires Postgres 12+ and cannot run inside a transaction block.

**For Diesel migrations:** Add a `metadata.toml` file to your migration directory:

```toml
# migrations/2024_01_01_reindex_users/metadata.toml
run_in_transaction = false
```

**For SQLx migrations:** Add the no-transaction directive at the top of your migration file:

```sql
-- no-transaction
REINDEX INDEX CONCURRENTLY idx_users_email;
```

**Note:** REINDEX CONCURRENTLY rebuilds the index without locking out writes. If it fails, the index may be left in an "invalid" state — check with `\d tablename` and run REINDEX again if needed.
