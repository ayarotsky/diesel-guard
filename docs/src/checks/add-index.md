# Adding an Index

**Check name:** `AddIndexCheck`

**Lock type:** SHARE (blocks all writes)

## Bad

Creating an index without CONCURRENTLY acquires a SHARE lock, blocking all write operations (INSERT, UPDATE, DELETE) for the duration of the index build.

```sql
CREATE INDEX idx_users_email ON users(email);
CREATE UNIQUE INDEX idx_users_username ON users(username);
```

## Good

Use CONCURRENTLY to allow concurrent writes during the index build:

```sql
CREATE INDEX CONCURRENTLY idx_users_email ON users(email);
CREATE UNIQUE INDEX CONCURRENTLY idx_users_username ON users(username);
```

**Important:** CONCURRENTLY cannot run inside a transaction block.

**For Diesel migrations:** Add a `metadata.toml` file to your migration directory:

```toml
# migrations/2024_01_01_add_user_index/metadata.toml
run_in_transaction = false
```

**For SQLx migrations:** Add the no-transaction directive at the top of your migration file:

```sql
-- no-transaction
CREATE INDEX CONCURRENTLY idx_users_email ON users(email);
```
