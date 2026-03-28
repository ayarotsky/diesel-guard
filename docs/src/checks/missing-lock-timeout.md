# Missing Lock Timeout

**Check name:** `MissingLockTimeoutCheck`

## Bad

DDL statements that acquire locks without a configured timeout can hang indefinitely waiting for the lock, blocking all subsequent queries on the table for the entire wait duration.

```sql
ALTER TABLE users ADD COLUMN admin BOOLEAN;
```

## Good

Set `lock_timeout` and `statement_timeout` before any DDL to ensure the migration fails fast rather than holding up production traffic:

```sql
SET lock_timeout = '2s';
SET statement_timeout = '60s';
ALTER TABLE users ADD COLUMN admin BOOLEAN;
```

## Disabling

If your team configures timeouts at the connection level (e.g., in your database connection pool), you can disable this check in `diesel-guard.toml`:

```toml
disable_checks = ["MissingLockTimeoutCheck"]
```
