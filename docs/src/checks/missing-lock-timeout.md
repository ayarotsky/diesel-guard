# Missing Lock Timeout

**Check name:** `MissingLockTimeoutCheck`

## Bad

DDL statements that acquire locks without both `lock_timeout` and `statement_timeout` configured can hang indefinitely waiting for locks, delaying production traffic. This check flags lock-prone DDL when either setting is missing.

```sql
ALTER TABLE users ADD COLUMN admin BOOLEAN;
```

## Good

Set both `lock_timeout` and `statement_timeout` before any DDL to ensure the migration fails fast rather than holding up production traffic:

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
